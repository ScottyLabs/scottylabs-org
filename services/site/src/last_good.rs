//! The last good answer of each public endpoint, kept in memory and served when the database fails, so visitors keep seeing the published content through an outage instead of an error.
//!
//! Only `GET /api/public/*` answers with status 200 are kept, by path, and for the project list by committee. Searches (`q`) are not kept: every search would be a new copy, and neither are paths written with percent-encoding, which name the same content under endless spellings. A copy served in place of an error says so with [`STALE_HEADER`] and is never cached, so the next request asks the database again.
//!
//! A copy never outlives what it shows: it goes when its path answers 404 or 410, when any write that can change public content succeeds (everything but draft saves, edit locks, sign-in and uploads), after [`MAX_AGE`], and for the ticker when its first announcement expires ([`KeepUntil`]). So an outage never brings back an archived project, an unpublished page or an ended announcement.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::{Request, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};

use crate::auth::csrf::is_write;
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
/// A copy is never served once this old, the day the public `stale-if-error` allows a CDN.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Set by a handler on a public answer that stops being true at a known time, such as the ticker when its first announcement expires. Its copy is not served after that.
#[derive(Clone, Copy, Debug)]
pub struct KeepUntil(pub DateTime<Utc>);

struct Copy {
    body: Bytes,
    content_type: Option<HeaderValue>,
    stored_at: Instant,
    /// Not served from this moment on.
    expires: Instant,
}

#[derive(Default)]
struct Copies {
    by_key: HashMap<String, Copy>,
    bytes: usize,
}

impl Copies {
    fn remove(&mut self, key: &str) -> Option<Copy> {
        let removed = self.by_key.remove(key)?;
        self.bytes -= removed.body.len();
        Some(removed)
    }
}

#[derive(Default)]
pub struct LastGood {
    copies: Mutex<Copies>,
}

impl LastGood {
    fn lock(&self) -> std::sync::MutexGuard<'_, Copies> {
        self.copies.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn store(&self, key: String, body: Bytes, content_type: Option<HeaderValue>, expires: Instant) {
        if body.len() > MAX_ENTRY_BYTES {
            return;
        }
        let mut copies = self.lock();
        copies.remove(&key);
        while !copies.by_key.is_empty()
            && (copies.by_key.len() >= MAX_ENTRIES || copies.bytes + body.len() > MAX_TOTAL_BYTES)
        {
            let oldest = copies
                .by_key
                .iter()
                .min_by_key(|(_, copy)| copy.stored_at)
                .map(|(key, _)| key.clone());
            match oldest {
                Some(oldest) => {
                    copies.remove(&oldest);
                }
                None => break,
            }
        }
        copies.bytes += body.len();
        copies.by_key.insert(
            key,
            Copy {
                body,
                content_type,
                stored_at: Instant::now(),
                expires,
            },
        );
    }

    /// The kept copy for `key`, as a response that says it is stale. A copy past its time is dropped instead.
    fn stale(&self, key: &str) -> Option<Response> {
        let mut copies = self.lock();
        let copy = copies.by_key.get(key)?;
        if Instant::now() >= copy.expires {
            copies.remove(key);
            return None;
        }
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

    fn forget(&self, key: &str) {
        self.lock().remove(key);
    }

    /// Drops every copy, after a change that may alter what the public endpoints answer.
    pub fn clear(&self) {
        let mut copies = self.lock();
        copies.by_key.clear();
        copies.bytes = 0;
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.lock().by_key.len()
    }
}

/// What a public answer is kept under: its path, plus the committee for the project list. `None` for anything that is not kept.
fn key(method: &Method, path: &str, query: Option<&str>) -> Option<String> {
    // axum decodes path parameters, so /committees/te%63h is the Tech page under another name. Only the plain spelling, which the site itself uses, is kept, so nobody can push the real copies out with made-up names.
    if method != Method::GET || !path.starts_with(PUBLIC_PREFIX) || path.contains('%') {
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

/// Writes after which public answers may differ: everything under `/api` but draft saves, edit locks, sign-in and uploads, none of which change published content.
fn changes_public_content(method: &Method, path: &str) -> bool {
    if !is_write(method) || !path.starts_with("/api/") {
        return false;
    }
    let unchanged = path.ends_with("/draft")
        || path.ends_with("/lock")
        || path.ends_with("/lock/take-over")
        || path.starts_with("/api/auth/")
        || (method == Method::POST && path == "/api/media");
    !unchanged
}

/// How long a copy made now may be served: [`MAX_AGE`], or less when the answer says when it stops being true.
fn expiry(now: Instant, until: Option<KeepUntil>) -> Instant {
    let limit = now + MAX_AGE;
    match until {
        Some(KeepUntil(at)) => {
            let left = (at - Utc::now()).to_std().unwrap_or(Duration::ZERO);
            limit.min(now + left)
        }
        None => limit,
    }
}

/// Keeps every good public answer and serves the kept one when a later request fails on our side.
pub async fn keep(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if changes_public_content(request.method(), request.uri().path()) {
        let response = next.run(request).await;
        if response.status().is_success() {
            state.last_good.clear();
        }
        return response;
    }
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
    if status.is_client_error() && status != StatusCode::TOO_MANY_REQUESTS {
        // Gone, such as an archived project: an outage must not bring it back.
        state.last_good.forget(&key);
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
    let expires = expiry(
        Instant::now(),
        response.extensions().get::<KeepUntil>().copied(),
    );
    let (parts, body) = response.into_parts();
    match axum::body::to_bytes(body, MAX_ENTRY_BYTES).await {
        Ok(bytes) => {
            state.last_good.store(
                key,
                bytes.clone(),
                parts.headers.get(CONTENT_TYPE).cloned(),
                expires,
            );
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

    fn later() -> Instant {
        Instant::now() + MAX_AGE
    }

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
        assert_eq!(
            key(&get, "/api/public/projects", Some("committee=te%63h")).as_deref(),
            Some("/api/public/projects?committee=tech")
        );
        assert_eq!(key(&get, "/api/public/projects", Some("q=nova")), None);
        assert_eq!(key(&get, "/api/me", None), None);
        assert_eq!(key(&Method::POST, "/api/public/ticker", None), None);
    }

    #[test]
    fn percent_encoded_paths_are_never_kept() {
        let get = Method::GET;
        for path in [
            "/api/public/committees/te%63h",
            "/api/public/projects/n%6Fva",
            "/api/public/%74icker",
        ] {
            assert_eq!(key(&get, path, None), None, "{path}");
        }
    }

    #[test]
    fn writes_that_can_change_public_content_clear_the_copies() {
        for (method, path) in [
            (Method::POST, "/api/projects/01/archive"),
            (Method::POST, "/api/projects/01/publish"),
            (Method::PATCH, "/api/projects/01/settings"),
            (Method::POST, "/api/committee-pages/tech/publish"),
            (Method::POST, "/api/announcements/01/end"),
            (Method::PATCH, "/api/announcements/01"),
            (Method::DELETE, "/api/announcements/01"),
            (Method::POST, "/api/approvals/01/approve"),
            (Method::DELETE, "/api/media/01"),
        ] {
            assert!(changes_public_content(&method, path), "{method} {path}");
        }
        for (method, path) in [
            (Method::GET, "/api/projects/01"),
            (Method::PUT, "/api/projects/01/draft"),
            (Method::PUT, "/api/committee-pages/tech/draft"),
            (Method::POST, "/api/committee-pages/tech/lock"),
            (Method::DELETE, "/api/committee-pages/tech/lock"),
            (Method::POST, "/api/committee-pages/tech/lock/take-over"),
            (Method::POST, "/api/auth/logout"),
            (Method::POST, "/api/media"),
        ] {
            assert!(!changes_public_content(&method, path), "{method} {path}");
        }
    }

    #[test]
    fn keeps_a_bounded_number_of_copies() {
        let store = LastGood::default();
        for index in 0..MAX_ENTRIES + 10 {
            store.store(
                format!("/api/public/projects/p{index}"),
                Bytes::from_static(b"{}"),
                None,
                later(),
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
            later(),
        );
        assert!(store.stale("/big").is_none());
        store.clear();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn copies_are_not_served_past_their_time() {
        let store = LastGood::default();
        let now = Instant::now();
        store.store("/old".into(), Bytes::from_static(b"{}"), None, now);
        assert!(store.stale("/old").is_none());
        assert_eq!(store.len(), 0);
        // The ticker's copy lasts until its first announcement expires, and never more than a day.
        let soon = expiry(
            now,
            Some(KeepUntil(Utc::now() + chrono::Duration::minutes(5))),
        );
        assert!(soon <= now + Duration::from_secs(5 * 60));
        assert!(soon > now + Duration::from_secs(4 * 60));
        let far = expiry(
            now,
            Some(KeepUntil(Utc::now() + chrono::Duration::days(30))),
        );
        assert_eq!(far, now + MAX_AGE);
        let past = expiry(
            now,
            Some(KeepUntil(Utc::now() - chrono::Duration::minutes(1))),
        );
        assert_eq!(past, now);
    }
}
