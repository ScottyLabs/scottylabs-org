//! The last good answer of each public endpoint, kept in memory and served when the database fails, so visitors keep seeing the published content through an outage instead of an error.
//!
//! Only `GET /api/public/*` answers with status 200 are kept, by path, and for the project list by committee. Searches (`q`) are not kept: every search would be a new copy. A copy served in place of an error says so with [`STALE_HEADER`] and is never cached, so the next request asks the database again.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::{Request, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::AppError;
use crate::routes::common::query_param;
use crate::state::AppState;

/// Marks an answer served from the last good copy because the database failed.
pub const STALE_HEADER: &str = "x-content-stale";
const PUBLIC_PREFIX: &str = "/api/public/";
/// Copies kept at most. Public answers exist only for published content, so a real site needs a few dozen.
const MAX_ENTRIES: usize = 256;
/// Larger answers are not kept.
const MAX_ENTRY_BYTES: usize = 1024 * 1024;
/// All copies together stay under this.
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;

struct Copy {
    body: Bytes,
    content_type: Option<HeaderValue>,
    stored_at: Instant,
}

#[derive(Default)]
pub struct LastGood {
    copies: Mutex<HashMap<String, Copy>>,
}

impl LastGood {
    fn store(&self, key: String, body: Bytes, content_type: Option<HeaderValue>) {
        if body.len() > MAX_ENTRY_BYTES {
            return;
        }
        let mut copies = self.copies.lock().unwrap_or_else(PoisonError::into_inner);
        copies.remove(&key);
        let mut total: usize = copies.values().map(|copy| copy.body.len()).sum();
        while !copies.is_empty()
            && (copies.len() >= MAX_ENTRIES || total + body.len() > MAX_TOTAL_BYTES)
        {
            let oldest = copies
                .iter()
                .min_by_key(|(_, copy)| copy.stored_at)
                .map(|(key, _)| key.clone());
            if let Some(removed) = oldest.and_then(|key| copies.remove(&key)) {
                total -= removed.body.len();
            }
        }
        copies.insert(
            key,
            Copy {
                body,
                content_type,
                stored_at: Instant::now(),
            },
        );
    }

    /// The kept copy for `key`, as a response that says it is stale.
    fn stale(&self, key: &str) -> Option<Response> {
        let copies = self.copies.lock().unwrap_or_else(PoisonError::into_inner);
        let copy = copies.get(key)?;
        let mut response = Body::from(copy.body.clone()).into_response();
        let headers = response.headers_mut();
        if let Some(content_type) = &copy.content_type {
            headers.insert(CONTENT_TYPE, content_type.clone());
        }
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
        headers.insert(STALE_HEADER, HeaderValue::from_static("true"));
        Some(response)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.copies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// What a public answer is kept under: its path, plus the committee for the project list. `None` for anything that is not kept.
fn key(method: &Method, path: &str, query: Option<&str>) -> Option<String> {
    if method != Method::GET || !path.starts_with(PUBLIC_PREFIX) {
        return None;
    }
    if path == "/api/public/projects" {
        if query_param(query, "q").is_some() {
            return None;
        }
        return Some(match query_param(query, "committee") {
            Some(committee) => format!("{path}?committee={committee}"),
            None => path.to_owned(),
        });
    }
    // The other public endpoints take no query, so any query names the same answer.
    Some(path.to_owned())
}

/// Keeps every good public answer and serves the kept one when a later request fails on our side.
pub async fn keep(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(key) = key(
        request.method(),
        request.uri().path(),
        request.uri().query(),
    ) else {
        return next.run(request).await;
    };
    let response = next.run(request).await;
    let status = response.status();
    if status.is_server_error() {
        if let Some(stale) = state.last_good.stale(&key) {
            tracing::warn!(path = %key, %status, "served the last good copy of a public answer");
            return stale;
        }
        return response;
    }
    // Only bodies of a known, small size are kept, so nothing is held back from the visitor.
    let small = response
        .body()
        .size_hint()
        .exact()
        .is_some_and(|size| usize::try_from(size).is_ok_and(|size| size <= MAX_ENTRY_BYTES));
    if status != StatusCode::OK || !small {
        return response;
    }
    let (parts, body) = response.into_parts();
    match axum::body::to_bytes(body, MAX_ENTRY_BYTES).await {
        Ok(bytes) => {
            state
                .last_good
                .store(key, bytes.clone(), parts.headers.get(CONTENT_TYPE).cloned());
            Response::from_parts(parts, Body::from(bytes))
        }
        Err(error) => {
            AppError::Internal(anyhow::anyhow!("reading a public answer: {error}")).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_searches_and_unused_queries() {
        let get = Method::GET;
        assert_eq!(
            key(&get, "/api/public/ticker", Some("x=1")).as_deref(),
            Some("/api/public/ticker")
        );
        assert_eq!(
            key(&get, "/api/public/projects", Some("committee=tech&x=1")).as_deref(),
            Some("/api/public/projects?committee=tech")
        );
        assert_eq!(key(&get, "/api/public/projects", Some("q=nova")), None);
        assert_eq!(key(&get, "/api/me", None), None);
        assert_eq!(key(&Method::POST, "/api/public/ticker", None), None);
    }

    #[test]
    fn keeps_a_bounded_number_of_copies() {
        let store = LastGood::default();
        for index in 0..MAX_ENTRIES + 10 {
            store.store(
                format!("/api/public/projects/p{index}"),
                Bytes::from_static(b"{}"),
                None,
            );
        }
        assert_eq!(store.len(), MAX_ENTRIES);
        // The oldest went first.
        assert!(store.stale("/api/public/projects/p0").is_none());
        assert!(
            store
                .stale(&format!("/api/public/projects/p{}", MAX_ENTRIES + 9))
                .is_some()
        );

        store.store(
            "/big".into(),
            Bytes::from(vec![b' '; MAX_ENTRY_BYTES + 1]),
            None,
        );
        assert!(store.stale("/big").is_none());
    }
}
