//! Uploads (POST /api/media), serving (GET /media/{id}), and the admin's list and delete (GET /api/media, DELETE /api/media/{id}).
//!
//! Each person may upload at most [`DAILY_UPLOADS`] images and [`DAILY_UPLOAD_BYTES`] of stored variants in any 24 hours, on top of the per-address rate limit, so one account cannot fill the shared host's disk. Images no page uses are deleted by `crate::retention` after a week; admins can delete one at once.

use axum::extract::multipart::{Field, MultipartError, MultipartRejection};
use axum::extract::{DefaultBodyLimit, Multipart, Path, RawQuery, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, RETRY_AFTER, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::UPLOAD_BODY_LIMIT;
use crate::audit::{self, subject};
use crate::auth::{PortalViewer, Viewer};
use crate::error::{AppError, AppResult};
use crate::media::{MAX_UPLOAD_BYTES, ProcessError, Processed, stored_bytes};
use crate::models::{ImageRef, MAX_ALT_LEN, PersonRef, check_line};
use crate::permissions;
use crate::routes::common::{self, no_store, query_param};
use crate::state::AppState;

/// Uploads one person may make in any 24 hours.
pub const DAILY_UPLOADS: i64 = 100;
/// Stored bytes (every variant) one person may add in any 24 hours.
pub const DAILY_UPLOAD_BYTES: i64 = 500 * 1024 * 1024;
/// Held per person while an upload is counted and recorded, so two uploads at once cannot both take the last place.
const QUOTA_LOCK: i32 = 724_630;

/// Mounted under /api with the upload body limit.
pub fn upload_router() -> Router<AppState> {
    Router::new()
        .route("/media", post(upload).get(list))
        .route("/media/{id}", delete(remove))
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT))
}

/// Mounted at the root: GET /media/{id}.
pub fn serve_router() -> Router<AppState> {
    Router::new().route("/media/{id}", get(serve))
}

#[derive(Serialize)]
struct UploadResponse {
    id: Uuid,
    image: ImageRef,
}

fn too_large() -> AppError {
    AppError::PayloadTooLarge("Choose an image of 5 MB or less.".into())
}

fn multipart_error(error: &MultipartError) -> AppError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        too_large()
    } else {
        AppError::BadRequest("The upload could not be read. Try choosing the image again.".into())
    }
}

/// Reads a field into memory, refusing to go past `limit` bytes.
async fn read_field(field: &mut Field<'_>, limit: usize) -> AppResult<Option<Bytes>> {
    let mut buffer = BytesMut::new();
    while let Some(chunk) = field.chunk().await.map_err(|e| multipart_error(&e))? {
        if buffer.len() + chunk.len() > limit {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(Some(buffer.freeze()))
}

fn processing_error(error: ProcessError) -> AppError {
    match error {
        ProcessError::Unsupported => {
            AppError::UnsupportedMediaType("Upload a JPEG, PNG or WebP image.".into())
        }
        ProcessError::TooLarge => AppError::PayloadTooLarge(
            "That image has too many pixels. Resize it to at most 8,000 by 5,000 pixels and try again.".into(),
        ),
        ProcessError::Unreadable(cause) => {
            tracing::debug!(cause, "an upload could not be decoded");
            AppError::field(
                "file",
                "We couldn't read that image. Export it again as a JPEG or PNG and try once more.",
            )
        }
        ProcessError::Encode(cause) => AppError::Internal(anyhow::anyhow!(cause)),
    }
}

async fn upload(
    State(state): State<AppState>,
    viewer: PortalViewer,
    multipart: Result<Multipart, MultipartRejection>,
) -> AppResult<Response> {
    let mut multipart = multipart.map_err(|_| {
        AppError::BadRequest("Send the image as a multipart form with a file field.".into())
    })?;
    let mut file: Option<Bytes> = None;
    let mut alt = String::new();
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| multipart_error(&e))?
    {
        match field.name() {
            Some("file") if file.is_some() => {
                return Err(AppError::field("file", "Upload one image at a time."));
            }
            Some("file") => {
                file = Some(
                    read_field(&mut field, MAX_UPLOAD_BYTES)
                        .await?
                        .ok_or_else(too_large)?,
                );
            }
            Some("alt") => {
                let raw = read_field(&mut field, MAX_ALT_LEN * 4)
                    .await?
                    .ok_or_else(|| AppError::field("alt", alt_too_long()))?;
                alt = String::from_utf8(raw.to_vec())
                    .map_err(|_| AppError::field("alt", "Write the description as plain text."))?;
            }
            _ => {}
        }
    }
    let file = file
        .filter(|bytes| !bytes.is_empty())
        .ok_or_else(|| AppError::field("file", "Choose an image to upload."))?;
    let alt = if alt.trim().is_empty() {
        String::new()
    } else {
        check_line(&alt, 1, MAX_ALT_LEN, "the image description")
            .map_err(|message| AppError::field("alt", message))?
    };

    // Checked before decoding too, so someone over the limit costs no work.
    if let Some(refusal) = quota_refusal(&mut *state.db.acquire().await?, &viewer, 0).await? {
        return Ok(refusal);
    }
    let processed = state.media.process(file).await.map_err(processing_error)?;
    let id = Uuid::now_v7();
    state.media.store_objects(id, &processed).await?;
    if let Some(refusal) = record_upload(&state, &viewer, id, &processed)
        .await
        .inspect_err(|_| {
            forget_objects(&state, id, &processed);
        })?
    {
        forget_objects(&state, id, &processed);
        return Ok(refusal);
    }
    let largest = processed.largest();
    let body = UploadResponse {
        id,
        image: ImageRef {
            src: format!("/media/{id}"),
            alt,
            width: largest.map(|v| v.width),
            height: largest.map(|v| v.height),
        },
    };
    let mut response = (StatusCode::CREATED, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

/// Counts the upload against the person's quota and records it with its audit entry, in one transaction. Returns the 429 to send instead when it would go over.
async fn record_upload(
    state: &AppState,
    viewer: &Viewer,
    id: Uuid,
    processed: &Processed,
) -> AppResult<Option<Response>> {
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2::text))")
        .bind(QUOTA_LOCK)
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
    let bytes = stored_bytes(processed);
    if let Some(refusal) = quota_refusal(&mut tx, viewer, bytes).await? {
        return Ok(Some(refusal));
    }
    state
        .media
        .insert(&mut tx, id, processed, viewer.user_id)
        .await?;
    let largest = processed.largest();
    audit::record_details(
        &mut tx,
        audit::Entry {
            actor: Some(viewer.user_id),
            action: "media.upload",
            subject_type: subject::MEDIA,
            subject_id: Some(id.to_string()),
            committee: None,
            summary: "Uploaded an image".into(),
        },
        json!({
            "after": {
                "id": id,
                "contentType": processed.format.content_type(),
                "width": largest.map(|v| v.width),
                "height": largest.map(|v| v.height),
                "bytes": bytes,
            }
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(None)
}

/// Removes the S3 objects of an upload that was not recorded, in the background. Without S3 there is nothing to remove.
fn forget_objects(state: &AppState, id: Uuid, processed: &Processed) {
    let media = state.media.clone();
    let storage = media.storage();
    let widths: Vec<i32> = processed
        .variants
        .iter()
        .map(|v| i32::try_from(v.width).unwrap_or(i32::MAX))
        .collect();
    tokio::spawn(async move {
        if let Err(error) = media.remove_objects(id, storage, &widths).await {
            tracing::warn!(error = %error, %id, "could not remove the files of an upload that was not recorded");
        }
    });
}

/// The 429 to send when `adding` more stored bytes would take the viewer past the daily quota, with `Retry-After` set to when the oldest upload counted leaves the 24 hours.
async fn quota_refusal(
    conn: &mut PgConnection,
    viewer: &Viewer,
    adding: i64,
) -> AppResult<Option<Response>> {
    let (count, bytes, oldest): (i64, i64, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT count(*), coalesce(sum(bytes), 0)::bigint, min(created_at)
         FROM media WHERE uploaded_by = $1 AND created_at > now() - interval '24 hours'",
    )
    .bind(viewer.user_id)
    .fetch_one(conn)
    .await?;
    let message = if count >= DAILY_UPLOADS {
        format!(
            "You have uploaded {DAILY_UPLOADS} images in the last 24 hours, the most allowed. Try again tomorrow, or ask an admin if you need more."
        )
    } else if bytes.saturating_add(adding) > DAILY_UPLOAD_BYTES {
        format!(
            "Your uploads in the last 24 hours add up to {} MB, the most allowed. Try again tomorrow, or ask an admin if you need more.",
            DAILY_UPLOAD_BYTES / (1024 * 1024)
        )
    } else {
        return Ok(None);
    };
    let retry_after = oldest
        .map(|oldest| (oldest + chrono::TimeDelta::hours(24) - Utc::now()).num_seconds())
        .unwrap_or(60)
        .max(1);
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({ "error": { "code": "rate_limited", "message": message } })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from(retry_after));
    Ok(Some(response))
}

/// One uploaded image in the admin's list. Mirrors `MediaItem` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaItem {
    id: Uuid,
    image: ImageRef,
    content_type: String,
    bytes: i64,
    uploaded_by: Option<PersonRef>,
    created_at: DateTime<Utc>,
    /// Whether a draft, a published version or an open submission shows it.
    in_use: bool,
}

#[derive(Serialize)]
struct MediaListResponse {
    media: Vec<MediaItem>,
}

#[derive(FromRow)]
struct MediaRow {
    id: Uuid,
    content_type: String,
    width: i32,
    height: i32,
    bytes: i64,
    created_at: DateTime<Utc>,
    uploader_id: Option<Uuid>,
    uploader_andrew_id: Option<String>,
    uploader_name: Option<String>,
    in_use: bool,
}

const LIST_LIMIT: i64 = 50;
const MAX_LIST_LIMIT: i64 = 200;

fn require_media_admin(viewer: &Viewer) -> AppResult<()> {
    if permissions::can_manage_media(viewer) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Only admins can list or delete uploaded images.".into(),
        ))
    }
}

/// GET /api/media?limit=&uploadedBy=: the newest uploads, for admins reviewing what is on the site. `uploadedBy` is an Andrew ID.
async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(query): RawQuery,
) -> AppResult<Response> {
    require_media_admin(&viewer)?;
    let limit = query_param(query.as_deref(), "limit")
        .and_then(|raw| raw.trim().parse::<i64>().ok())
        .unwrap_or(LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let uploaded_by = query_param(query.as_deref(), "uploadedBy").map(|raw| raw.to_lowercase());
    let rows = sqlx::query_as::<_, MediaRow>(&format!(
        "WITH in_use AS ({}) 
         SELECT m.id, m.content_type, m.width, m.height, m.bytes, m.created_at,
                u.id AS uploader_id, u.andrew_id AS uploader_andrew_id, u.name AS uploader_name,
                EXISTS (SELECT 1 FROM in_use WHERE in_use.id = m.id) AS in_use
         FROM media m LEFT JOIN users u ON u.id = m.uploaded_by
         WHERE $2::text IS NULL OR u.andrew_id = $2
         ORDER BY m.created_at DESC, m.id DESC
         LIMIT $1",
        crate::retention::MEDIA_IN_USE
    ))
    .bind(limit)
    .bind(uploaded_by)
    .fetch_all(&state.db)
    .await?;
    let media = rows
        .into_iter()
        .map(|row| MediaItem {
            image: ImageRef {
                src: format!("/media/{}", row.id),
                alt: String::new(),
                width: u32::try_from(row.width).ok(),
                height: u32::try_from(row.height).ok(),
            },
            id: row.id,
            content_type: row.content_type,
            bytes: row.bytes,
            uploaded_by: match (row.uploader_id, row.uploader_andrew_id) {
                (Some(id), Some(andrew_id)) => Some(PersonRef::new(
                    id,
                    andrew_id,
                    row.uploader_name.unwrap_or_default(),
                )),
                _ => None,
            },
            created_at: row.created_at,
            in_use: row.in_use,
        })
        .collect();
    Ok(no_store(Json(MediaListResponse { media }).into_response()))
}

#[derive(FromRow)]
struct StoredMedia {
    id: Uuid,
    storage: String,
    variants: Vec<i32>,
    content_type: String,
    bytes: i64,
    uploaded_by: Option<Uuid>,
    created_at: DateTime<Utc>,
}

/// DELETE /api/media/:id: an admin takes an image down, from storage as well. Pages that still show it lose the picture, and drafts that use it ask for another one at their next save. Browsers that already have it keep their copy until it leaves their cache.
async fn remove(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::parse_id(&id, "image")?;
    let mut tx = state.db.begin().await?;
    let viewer = permissions::recheck(&mut tx, &viewer).await?;
    require_media_admin(&viewer)?;
    let stored = sqlx::query_as::<_, StoredMedia>(
        "SELECT id, storage, variants, content_type, bytes, uploaded_by, created_at
         FROM media WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("image"))?;
    // The files go first: if that fails the row stays, and deleting again finishes the job.
    state
        .media
        .remove_objects(stored.id, &stored.storage, &stored.variants)
        .await?;
    sqlx::query("DELETE FROM media WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::record_details(
        &mut tx,
        audit::Entry {
            actor: Some(viewer.user_id),
            action: "media.delete",
            subject_type: subject::MEDIA,
            subject_id: Some(id.to_string()),
            committee: None,
            summary: "Deleted an uploaded image".into(),
        },
        audit::change(
            json!({
                "id": stored.id,
                "contentType": stored.content_type,
                "bytes": stored.bytes,
                "uploadedBy": stored.uploaded_by,
                "uploadedAt": stored.created_at,
                "storage": stored.storage,
            }),
            serde_json::Value::Null,
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

fn alt_too_long() -> String {
    format!("Keep the image description to {MAX_ALT_LEN} characters or fewer.")
}

fn requested_width(query: Option<&str>) -> Option<u32> {
    url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
        .find(|(key, _)| key == "w")
        .and_then(|(_, value)| value.trim().parse::<u32>().ok())
}

async fn serve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let Ok(id) = Uuid::parse_str(&id) else {
        return AppError::not_found("image").into_response();
    };
    match state
        .media
        .open(id, requested_width(query.as_deref()))
        .await
    {
        Ok(Some(media)) => {
            let mut response = media.body.into_response();
            let headers = response.headers_mut();
            if let Ok(value) = HeaderValue::from_str(&media.content_type) {
                headers.insert(CONTENT_TYPE, value);
            }
            if let Some(length) = media.length {
                headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
            }
            headers.insert(
                CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            response
        }
        Ok(None) => AppError::not_found("image").into_response(),
        Err(error) => error.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use std::time::Duration;

    use super::requested_width;
    use crate::media::process::tests::{jpeg, png, webp};
    use crate::media::tests::fake_s3;
    use crate::models::{Committee, Role};
    use crate::testing::{self, Req, Res, TestUser};

    async fn upload(
        app: &axum::Router,
        user: &TestUser,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
        alt: &str,
    ) -> Res {
        Req::post("/api/media")
            .user(user)
            .multipart(&[
                ("file", Some(filename), content_type, bytes),
                ("alt", None, "text/plain", alt.as_bytes()),
            ])
            .send(app)
            .await
    }

    #[test]
    fn parses_the_width() {
        assert_eq!(requested_width(Some("w=960")), Some(960));
        assert_eq!(requested_width(Some("x=1&w=480")), Some(480));
        assert_eq!(requested_width(Some("w=wide")), None);
        assert_eq!(requested_width(None), None);
    }

    // Encoding, decoding and resizing these images takes seconds in a debug build, long enough for the same test in another worktree to replace a database named after the test, so the tests that do it get databases of their own.
    #[tokio::test]
    async fn uploads_a_jpeg_and_serves_each_width() {
        let isolated = testing::IsolatedDb::new().await;
        let db = isolated.pool.clone();
        let app = testing::app(db.clone()).await;
        let lead = testing::user(&db, "uplead", &[(Role::Lead, Some(Committee::Events))]).await;

        let res = upload(
            &app,
            &lead,
            "photo.jpg",
            "image/jpeg",
            &jpeg(2400, 1200),
            " Students at a table ",
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
        let id = res.json["id"].as_str().unwrap().to_owned();
        assert_eq!(res.json["image"]["src"], format!("/media/{id}"));
        assert_eq!(res.json["image"]["alt"], "Students at a table");
        assert_eq!(res.json["image"]["width"], 1920);
        assert_eq!(res.json["image"]["height"], 960);

        let stored: (String, Vec<i32>, String) =
            sqlx::query_as("SELECT content_type, variants, storage FROM media WHERE id = $1::uuid")
                .bind(&id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            stored,
            ("image/jpeg".into(), vec![480, 960, 1920], "database".into())
        );

        for (asked, served) in [
            ("?w=480", 480),
            ("?w=700", 960),
            ("", 1920),
            ("?w=9999", 1920),
        ] {
            let res = Req::get(&format!("/media/{id}{asked}")).send(&app).await;
            assert_eq!(res.status, StatusCode::OK);
            assert_eq!(res.header("content-type"), Some("image/jpeg"));
            assert_eq!(
                res.header("cache-control"),
                Some("public, max-age=31536000, immutable")
            );
            assert_eq!(res.header("x-content-type-options"), Some("nosniff"));
            let decoded = image::load_from_memory(&res.body).unwrap();
            assert_eq!(decoded.width(), served, "{asked}");
        }
    }

    #[tokio::test]
    async fn serves_images_stored_in_s3() {
        let isolated = testing::IsolatedDb::new().await;
        let db = isolated.pool.clone();
        let mut config = testing::config();
        config.s3 = Some(fake_s3(Duration::ZERO).await);
        let app = testing::app_with(db.clone(), config).await;
        let lead = testing::user(&db, "s3lead", &[(Role::Lead, Some(Committee::Events))]).await;

        let res = upload(&app, &lead, "photo.jpg", "image/jpeg", &jpeg(1200, 600), "").await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
        let id = res.json["id"].as_str().unwrap().to_owned();
        let blobs: i64 = sqlx::query_scalar("SELECT count(*) FROM media_blobs")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(blobs, 0);

        for (asked, served) in [("?w=480", 480), ("", 1200)] {
            let res = Req::get(&format!("/media/{id}{asked}")).send(&app).await;
            assert_eq!(res.status, StatusCode::OK);
            let length = res.body.len().to_string();
            assert_eq!(res.header("content-length"), Some(length.as_str()));
            let decoded = image::load_from_memory(&res.body).unwrap();
            assert_eq!(decoded.width(), served, "{asked}");
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn transparent_png_stays_png(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "upadmin", &[(Role::Admin, None)]).await;
        let res = upload(
            &app,
            &admin,
            "logo.png",
            "image/png",
            &png(300, 200, 100),
            "",
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
        assert_eq!(res.json["image"]["alt"], "");
        assert_eq!(res.json["image"]["width"], 300);
        let id = res.json["id"].as_str().unwrap();
        let served = Req::get(&format!("/media/{id}?w=480")).send(&app).await;
        assert_eq!(served.header("content-type"), Some("image/png"));

        let webp_res = upload(&app, &admin, "x.webp", "image/webp", &webp(64, 64), "").await;
        assert_eq!(webp_res.status, StatusCode::CREATED);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn rejects_other_formats_by_content(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "uprejects", &[(Role::Admin, None)]).await;
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        let gif = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";
        for (name, content_type, bytes) in [
            ("logo.svg", "image/svg+xml", &svg[..]),
            ("anim.gif", "image/gif", &gif[..]),
            // The name and header claim PNG; the bytes are SVG.
            ("spoofed.png", "image/png", &svg[..]),
        ] {
            let res = upload(&app, &admin, name, content_type, bytes, "").await;
            assert_eq!(res.status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{name}");
            assert_eq!(res.code(), "unsupported_media_type");
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn rejects_oversized_and_missing_files(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "upsize", &[(Role::Admin, None)]).await;

        let mut big = jpeg(16, 16);
        big.resize(5 * 1024 * 1024 + 1, 0);
        let res = upload(&app, &admin, "big.jpg", "image/jpeg", &big, "").await;
        assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(res.message(), "Choose an image of 5 MB or less.");

        let mut huge = jpeg(16, 16);
        huge.resize(7 * 1024 * 1024, 0);
        let res = upload(&app, &admin, "huge.jpg", "image/jpeg", &huge, "").await;
        assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);

        let res = Req::post("/api/media")
            .user(&admin)
            .multipart(&[("alt", None, "text/plain", b"Nothing attached")])
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            res.json["error"]["fields"]["file"],
            "Choose an image to upload."
        );

        let res = Req::post("/api/media")
            .user(&admin)
            .json(serde_json::json!({}))
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn uploads_need_a_portal_role(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let member = testing::user(&db, "upmember", &[]).await;
        let res = upload(&app, &member, "a.jpg", "image/jpeg", &jpeg(10, 10), "").await;
        assert_eq!(res.status, StatusCode::FORBIDDEN);
        let res = Req::post("/api/media")
            .multipart(&[("file", Some("a.jpg"), "image/jpeg", &jpeg(10, 10))])
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }

    /// Rows for uploads someone made `hours_ago`, without files.
    async fn uploaded(db: &PgPool, by: uuid::Uuid, count: i64, bytes: i64, hours_ago: i32) {
        sqlx::query(
            "INSERT INTO media (id, content_type, width, height, variants, bytes, storage, uploaded_by, created_at)
             SELECT gen_random_uuid(), 'image/jpeg', 10, 10, '{10}', $2, 'database', $1,
                    now() - make_interval(hours => $4)
             FROM generate_series(1, $3)",
        )
        .bind(by)
        .bind(bytes)
        .bind(count)
        .bind(hours_ago)
        .execute(db)
        .await
        .unwrap();
    }

    /// One person uploads at most 100 images and 500 MB in any 24 hours (AVAIL-7).
    #[sqlx::test(migrations = "./migrations")]
    async fn uploads_have_a_daily_quota_per_person(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let lead = testing::user(&db, "quotalead", &[(Role::Lead, Some(Committee::Events))]).await;
        let other =
            testing::user(&db, "quotaother", &[(Role::Lead, Some(Committee::Events))]).await;
        // Yesterday's uploads no longer count.
        uploaded(&db, lead.id, 100, 1000, 25).await;
        uploaded(&db, lead.id, 99, 1000, 2).await;
        let res = upload(&app, &lead, "a.jpg", "image/jpeg", &jpeg(10, 10), "").await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());

        let res = upload(&app, &lead, "a.jpg", "image/jpeg", &jpeg(10, 10), "").await;
        assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(res.code(), "rate_limited");
        assert_eq!(
            res.message(),
            "You have uploaded 100 images in the last 24 hours, the most allowed. Try again tomorrow, or ask an admin if you need more."
        );
        let retry: i64 = res.header("retry-after").unwrap().parse().unwrap();
        assert!((1..=24 * 3600).contains(&retry), "{retry}");
        // Someone else is not affected.
        let res = upload(&app, &other, "a.jpg", "image/jpeg", &jpeg(10, 10), "").await;
        assert_eq!(res.status, StatusCode::CREATED);

        // The stored size counts too.
        uploaded(&db, other.id, 1, super::DAILY_UPLOAD_BYTES, 1).await;
        let res = upload(&app, &other, "a.jpg", "image/jpeg", &jpeg(10, 10), "").await;
        assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
        assert!(res.message().contains("500 MB"));
    }

    /// Uploads are audited, admins can list them and take one down, and nobody else can (UPLOAD-2).
    #[sqlx::test(migrations = "./migrations")]
    async fn admins_list_and_delete_uploads(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "mediaadmin", &[(Role::Admin, None)]).await;
        let director = testing::user(
            &db,
            "mediadir",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        let res = upload(
            &app,
            &director,
            "a.png",
            "image/png",
            &png(20, 10, 255),
            "A dog",
        )
        .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
        let id = res.json["id"].as_str().unwrap().to_owned();

        let audited: (Option<uuid::Uuid>, String, serde_json::Value) = sqlx::query_as(
            "SELECT actor_id, subject_id, details FROM audit_log WHERE action = 'media.upload'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(audited.0, Some(director.id));
        assert_eq!(audited.1, id);
        assert_eq!(audited.2["after"]["width"], 20);

        for user in [&director] {
            let res = Req::get("/api/media").user(user).send(&app).await;
            assert_eq!(res.status, StatusCode::FORBIDDEN);
            let res = Req::delete(&format!("/api/media/{id}"))
                .user(user)
                .send(&app)
                .await;
            assert_eq!(res.status, StatusCode::FORBIDDEN);
        }
        let list = Req::get("/api/media?uploadedBy=mediadir")
            .user(&admin)
            .send(&app)
            .await;
        assert_eq!(list.status, StatusCode::OK, "{}", list.text());
        assert_eq!(list.json["media"][0]["id"], id.as_str());
        assert_eq!(list.json["media"][0]["uploadedBy"]["andrewId"], "mediadir");
        assert_eq!(list.json["media"][0]["inUse"], false);
        assert_eq!(
            list.json["media"][0]["image"]["src"],
            format!("/media/{id}")
        );

        // A project draft that shows it puts it in use.
        let nova = testing::project(&db, Committee::Events, "nova").await;
        sqlx::query("UPDATE projects SET draft = jsonb_set(draft, '{media}', jsonb_build_array(jsonb_build_object('src', '/media/' || $2, 'alt', 'x'))) WHERE id = $1")
            .bind(nova)
            .bind(&id)
            .execute(&db)
            .await
            .unwrap();
        let list = Req::get("/api/media").user(&admin).send(&app).await;
        assert_eq!(list.json["media"][0]["inUse"], true);

        let res = Req::delete(&format!("/api/media/{id}"))
            .user(&admin)
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text());
        let served = Req::get(&format!("/media/{id}")).send(&app).await;
        assert_eq!(served.status, StatusCode::NOT_FOUND);
        let blobs: i64 = sqlx::query_scalar("SELECT count(*) FROM media_blobs")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(blobs, 0);
        let deleted: (Option<uuid::Uuid>, serde_json::Value) =
            sqlx::query_as("SELECT actor_id, details FROM audit_log WHERE action = 'media.delete'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(deleted.0, Some(admin.id));
        assert_eq!(deleted.1["before"]["uploadedBy"], director.id.to_string());
        let again = Req::delete(&format!("/api/media/{id}"))
            .user(&admin)
            .send(&app)
            .await;
        assert_eq!(again.status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn unknown_media_is_not_found(db: PgPool) {
        let app = testing::app(db).await;
        let res = Req::get("/media/0190e0a4-7c1b-7d2e-9f00-000000000001?w=480")
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::NOT_FOUND);
        let res = Req::get("/media/not-a-uuid").send(&app).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND);
    }
}
