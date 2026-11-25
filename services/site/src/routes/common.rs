//! Small helpers the route modules share.

use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::response::Response;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Marks a response as never cached. `static_files::security_headers` already adds this to API responses that set no `Cache-Control`; handlers use it where they say so on purpose.
pub fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Text in curly quotes for activity summaries and messages, trimmed.
pub fn quoted(text: &str) -> String {
    format!("\u{201c}{}\u{201d}", text.trim())
}

/// Runs CPU-bound work, such as sanitizing rich text, on the blocking thread pool, so a slow request never stalls the others that share its async worker thread.
pub async fn blocking<T>(work: impl FnOnce() -> T + Send + 'static) -> AppResult<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("a blocking task failed: {error}")))
}

/// One query parameter for filters and searches: control characters removed (Postgres cannot compare NUL), then trimmed. `None` when missing or empty.
pub fn query_param(raw: Option<&str>, name: &str) -> Option<String> {
    url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| {
            let visible: String = value.chars().filter(|c| !c.is_control()).collect();
            visible.trim().to_owned()
        })
        .filter(|value| !value.is_empty())
}

/// An id from the path. Anything that is not one is a `what` we don't have, so a mangled link answers 404 in the API's error shape.
pub fn parse_id(raw: &str, what: &str) -> AppResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| AppError::not_found(what))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_params_are_trimmed_and_cleaned() {
        assert_eq!(
            query_param(Some("q=%20nova%20&x=1"), "q"),
            Some("nova".into())
        );
        assert_eq!(query_param(Some("q=a%00b%07"), "q"), Some("ab".into()));
        assert_eq!(query_param(Some("q=%20"), "q"), None);
        assert_eq!(query_param(None, "q"), None);
    }

    #[test]
    fn quotes_are_curly_and_trimmed() {
        assert_eq!(quoted(" Nova "), "\u{201c}Nova\u{201d}");
    }

    #[test]
    fn ids_that_are_not_ids_are_not_found() {
        let id = Uuid::now_v7();
        assert_eq!(parse_id(&id.to_string(), "project").unwrap(), id);
        let error = parse_id("not-a-uuid", "announcement").unwrap_err();
        assert_eq!(error.code(), "not_found");
        assert_eq!(error.to_string(), "We couldn't find that announcement.");
    }
}
