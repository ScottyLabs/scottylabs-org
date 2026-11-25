//! A JSON body extractor whose rejections use the API error shape. `axum::Json` answers bad bodies in plain text; use `crate::json::Json` in handlers instead.

use std::collections::BTreeMap;

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use crate::error::AppError;

pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let value = match axum::Json::<serde_json::Value>::from_request(request, state).await {
            Ok(axum::Json(value)) => value,
            Err(rejection) => return Err(rejection_to_error(&rejection)),
        };
        // Postgres cannot store NUL in text or JSON, so a body holding one would fail as an internal error deep in a handler.
        if has_nul(&value) {
            return Err(AppError::Validation {
                message: "Remove the special characters from what you entered, then try again."
                    .into(),
                fields: BTreeMap::new(),
            });
        }
        serde_json::from_value(value).map(Self).map_err(|error| {
            tracing::debug!(error = %error, "rejected a JSON body");
            not_valid()
        })
    }
}

fn has_nul(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains('\0'),
        serde_json::Value::Array(items) => items.iter().any(has_nul),
        serde_json::Value::Object(entries) => entries
            .iter()
            .any(|(key, value)| key.contains('\0') || has_nul(value)),
        _ => false,
    }
}

fn not_valid() -> AppError {
    AppError::BadRequest(
        "Some of the details sent are missing or not valid. Reload the page and try again.".into(),
    )
}

fn rejection_to_error(rejection: &JsonRejection) -> AppError {
    tracing::debug!(error = %rejection.body_text(), "rejected a JSON body");
    match rejection {
        JsonRejection::MissingJsonContentType(_) => AppError::UnsupportedMediaType(
            "Send the request body as JSON with Content-Type: application/json.".into(),
        ),
        JsonRejection::JsonDataError(_) => not_valid(),
        JsonRejection::BytesRejection(_) if rejection.status().as_u16() == 413 => {
            AppError::PayloadTooLarge("That request is too large.".into())
        }
        _ => AppError::BadRequest(
            "The request body is not valid JSON. Reload the page and try again.".into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::post;
    use http_body_util::BodyExt;
    use serde::Deserialize;
    use tower::ServiceExt;

    use super::*;

    #[derive(Deserialize)]
    struct Input {
        #[allow(dead_code)]
        name: String,
    }

    async fn handler(Json(_): Json<Input>) -> StatusCode {
        StatusCode::NO_CONTENT
    }

    async fn call(
        content_type: Option<&str>,
        body: &'static str,
    ) -> (StatusCode, serde_json::Value) {
        let app = Router::new().route("/", post(handler));
        let mut request = Request::post("/");
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = app
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or_default())
    }

    #[tokio::test]
    async fn rejections_use_the_error_shape() {
        let json = Some("application/json");
        assert_eq!(
            call(json, r#"{"name":"x"}"#).await.0,
            StatusCode::NO_CONTENT
        );

        let (status, body) = call(json, "{").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "bad_request");

        let (status, body) = call(json, r#"{"name":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .ends_with("try again.")
        );

        // NUL cannot be stored, so it is refused before any handler runs.
        let (status, body) = call(json, r#"{"name":"a\u0000b"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_failed");
        let (status, _) = call(json, r#"{"name":"x","extra":["\u0000"]}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        // An escaped backslash before u0000 is ordinary text.
        assert_eq!(
            call(json, r#"{"name":"a\\u0000b"}"#).await.0,
            StatusCode::NO_CONTENT
        );

        let (status, body) = call(None, r#"{"name":"x"}"#).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(body["error"]["code"], "unsupported_media_type");
    }
}
