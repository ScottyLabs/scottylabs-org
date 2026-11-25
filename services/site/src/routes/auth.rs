//! Signing in and out: Keycloak through the Ricochet relay, dev sign-in as a seeded persona, and logout.

use axum::extract::{RawQuery, State};
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cookie::time::Duration as CookieDuration;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::audit;
use crate::auth::normalize_andrew_id;
use crate::auth::oidc::{Identity, OidcProvider, RelayState, SignInError, clip, decode_state};
use crate::auth::session::{self, NewSession, secrets_match};
use crate::config::{Config, DevLogin};
use crate::error::{AppError, AppResult};
use crate::models::AuthMode;
use crate::routes::common::no_store;
use crate::seed::people::{PERSONAS, persona};
use crate::state::AppState;

/// A sign-in must come back within this long.
const ATTEMPT_MAX_AGE_SECONDS: i64 = 600;
const SIGN_IN_PAGE: &str = "/portal/sign-in/";
const PORTAL_HOME: &str = "/portal/";
const MAX_RETURN_TO_LEN: usize = 512;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/config", get(auth_config))
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/dev-login", post(dev_login))
        .route("/auth/logout", post(logout))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthConfigResponse {
    oidc: bool,
    dev_login: bool,
    dev_login_needs_key: bool,
    dev_personas: Vec<PersonaDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersonaDto {
    andrew_id: &'static str,
    name: &'static str,
    label: &'static str,
}

/// Dev sign-in needs the switch and the seeded personas it signs in as.
fn dev_login_available(config: &Config) -> bool {
    config.dev_login != DevLogin::Off && config.seed_demo
}

async fn auth_config(State(state): State<AppState>) -> Response {
    let dev_login = dev_login_available(&state.config);
    let body = AuthConfigResponse {
        oidc: state.oidc.is_some(),
        dev_login,
        dev_login_needs_key: dev_login && matches!(state.config.dev_login, DevLogin::Keyed(_)),
        dev_personas: if dev_login {
            PERSONAS
                .iter()
                .map(|p| PersonaDto {
                    andrew_id: p.andrew_id,
                    name: p.name,
                    label: p.label,
                })
                .collect()
        } else {
            Vec::new()
        },
    };
    no_store(Json(body).into_response())
}

/// Keeps a sign-in's destination inside the portal. Anything else becomes the portal home.
pub fn safe_return_to(raw: Option<&str>) -> String {
    let Some(path) = raw.map(str::trim) else {
        return PORTAL_HOME.into();
    };
    let in_portal = path == "/portal"
        || ["/portal/", "/portal?", "/portal#"]
            .iter()
            .any(|prefix| path.starts_with(prefix));
    let safe = in_portal
        && path.len() <= MAX_RETURN_TO_LEN
        && !path.contains("//")
        && !path.contains('\\')
        && !path.split(['/', '?', '#']).any(|segment| segment == "..")
        && path.chars().all(|c| c.is_ascii_graphic());
    if safe {
        path.into()
    } else {
        PORTAL_HOME.into()
    }
}

fn query_param(raw: Option<&str>, name: &str) -> Option<String> {
    url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

fn redirect(location: &str) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(LOCATION, value);
    }
    no_store(response)
}

fn sign_in_error(code: &str) -> Response {
    redirect(&format!("{SIGN_IN_PAGE}?error={code}"))
}

/// Binds a sign-in to the browser that started it, so nobody can finish their own sign-in in someone else's browser.
fn attempt_cookie_name(config: &Config) -> &'static str {
    if config.is_https() {
        "__Host-sl_signin"
    } else {
        "sl_signin"
    }
}

fn callback_url(config: &Config) -> String {
    let mut url = config.app_url.clone();
    url.set_path("/api/auth/callback");
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

async fn login(State(state): State<AppState>, RawQuery(query): RawQuery) -> Response {
    let return_to = safe_return_to(query_param(query.as_deref(), "returnTo").as_deref());
    let Some(oidc) = &state.oidc else {
        return sign_in_error("not_configured");
    };
    let authorization = match oidc.authorize(&callback_url(&state.config)).await {
        Ok(authorization) => authorization,
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "could not start a Keycloak sign-in");
            return sign_in_error("failed");
        }
    };
    // Abandoned attempts go away here as well as at sign-in, so they cannot pile up.
    let stored = sqlx::query(
        "WITH stale AS (
             DELETE FROM auth_attempts WHERE created_at <= now() - make_interval(secs => $5)
         )
         INSERT INTO auth_attempts (csrf, pkce_verifier, nonce, return_to) VALUES ($1, $2, $3, $4)",
    )
    .bind(&authorization.csrf)
    .bind(&authorization.pkce_verifier)
    .bind(&authorization.nonce)
    .bind(&return_to)
    .bind(ATTEMPT_MAX_AGE_SECONDS as f64)
    .execute(&state.db)
    .await;
    if let Err(error) = stored {
        tracing::error!(error = %error, "could not store a sign-in attempt");
        return sign_in_error("failed");
    }
    let mut response = redirect(authorization.url.as_str());
    session::append_cookie(
        response.headers_mut(),
        &session::build_cookie(
            &state.config,
            attempt_cookie_name(&state.config),
            &authorization.csrf,
            CookieDuration::seconds(ATTEMPT_MAX_AGE_SECONDS),
        ),
    );
    response
}

async fn callback(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let mut response = match &state.oidc {
        None => sign_in_error("not_configured"),
        Some(oidc) => match finish_sign_in(&state, oidc, query.as_deref(), &headers).await {
            Ok((return_to, session)) => {
                let mut response = redirect(&return_to);
                session::append_cookie(
                    response.headers_mut(),
                    &session::session_cookie(&state.config, &session.token),
                );
                response
            }
            Err(error) => {
                error.log();
                sign_in_error(error.code())
            }
        },
    };
    session::append_cookie(
        response.headers_mut(),
        &session::build_cookie(
            &state.config,
            attempt_cookie_name(&state.config),
            "",
            CookieDuration::ZERO,
        ),
    );
    response
}

#[derive(FromRow)]
struct Attempt {
    pkce_verifier: String,
    nonce: String,
    return_to: String,
    fresh: bool,
}

/// Uses up the stored attempt for `csrf`. Each attempt works once.
async fn take_attempt(state: &AppState, csrf: &str) -> AppResult<Option<Attempt>> {
    let attempt = sqlx::query_as::<_, Attempt>(
        "DELETE FROM auth_attempts WHERE csrf = $1
         RETURNING pkce_verifier, nonce, return_to,
                   created_at > now() - make_interval(secs => $2) AS fresh",
    )
    .bind(csrf)
    .bind(ATTEMPT_MAX_AGE_SECONDS as f64)
    .fetch_optional(&state.db)
    .await?;
    Ok(attempt)
}

/// Uses up the attempt behind a refused callback, so its `state` cannot be tried again.
async fn discard_attempt(state: &AppState, relay: Option<&RelayState>) -> Result<(), SignInError> {
    if let Some(relay) = relay {
        take_attempt(state, &relay.csrf).await.map_err(internal)?;
    }
    Ok(())
}

/// The callback's checks, in order. Returns where to send the person and their new session.
async fn finish_sign_in(
    state: &AppState,
    oidc: &OidcProvider,
    query: Option<&str>,
    headers: &HeaderMap,
) -> Result<(String, NewSession), SignInError> {
    let relay = query_param(query, "state")
        .as_deref()
        .and_then(decode_state);

    // RFC 9207: Keycloak names itself in `iss`, on errors too, and Ricochet passes it through. A callback without `iss` is still accepted.
    if let Some(iss) = query_param(query, "iss")
        && iss != oidc.issuer()
    {
        discard_attempt(state, relay.as_ref()).await?;
        return Err(SignInError::Failed(anyhow::anyhow!(
            "the callback names the issuer {:?}, not {:?}",
            clip(&iss),
            oidc.issuer()
        )));
    }

    if let Some(error) = query_param(query, "error") {
        discard_attempt(state, relay.as_ref()).await?;
        return Err(SignInError::provider(
            &error,
            query_param(query, "error_description").as_deref(),
        ));
    }

    let relay = relay.ok_or_else(|| failed("the callback has no valid state"))?;
    match session::read_cookie(headers, attempt_cookie_name(&state.config)) {
        None => {
            return Err(SignInError::Expired(
                "the browser came back without the sign-in cookie",
            ));
        }
        Some(cookie) if !secrets_match(&cookie, &relay.csrf) => {
            return Err(failed("the state does not belong to this browser"));
        }
        Some(_) => {}
    }
    let attempt = take_attempt(state, &relay.csrf)
        .await
        .map_err(internal)?
        .ok_or(SignInError::Expired(
            "the sign-in attempt is unknown or was already used",
        ))?;
    if !attempt.fresh {
        return Err(SignInError::Expired(
            "the sign-in attempt is older than ten minutes",
        ));
    }
    let code = query_param(query, "code").ok_or_else(|| failed("the callback has no code"))?;

    let identity = oidc
        .finish(code, attempt.pkce_verifier, attempt.nonce)
        .await?;
    let session = sign_in(state, oidc, &identity).await?;
    Ok((safe_return_to(Some(&attempt.return_to)), session))
}

fn failed(reason: &str) -> SignInError {
    SignInError::Failed(anyhow::anyhow!(reason.to_owned()))
}

fn internal(error: AppError) -> SignInError {
    SignInError::Failed(anyhow::anyhow!(error.to_string()))
}

/// Records the person, applies the admin group, and starts their session, all in one transaction.
async fn sign_in(
    state: &AppState,
    oidc: &OidcProvider,
    identity: &Identity,
) -> Result<NewSession, SignInError> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(AppError::from)
        .map_err(internal)?;
    let user_id = upsert_user(&mut tx, identity).await?;
    sync_admin_group(
        &mut tx,
        user_id,
        identity,
        &oidc.settings.project_admin_group,
    )
    .await
    .map_err(internal)?;
    let session = session::create(&mut tx, user_id, AuthMode::Oidc)
        .await
        .map_err(internal)?;
    tx.commit()
        .await
        .map_err(AppError::from)
        .map_err(internal)?;
    Ok(session)
}

/// Finds the person by Keycloak subject, then by Andrew ID (people can be added before they first sign in), else creates them. Name and email are refreshed from the token.
pub async fn upsert_user(
    conn: &mut PgConnection,
    identity: &Identity,
) -> Result<Uuid, SignInError> {
    let by_subject: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, andrew_id FROM users WHERE keycloak_sub = $1 FOR UPDATE")
            .bind(&identity.subject)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| internal(e.into()))?;

    if let Some((id, andrew_id)) = by_subject {
        sqlx::query(
            "UPDATE users SET name = coalesce($2, name), email = coalesce($3, email) WHERE id = $1",
        )
        .bind(id)
        .bind(&identity.name)
        .bind(&identity.email)
        .execute(&mut *conn)
        .await
        .map_err(|e| internal(e.into()))?;
        if andrew_id != identity.andrew_id {
            // Follow a renamed account unless someone else already has that Andrew ID.
            sqlx::query(
                "UPDATE users SET andrew_id = $2 WHERE id = $1
                 AND NOT EXISTS (SELECT 1 FROM users WHERE andrew_id = $2)",
            )
            .bind(id)
            .bind(&identity.andrew_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| internal(e.into()))?;
        }
        return Ok(id);
    }

    let by_andrew_id: Option<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT id, keycloak_sub FROM users WHERE andrew_id = $1 FOR UPDATE")
            .bind(&identity.andrew_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| internal(e.into()))?;

    match by_andrew_id {
        Some((_, Some(_))) => Err(failed(
            "this Andrew ID is already linked to a different Keycloak account",
        )),
        Some((id, None)) => {
            sqlx::query(
                "UPDATE users SET keycloak_sub = $2, name = coalesce($3, name), email = coalesce($4, email)
                 WHERE id = $1",
            )
            .bind(id)
            .bind(&identity.subject)
            .bind(&identity.name)
            .bind(&identity.email)
            .execute(&mut *conn)
            .await
            .map_err(|e| internal(e.into()))?;
            Ok(id)
        }
        None => {
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO users (id, keycloak_sub, andrew_id, name, email) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(&identity.subject)
            .bind(&identity.andrew_id)
            .bind(identity.name.as_deref().unwrap_or_default())
            .bind(&identity.email)
            .execute(&mut *conn)
            .await
            .map_err(|e| internal(e.into()))?;
            Ok(id)
        }
    }
}

fn same_group(a: &str, b: &str) -> bool {
    a.trim_start_matches('/') == b.trim_start_matches('/')
}

/// Members of `PROJECT_ADMIN_GROUP` are admins. An admin role that came only from the group goes away when the token shows they left it (any `groups` list without the group, including an empty one), unless they are the last admin. Tokens without a `groups` claim change nothing.
pub async fn sync_admin_group(
    conn: &mut PgConnection,
    user_id: Uuid,
    identity: &Identity,
    admin_group: &str,
) -> AppResult<()> {
    let Some(groups) = &identity.groups else {
        return Ok(());
    };
    let name = identity.name.as_deref().unwrap_or(&identity.andrew_id);
    if groups.iter().any(|group| same_group(group, admin_group)) {
        let granted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, source)
             SELECT $1, $2, 'admin', NULL, true, 'keycloak_group'
             WHERE NOT EXISTS (SELECT 1 FROM role_assignments WHERE user_id = $2 AND role = 'admin')
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await?;
        if granted.is_some() {
            audit::record(
                conn,
                audit::Entry {
                    actor: None,
                    action: "role.add",
                    subject_type: audit::subject::PERSON,
                    subject_id: Some(user_id.to_string()),
                    committee: None,
                    summary: format!("{name} became an admin through the Keycloak admin group"),
                },
            )
            .await?;
        }
        return Ok(());
    }

    let admins: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, user_id, source FROM role_assignments WHERE role = 'admin' ORDER BY id FOR UPDATE",
    )
    .fetch_all(&mut *conn)
    .await?;
    let from_group = admins
        .iter()
        .find(|(_, owner, source)| *owner == user_id && source == "keycloak_group");
    if let Some((assignment_id, _, _)) = from_group
        && admins.len() > 1
    {
        sqlx::query("DELETE FROM role_assignments WHERE id = $1")
            .bind(assignment_id)
            .execute(&mut *conn)
            .await?;
        audit::record(
            conn,
            audit::Entry {
                actor: None,
                action: "role.remove",
                subject_type: audit::subject::PERSON,
                subject_id: Some(user_id.to_string()),
                committee: None,
                summary: format!(
                    "{name} is no longer an admin after leaving the Keycloak admin group"
                ),
            },
        )
        .await?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevLoginInput {
    andrew_id: String,
    #[serde(default)]
    key: Option<String>,
}

async fn dev_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    crate::json::Json(input): crate::json::Json<DevLoginInput>,
) -> AppResult<Response> {
    if !dev_login_available(&state.config) {
        return Err(AppError::Forbidden(
            "Dev sign-in is turned off on this site. Continue with ScottyLabs instead.".into(),
        ));
    }
    if let DevLogin::Keyed(expected) = &state.config.dev_login
        && !secrets_match(input.key.as_deref().unwrap_or_default(), expected)
    {
        return Err(AppError::Forbidden(
            "That preview key is not right. Ask the UIA team for the current key.".into(),
        ));
    }
    let not_a_persona = || AppError::field("andrewId", "Choose one of the people listed.");
    let andrew_id = normalize_andrew_id(&input.andrew_id).ok_or_else(not_a_persona)?;
    persona(&andrew_id).ok_or_else(not_a_persona)?;
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE andrew_id = $1")
        .bind(&andrew_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(not_a_persona)?;

    let mut tx = state.db.begin().await?;
    if let Some(old) = session::token_from_headers(&headers, &state.config) {
        session::delete_by_token(&mut *tx, &old).await?;
    }
    let session = session::create(&mut tx, user_id, AuthMode::Dev).await?;
    tx.commit().await?;

    let mut response = no_store(StatusCode::NO_CONTENT.into_response());
    session::append_cookie(
        response.headers_mut(),
        &session::session_cookie(&state.config, &session.token),
    );
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = session::token_from_headers(&headers, &state.config) {
        session::delete_by_token(&state.db, &token).await?;
    }
    let mut response = no_store(StatusCode::NO_CONTENT.into_response());
    session::append_cookie(
        response.headers_mut(),
        &session::clear_session_cookie(&state.config),
    );
    Ok(response)
}

#[cfg(test)]
mod tests;
