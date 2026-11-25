//! Uploaded images: processing, and storage in S3 (Garage) when configured, otherwise in Postgres.
//! Objects live under `media/<uuid>/<width>`.

pub mod process;

use std::sync::Arc;
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
/// How long fetching an image from S3 may wait for its next bytes before giving up.
const DOWNLOAD_STALL: Duration = Duration::from_secs(15);
const DOWNLOAD_CONNECT: Duration = Duration::from_secs(5);

struct S3Backend {
    bucket: Bucket,
    credentials: Credentials,
}

pub struct MediaStore {
    s3: Option<S3Backend>,
    db: PgPool,
    /// For uploads to S3, with a deadline for the whole request.
    upload_http: reqwest::Client,
    /// For images streamed from S3 to visitors. It has no deadline for the whole download, because a visitor on a slow connection sets the pace; it only gives up when S3 stops sending.
    download_http: reqwest::Client,
    processing: Arc<Semaphore>,
}

/// A stored image ready to send.
pub struct MediaBody {
    pub content_type: String,
    pub length: Option<u64>,
    pub body: Body,
}

impl MediaStore {
    /// `http` uploads to S3 and must not follow redirects. Downloads use a client of their own.
    pub fn new(s3: Option<S3Settings>, db: PgPool, http: reqwest::Client) -> anyhow::Result<Self> {
        let download_http =
            download_client(DOWNLOAD_STALL).context("building the media download client")?;
        Ok(Self::with_clients(s3, db, http, download_http))
    }

    fn with_clients(
        s3: Option<S3Settings>,
        db: PgPool,
        upload_http: reqwest::Client,
        download_http: reqwest::Client,
    ) -> Self {
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
            upload_http,
            download_http,
            processing: Arc::new(Semaphore::new(CONCURRENT_PROCESSING)),
        }
    }

    fn storage(&self) -> &'static str {
        if self.s3.is_some() { "s3" } else { "database" }
    }

    /// Checks and re-encodes an upload off the async runtime.
    pub async fn process(&self, bytes: Bytes) -> Result<Processed, ProcessError> {
        run_limited(&self.processing, move || process::process(&bytes)).await?
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
            .upload_http
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
            .download_http
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

/// Runs blocking work once one of `limit`'s permits is free. The permit moves into the blocking task and is released only when the work ends: tokio cannot cancel blocking tasks, so a client that hangs up mid-upload must not free a slot while its image is still being decoded.
async fn run_limited<T, F>(limit: &Arc<Semaphore>, work: F) -> Result<T, ProcessError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = Arc::clone(limit)
        .acquire_owned()
        .await
        .map_err(|_| ProcessError::Encode("the media store is shutting down".into()))?;
    tokio::task::spawn_blocking(move || {
        let result = work();
        drop(permit);
        result
    })
    .await
    .map_err(|error| ProcessError::Encode(error.to_string()))
}

fn download_client(stall: Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(DOWNLOAD_CONNECT)
        .read_timeout(stall)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("scottylabs-site")
        .build()
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
pub(crate) mod tests {
    use std::collections::HashMap;
    use std::convert::Infallible;
    use std::sync::Mutex;
    use std::time::Instant;

    use axum::Router;
    use axum::extract::State;
    use axum::http::{Method, StatusCode, Uri, header};
    use axum::response::{IntoResponse, Response};
    use futures::StreamExt;
    use http_body_util::BodyExt;

    use super::*;
    use crate::media::process::tests::jpeg;
    use crate::testing;

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

    #[tokio::test]
    async fn a_dropped_upload_keeps_its_slot_until_decoding_ends() {
        let limit = Arc::new(Semaphore::new(1));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel::<()>();
        let request = tokio::spawn({
            let limit = Arc::clone(&limit);
            async move {
                run_limited(&limit, move || {
                    let _ = started_tx.send(());
                    let _ = finish_rx.recv();
                })
                .await
            }
        });
        started_rx.await.unwrap();

        // The client hangs up, so the request is dropped, but the decode keeps running.
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        assert_eq!(limit.available_permits(), 0);

        finish_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while limit.available_permits() == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the slot is released when decoding ends");
    }

    /// A stand-in for Garage: keeps PUT bodies by path and sends them back on GET in
    /// [`CHUNKS`] pieces, pausing before every piece after the first.
    #[derive(Clone)]
    struct FakeS3 {
        objects: Arc<Mutex<HashMap<String, Bytes>>>,
        pause: Duration,
    }

    const CHUNKS: usize = 8;

    async fn fake_s3_handler(
        State(s3): State<FakeS3>,
        method: Method,
        uri: Uri,
        body: Bytes,
    ) -> Response {
        let key = uri.path().to_owned();
        if method == Method::PUT {
            s3.objects.lock().unwrap().insert(key, body);
            return StatusCode::OK.into_response();
        }
        let Some(object) = s3.objects.lock().unwrap().get(&key).cloned() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let size = object.len().div_ceil(CHUNKS).max(1);
        let pieces: Vec<(usize, Bytes)> = (0..object.len())
            .step_by(size)
            .map(|start| object.slice(start..(start + size).min(object.len())))
            .enumerate()
            .collect();
        let pause = s3.pause;
        let stream = futures::stream::iter(pieces).then(move |(index, piece)| async move {
            if index > 0 {
                tokio::time::sleep(pause).await;
            }
            Ok::<_, Infallible>(piece)
        });
        (
            [(header::CONTENT_LENGTH, object.len())],
            Body::from_stream(stream),
        )
            .into_response()
    }

    /// Starts a fake S3 server and returns settings that point at it.
    pub(crate) async fn fake_s3(pause: Duration) -> S3Settings {
        let state = FakeS3 {
            objects: Arc::default(),
            pause,
        };
        let app = Router::new().fallback(fake_s3_handler).with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        S3Settings {
            endpoint: url::Url::parse(&format!("http://{address}")).unwrap(),
            region: "garage".into(),
            access_key: "test-access".into(),
            secret_key: "test-secret".into(),
            bucket: "media".into(),
        }
    }

    /// Saves a 1200 px JPEG (variants 480, 960, 1200) through a store whose downloads give up after `stall` without new bytes.
    async fn stored_in_s3(
        db: &PgPool,
        pause: Duration,
        stall: Duration,
    ) -> (MediaStore, Uuid, Processed) {
        let store = MediaStore::with_clients(
            Some(fake_s3(pause).await),
            db.clone(),
            reqwest::Client::new(),
            download_client(stall).unwrap(),
        );
        let owner = testing::user(db, "s3owner", &[]).await;
        let processed = process::process(&jpeg(1200, 600)).unwrap();
        let id = Uuid::now_v7();
        store.save(id, &processed, owner.id).await.unwrap();
        (store, id, processed)
    }

    // Encoding and resizing the image takes seconds in a debug build, so the S3 tests get databases of their own; `testing::IsolatedDb` says why.
    #[tokio::test]
    async fn slow_downloads_are_not_cut_off() {
        let isolated = testing::IsolatedDb::new().await;
        let db = &isolated.pool;
        let stall = Duration::from_millis(400);
        let (store, id, processed) = stored_in_s3(db, Duration::from_millis(150), stall).await;
        let storage: String = sqlx::query_scalar("SELECT storage FROM media WHERE id = $1")
            .bind(id)
            .fetch_one(db)
            .await
            .unwrap();
        assert_eq!(storage, "s3");

        let started = Instant::now();
        let media = store.open(id, Some(700)).await.unwrap().unwrap();
        let expected = &processed.variants[1];
        assert_eq!(expected.width, 960);
        assert_eq!(media.length, Some(expected.bytes.len() as u64));
        let body = media.body.collect().await.expect("the whole image arrives");
        // Seven pauses of 150 ms: the download outlasts the stall limit twice over and still completes.
        assert!(started.elapsed() > stall * 2);
        assert_eq!(body.to_bytes(), expected.bytes);
    }

    #[tokio::test]
    async fn stalled_downloads_end_with_an_error() {
        let isolated = testing::IsolatedDb::new().await;
        let (store, id, _) = stored_in_s3(
            &isolated.pool,
            Duration::from_secs(2),
            Duration::from_millis(200),
        )
        .await;
        let media = store.open(id, None).await.unwrap().unwrap();
        let started = Instant::now();
        assert!(media.body.collect().await.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
