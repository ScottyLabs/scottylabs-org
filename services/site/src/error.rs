use std::collections::BTreeMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Every API error. Serializes as `{ "error": { "code", "message", "fields"? } }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{message}")]
    Validation {
        message: String,
        fields: BTreeMap<String, String>,
    },
    #[error("Sign in to continue.")]
    Unauthenticated,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    PayloadTooLarge(String),
    #[error("{0}")]
    UnsupportedMediaType(String),
    #[error("Too many requests. Wait a minute and try again.")]
    RateLimited,
    /// One of the error codes in the contract, which the web client understands. Sign-in answers with redirects instead today, so nothing builds it yet.
    #[allow(dead_code)]
    #[error("Sign-in with ScottyLabs is not set up for this site yet.")]
    AuthNotConfigured,
    #[error("Something went wrong on our side. Try again in a moment.")]
    Internal(#[source] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn not_found(what: &str) -> Self {
        Self::NotFound(format!("We couldn't find that {what}."))
    }

    /// A validation error about one field.
    pub fn field(field: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        let mut fields = BTreeMap::new();
        fields.insert(field.to_owned(), message.clone());
        Self::Validation { message, fields }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::Validation { .. } => "validation_failed",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::PayloadTooLarge(_) => "payload_too_large",
            Self::UnsupportedMediaType(_) => "unsupported_media_type",
            Self::RateLimited => "rate_limited",
            Self::AuthNotConfigured => "auth_not_configured",
            Self::Internal(_) => "internal",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::AuthNotConfigured => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Collects field errors, then fails once with all of them.
#[derive(Debug, Default)]
pub struct Problems {
    fields: BTreeMap<String, String>,
}

impl Problems {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.fields
            .entry(field.into())
            .or_insert_with(|| message.into());
    }

    /// `Ok(())` when nothing was added, otherwise a validation error whose message is the first problem.
    pub fn finish(self) -> AppResult<()> {
        match self.fields.values().next() {
            None => Ok(()),
            Some(first) => Err(AppError::Validation {
                message: first.clone(),
                fields: self.fields,
            }),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<&'a BTreeMap<String, String>>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let Self::Internal(cause) = &self {
            tracing::error!(error = ?cause, "internal error");
        }
        let fields = match &self {
            Self::Validation { fields, .. } if !fields.is_empty() => Some(fields),
            _ => None,
        };
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code(),
                message: self.to_string(),
                fields,
            },
        };
        let mut response = (self.status(), Json(body)).into_response();
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        response
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::Internal(error.into())
    }
}
