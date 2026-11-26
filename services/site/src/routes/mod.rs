//! The router: /api, /media and the static site, with shared middleware.

mod activity;
mod announcements;
mod approvals;
mod auth;
mod committee_pages;
pub(crate) mod common;
mod me;
mod media;
mod overview;
mod people;
mod projects;
#[cfg(test)]
mod resilience_tests;

use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::RETRY_AFTER;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::trace::{DefaultOnFailure, TraceLayer};
use tracing::Level;

use crate::auth::csrf::full_path;
use crate::db::Phase;
use crate::error::{AppError, RETRY_AFTER_SECONDS};
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
        .method_not_allowed_fallback(api_method_not_allowed)
        .fallback(api_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::csrf::require_csrf,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::ratelimit::limit,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_database,
        ));

    Router::new()
        .nest("/api", api)
        .merge(media::serve_router())
        .fallback(crate::static_files::serve)
        // Inside the copies of public answers, so a request stopped for taking too long still gets one.
        .layer(middleware::from_fn(crate::server::limit_duration))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::last_good::keep,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::host_redirect::canonical_host,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::static_files::security_headers,
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(request_span)
                // Internal errors already log their cause; a second line per 500 only adds noise.
                .on_failure(DefaultOnFailure::new().level(Level::DEBUG)),
        )
        .with_state(state)
}

/// Every log line written while handling a request names its method and path. Not the query string, which can hold a sign-in code.
fn request_span(request: &axum::http::Request<axum::body::Body>) -> tracing::Span {
    tracing::info_span!(
        "request",
        method = %request.method(),
        path = %request.uri().path(),
    )
}

/// How long the health check waits for the database, under the 2 seconds kennel's probe waits for an answer.
const HEALTH_QUERY_LIMIT: Duration = Duration::from_millis(1500);

/// Readiness, for kennel's deploy check and monitors: `200 ok` once the database is migrated and seeded and answers, otherwise `503` with `starting` (the first attempt is still running) or `unavailable`, and `Retry-After`. It never says why; the log does. The static site is served either way.
async fn health(State(state): State<AppState>) -> Response {
    let phase = state.database.phase();
    let answers = phase == Phase::Ready
        && matches!(
            tokio::time::timeout(
                HEALTH_QUERY_LIMIT,
                sqlx::query("SELECT 1").execute(&state.db)
            )
            .await,
            Ok(Ok(_))
        );
    if answers {
        return (StatusCode::OK, "ok").into_response();
    }
    let status = if phase == Phase::Starting {
        "starting"
    } else {
        "unavailable"
    };
    let mut response = (StatusCode::SERVICE_UNAVAILABLE, status).into_response();
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from(RETRY_AFTER_SECONDS));
    response
}

/// Until the database is migrated and seeded, every API call but the health check answers 503 `unavailable` with `Retry-After`.
async fn require_database(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if state.database.is_ready() || full_path(&request) == "/api/health" {
        return next.run(request).await;
    }
    AppError::database_unavailable().into_response()
}

async fn api_not_found() -> AppError {
    AppError::NotFound("There is no API endpoint at that address.".into())
}

/// The route exists but not for this method. axum adds the `Allow` header.
async fn api_method_not_allowed() -> AppError {
    AppError::MethodNotAllowed
}
