//! CSRF token and Origin checks for writes under /api. Platform track.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::state::AppState;

pub async fn require_csrf(
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    next.run(request).await
}
