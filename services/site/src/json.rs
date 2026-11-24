//! A JSON body extractor whose rejections use the API error shape. `axum::Json` answers bad bodies in plain text; use `crate::json::Json` in handlers instead.

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
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(rejection_to_error(&rejection)),
        }
    }
}

fn rejection_to_error(rejection: &JsonRejection) -> AppError {
    tracing::debug!(error = %rejection.body_text(), "rejected a JSON body");
    match rejection {
        JsonRejection::MissingJsonContentType(_) => AppError::UnsupportedMediaType(
            "Send the request body as JSON with Content-Type: application/json.".into(),
        ),
        JsonRejection::JsonDataError(_) => AppError::BadRequest(
            "Some of the details sent are missing or not valid. Reload the page and try again."
                .into(),
        ),
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

        let (status, body) = call(None, r#"{"name":"x"}"#).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(body["error"]["code"], "unsupported_media_type");
    }
}
