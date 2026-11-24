//! CSRF token and Origin checks for writes under /api.
//!
//! Every POST, PUT, PATCH and DELETE must come from this site's origin when the browser says where it came from, and must carry the session's CSRF token in `X-CSRF-Token` whenever a valid session cookie is sent. Dev login is exempt from the token because it runs before a session exists; it still gets the Origin check, and its JSON body cannot be sent by a cross-site form.

use axum::extract::{OriginalUri, Request, State};
use axum::http::header::ORIGIN;
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::extract::current_viewer;
use super::session::secrets_match;
use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;

pub const CSRF_HEADER: &str = "x-csrf-token";
const DEV_LOGIN_PATH: &str = "/api/auth/dev-login";

pub async fn require_csrf(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !is_write(request.method()) {
        return next.run(request).await;
    }
    match check(&state, request).await {
        Ok(request) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

pub fn is_write(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// The full request path, also inside routers nested under /api, which see it with the prefix stripped.
pub fn full_path(request: &Request) -> &str {
    request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path(), |original| original.0.path())
}

async fn check(state: &AppState, request: Request) -> Result<Request, AppError> {
    check_origin(request.headers(), &state.config)?;
    if full_path(&request) == DEV_LOGIN_PATH {
        return Ok(request);
    }
    let (mut parts, body) = request.into_parts();
    if let Some(viewer) = current_viewer(&mut parts.extensions, &parts.headers, state).await? {
        let sent = parts
            .headers
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !secrets_match(sent, &viewer.csrf_token) {
            tracing::warn!(session = %viewer.session_id, "rejected a write with a missing or wrong CSRF token");
            return Err(AppError::Forbidden(
                "Your session check failed. Reload the page and try again.".into(),
            ));
        }
    }
    Ok(Request::from_parts(parts, body))
}

/// Rejects requests whose `Origin` header names another site. Requests without one pass.
fn check_origin(headers: &HeaderMap, config: &Config) -> Result<(), AppError> {
    let Some(origin) = headers.get(ORIGIN) else {
        return Ok(());
    };
    if origin.as_bytes() == config.origin().as_bytes() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "This request came from another site, so it was blocked. Reload the page and try again."
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderValue, StatusCode};
    use serde_json::json;
    use sqlx::PgPool;

    use super::*;
    use crate::models::{Committee, Role};
    use crate::testing::{self, Req};

    #[test]
    fn origin_must_match_app_url() {
        let config = testing::config();
        let mut headers = HeaderMap::new();
        assert!(check_origin(&headers, &config).is_ok());
        headers.insert(ORIGIN, HeaderValue::from_static("http://localhost:5173"));
        assert!(check_origin(&headers, &config).is_ok());
        for bad in [
            "https://localhost:5173",
            "http://localhost:5174",
            "http://evil.example",
            "null",
        ] {
            headers.insert(ORIGIN, HeaderValue::from_str(bad).unwrap());
            assert!(check_origin(&headers, &config).is_err(), "{bad}");
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn writes_need_the_session_token(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "csrfadmin", &[(Role::Admin, None)]).await;

        let missing = Req::post("/api/auth/logout")
            .cookie(&admin)
            .send(&app)
            .await;
        assert_eq!(missing.status, StatusCode::FORBIDDEN);
        assert_eq!(missing.code(), "forbidden");

        let wrong = Req::post("/api/auth/logout")
            .cookie(&admin)
            .header(CSRF_HEADER, "not-the-token")
            .send(&app)
            .await;
        assert_eq!(wrong.status, StatusCode::FORBIDDEN);

        // The session survived both attempts.
        let me = Req::get("/api/me").user(&admin).send(&app).await;
        assert_eq!(me.status, StatusCode::OK);

        let ok = Req::post("/api/auth/logout").user(&admin).send(&app).await;
        assert_eq!(ok.status, StatusCode::NO_CONTENT);
        let after = Req::get("/api/me").user(&admin).send(&app).await;
        assert_eq!(after.status, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn writes_from_other_origins_are_blocked(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let director = testing::user(
            &db,
            "csrfdirector",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        let blocked = Req::post("/api/auth/logout")
            .user(&director)
            .header("origin", "https://evil.example")
            .send(&app)
            .await;
        assert_eq!(blocked.status, StatusCode::FORBIDDEN);
        assert!(
            blocked
                .message()
                .starts_with("This request came from another site")
        );
        let still_signed_in = Req::get("/api/me").user(&director).send(&app).await;
        assert_eq!(still_signed_in.status, StatusCode::OK);

        let same_site = Req::post("/api/auth/logout")
            .user(&director)
            .header("origin", "http://localhost:5173")
            .send(&app)
            .await;
        assert_eq!(same_site.status, StatusCode::NO_CONTENT);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn dev_login_needs_no_token_but_checks_origin(db: PgPool) {
        testing::seed_personas(&db).await;
        let app = testing::app(db.clone()).await;
        let body = json!({ "andrewId": "tkanz", "key": null });

        let cross_site = Req::post("/api/auth/dev-login")
            .header("origin", "https://evil.example")
            .json(body.clone())
            .send(&app)
            .await;
        assert_eq!(cross_site.status, StatusCode::FORBIDDEN);

        let ok = Req::post("/api/auth/dev-login")
            .header("origin", "http://localhost:5173")
            .json(body)
            .send(&app)
            .await;
        assert_eq!(ok.status, StatusCode::NO_CONTENT);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn reads_need_no_token(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "readadmin", &[(Role::Admin, None)]).await;
        let me = Req::get("/api/me").cookie(&admin).send(&app).await;
        assert_eq!(me.status, StatusCode::OK);
    }
}
