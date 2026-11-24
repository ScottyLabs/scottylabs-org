//! Uploaded images: processing, and storage in S3 (Garage) when configured, otherwise in Postgres.
//! Objects live under `media/<uuid>/<width>`.

pub mod process;

use std::time::Duration;

use anyhow::{Context, anyhow};
use axum::body::Body;
use bytes::Bytes;
use futures::future::try_join_all;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use sqlx::PgPool;
use tokio::sync::Semaphore;
use uuid::Uuid;

pub use process::{MAX_UPLOAD_BYTES, ProcessError, Processed};

use crate::config::S3Settings;
use crate::error::{AppError, AppResult};

/// Presigned URLs only need to live for the request that uses them.
const PRESIGN_FOR: Duration = Duration::from_secs(60);
/// Decoding a large image takes hundreds of megabytes, so only a few run at once.
const CONCURRENT_PROCESSING: usize = 2;

struct S3Backend {
    bucket: Bucket,
    credentials: Credentials,
}

pub struct MediaStore {
    s3: Option<S3Backend>,
    db: PgPool,
    http: reqwest::Client,
    processing: Semaphore,
}

/// A stored image ready to send.
pub struct MediaBody {
    pub content_type: String,
    pub length: Option<u64>,
    pub body: Body,
}

impl MediaStore {
    pub fn new(s3: Option<S3Settings>, db: PgPool, http: reqwest::Client) -> Self {
        let s3 = s3.and_then(|settings| {
            match Bucket::new(
                settings.endpoint.clone(),
                UrlStyle::Path,
                settings.bucket.clone(),
                settings.region.clone(),
            ) {
                Ok(bucket) => Some(S3Backend {
                    bucket,
                    credentials: Credentials::new(settings.access_key, settings.secret_key),
                }),
                Err(error) => {
                    tracing::error!(error = %error, "S3 settings are invalid; storing media in Postgres");
                    None
                }
            }
        });
        Self {
            s3,
            db,
            http,
            processing: Semaphore::new(CONCURRENT_PROCESSING),
        }
    }

    fn storage(&self) -> &'static str {
        if self.s3.is_some() { "s3" } else { "database" }
    }

    /// Checks and re-encodes an upload off the async runtime.
    pub async fn process(&self, bytes: Bytes) -> Result<Processed, ProcessError> {
        let _permit = self
            .processing
            .acquire()
            .await
            .map_err(|_| ProcessError::Encode("the media store is shutting down".into()))?;
        tokio::task::spawn_blocking(move || process::process(&bytes))
            .await
            .map_err(|error| ProcessError::Encode(error.to_string()))?
    }

    /// Stores every variant, then records the image. Without a database row nothing is served.
    pub async fn save(&self, id: Uuid, processed: &Processed, uploaded_by: Uuid) -> AppResult<()> {
        let largest = processed
            .largest()
            .ok_or_else(|| AppError::Internal(anyhow!("an upload produced no variants")))?;
        let widths: Vec<i32> = processed
            .variants
            .iter()
            .map(|v| i32::try_from(v.width).unwrap_or(i32::MAX))
            .collect();
        let total: i64 = processed
            .variants
            .iter()
            .map(|v| v.bytes.len() as i64)
            .sum();

        if let Some(s3) = &self.s3 {
            let uploads = processed.variants.iter().map(|variant| {
                self.put_object(
                    s3,
                    id,
                    variant.width,
                    processed.format.content_type(),
                    &variant.bytes,
                )
            });
            try_join_all(uploads).await?;
        }

        let mut tx = self.db.begin().await?;
        sqlx::query(
            "INSERT INTO media (id, content_type, width, height, variants, bytes, storage, uploaded_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(id)
        .bind(processed.format.content_type())
        .bind(i32::try_from(largest.width).unwrap_or(i32::MAX))
        .bind(i32::try_from(largest.height).unwrap_or(i32::MAX))
        .bind(&widths)
        .bind(total)
        .bind(self.storage())
        .bind(uploaded_by)
        .execute(&mut *tx)
        .await?;
        if self.s3.is_none() {
            for variant in &processed.variants {
                sqlx::query("INSERT INTO media_blobs (media_id, width, data) VALUES ($1, $2, $3)")
                    .bind(id)
                    .bind(i32::try_from(variant.width).unwrap_or(i32::MAX))
                    .bind(&variant.bytes)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    async fn put_object(
        &self,
        s3: &S3Backend,
        id: Uuid,
        width: u32,
        content_type: &str,
        bytes: &[u8],
    ) -> AppResult<()> {
        let key = object_key(id, width);
        let url = s3
            .bucket
            .put_object(Some(&s3.credentials), &key)
            .sign(PRESIGN_FOR);
        let response = self
            .http
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes.to_vec())
            .send()
            .await
            .with_context(|| format!("uploading {key}"))?;
        if !response.status().is_success() {
            return Err(AppError::Internal(anyhow!(
                "S3 answered {} when uploading {key}",
                response.status()
            )));
        }
        Ok(())
    }

    /// The smallest stored variant at least `width` wide (or the widest), or `None` for unknown ids.
    pub async fn open(&self, id: Uuid, width: Option<u32>) -> AppResult<Option<MediaBody>> {
        let row: Option<(String, Vec<i32>, String)> =
            sqlx::query_as("SELECT content_type, variants, storage FROM media WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        let Some((content_type, variants, storage)) = row else {
            return Ok(None);
        };
        let Some(chosen) = choose_variant(&variants, width) else {
            return Ok(None);
        };
        if storage == "database" {
            let data: Option<Vec<u8>> = sqlx::query_scalar(
                "SELECT data FROM media_blobs WHERE media_id = $1 AND width = $2",
            )
            .bind(id)
            .bind(chosen)
            .fetch_optional(&self.db)
            .await?;
            return Ok(data.map(|data| MediaBody {
                content_type,
                length: Some(data.len() as u64),
                body: Body::from(data),
            }));
        }
        let s3 = self.s3.as_ref().ok_or_else(|| {
            AppError::Internal(anyhow!("media {id} is in S3, but S3 is not configured"))
        })?;
        self.get_object(
            s3,
            id,
            u32::try_from(chosen).unwrap_or_default(),
            content_type,
        )
        .await
    }

    async fn get_object(
        &self,
        s3: &S3Backend,
        id: Uuid,
        width: u32,
        content_type: String,
    ) -> AppResult<Option<MediaBody>> {
        let key = object_key(id, width);
        let url = s3
            .bucket
            .get_object(Some(&s3.credentials), &key)
            .sign(PRESIGN_FOR);
        let response = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("fetching {key}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            tracing::warn!(key, "media row exists but its object is missing");
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(AppError::Internal(anyhow!(
                "S3 answered {} when fetching {key}",
                response.status()
            )));
        }
        let length = response.content_length();
        let chunks = futures::stream::try_unfold(response, |mut response| async move {
            Ok::<_, reqwest::Error>(response.chunk().await?.map(|chunk| (chunk, response)))
        });
        Ok(Some(MediaBody {
            content_type,
            length,
            body: Body::from_stream(chunks),
        }))
    }
}

fn object_key(id: Uuid, width: u32) -> String {
    format!("media/{id}/{width}")
}

/// The narrowest stored width at least `wanted` wide, or the widest when none is.
pub fn choose_variant(stored: &[i32], wanted: Option<u32>) -> Option<i32> {
    let mut widths = stored.to_vec();
    widths.sort_unstable();
    let widest = widths.last().copied();
    match wanted {
        None => widest,
        Some(wanted) => widths
            .into_iter()
            .find(|w| i64::from(*w) >= i64::from(wanted))
            .or(widest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_the_smallest_variant_that_is_wide_enough() {
        let stored = [480, 960, 1920];
        assert_eq!(choose_variant(&stored, None), Some(1920));
        assert_eq!(choose_variant(&stored, Some(1)), Some(480));
        assert_eq!(choose_variant(&stored, Some(480)), Some(480));
        assert_eq!(choose_variant(&stored, Some(481)), Some(960));
        assert_eq!(choose_variant(&stored, Some(5000)), Some(1920));
        assert_eq!(choose_variant(&[1920, 480], Some(500)), Some(1920));
        assert_eq!(choose_variant(&[], Some(500)), None);
    }

    #[test]
    fn keys_follow_the_contract() {
        let id = Uuid::nil();
        assert_eq!(object_key(id, 960), format!("media/{id}/960"));
    }
}
