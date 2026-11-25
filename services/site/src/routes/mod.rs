//! The router: /api, /media and the static site, with shared middleware.

mod activity;
mod announcements;
mod approvals;
mod auth;
mod committee_pages;
mod common;
mod me;
mod media;
mod overview;
mod people;
mod projects;

use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::get;
use tower_http::trace::TraceLayer;

use crate::error::AppError;
use crate::state::AppState;

pub use announcements::{
    apply_approval as apply_announcement_approval, apply_send_back as apply_announcement_send_back,
};
pub use people::spawn_governance_sync;
pub use projects::{
    apply_approval as apply_project_approval, apply_send_back as apply_project_send_back,
};

/// JSON bodies larger than this are rejected. Uploads have their own, larger limit.
pub const JSON_BODY_LIMIT: usize = 256 * 1024;
pub const UPLOAD_BODY_LIMIT: usize = 6 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .merge(auth::router())
        .merge(me::router())
        .merge(people::router())
        .merge(activity::router())
        .merge(announcements::router())
        .merge(projects::router())
        .merge(committee_pages::router())
        .merge(overview::router())
        .merge(approvals::router())
        .layer(DefaultBodyLimit::max(JSON_BODY_LIMIT))
        .merge(media::upload_router())
        .fallback(api_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::csrf::require_csrf,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::ratelimit::limit,
        ));

    Router::new()
        .nest("/api", api)
        .merge(media::serve_router())
        .fallback(crate::static_files::serve)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::static_files::security_headers,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map(|_| "ok")
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

async fn api_not_found() -> AppError {
    AppError::NotFound("There is no API endpoint at that address.".into())
}
