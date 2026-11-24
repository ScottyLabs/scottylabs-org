//! Uploads (POST /api/media) and serving (GET /media/{id}). Platform track.

use axum::Router;
use axum::extract::DefaultBodyLimit;

use super::UPLOAD_BODY_LIMIT;
use crate::state::AppState;

/// Mounted under /api with the upload body limit.
pub fn upload_router() -> Router<AppState> {
    Router::new().layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT))
}

/// Mounted at the root: GET /media/{id}.
pub fn serve_router() -> Router<AppState> {
    Router::new()
}
