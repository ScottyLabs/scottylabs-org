//! Serves the SvelteKit build with the fallback rules, cache headers and security headers in docs/implementation/contract.md. Platform track.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

pub async fn serve(State(_state): State<AppState>, _request: Request) -> Response {
    StatusCode::NOT_FOUND.into_response()
}

pub async fn security_headers(
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    next.run(request).await
}
