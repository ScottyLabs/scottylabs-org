//! Per-IP request limits. Platform track.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::state::AppState;

#[derive(Default)]
pub struct RateLimiter;

impl RateLimiter {
    pub fn new() -> Self {
        Self
    }
}

pub async fn limit(State(_state): State<AppState>, request: Request, next: Next) -> Response {
    next.run(request).await
}
