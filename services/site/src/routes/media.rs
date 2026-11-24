//! Uploads (POST /api/media) and serving (GET /media/{id}).

use axum::Router;
use axum::extract::multipart::{Field, MultipartError, MultipartRejection};
use axum::extract::{DefaultBodyLimit, Multipart, Path, RawQuery, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use bytes::{Bytes, BytesMut};
use serde::Serialize;
use uuid::Uuid;

use super::UPLOAD_BODY_LIMIT;
use crate::auth::PortalViewer;
use crate::error::{AppError, AppResult};
use crate::media::{MAX_UPLOAD_BYTES, ProcessError};
use crate::models::{ImageRef, MAX_ALT_LEN, check_line};
use crate::state::AppState;

/// Mounted under /api with the upload body limit.
pub fn upload_router() -> Router<AppState> {
    Router::new()
        .route("/media", post(upload))
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
            "That image is too large. Use one under 12,000 pixels wide and tall.".into(),
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

    let processed = state.media.process(file).await.map_err(processing_error)?;
    let id = Uuid::now_v7();
    state.media.save(id, &processed, viewer.user_id).await?;
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

    #[sqlx::test(migrations = "./migrations")]
    async fn uploads_a_jpeg_and_serves_each_width(db: PgPool) {
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

    #[sqlx::test(migrations = "./migrations")]
    async fn serves_images_stored_in_s3(db: PgPool) {
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
