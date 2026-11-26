//! Signing in and out: Keycloak (through the Ricochet relay on previews and staging, straight back in production), dev sign-in as a seeded persona, and logout. Also two admin actions for the People page: ending all of a person's sessions, and resetting the link between their Andrew ID and a Keycloak account.
//!
//! Sign-ins, sign-outs, ended sessions and refused identities are written to the audit log with no actor and the person as the subject. Recent activity leaves these out (`audit::HIDDEN_FROM_ACTIVITY`), so nobody can bury other entries by signing in over and over. Failures that need no proof of who someone is, such as a cancelled sign-in, a refused code or a wrong preview key, are only logged: anyone can cause them, and the log only grows.

use axum::extract::{Path, RawQuery, State};
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use cookie::time::Duration as CookieDuration;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::audit;
use crate::auth::groups::sync_admin_group;
use crate::auth::oidc::{
    Identity, OidcProvider, RelayState, SignInError, SignedIn, clip, decode_state,
};
use crate::auth::session::{self, NewSession, SessionExtras, secrets_match};
use crate::auth::{PortalViewer, normalize_andrew_id};
use crate::config::{Config, DevLogin, sign_in_callback_url};
use crate::error::{AppError, AppResult};
use crate::models::AuthMode;
use crate::permissions;
use crate::routes::common::{no_store, parse_id};
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
        .route("/auth/dev-personas", post(dev_personas))
        .route("/auth/logout", post(logout))
        .route("/people/{id}/sessions", delete(end_sessions))
        .route("/people/{id}/keycloak-link", delete(reset_keycloak_link))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthConfigResponse {
    oidc: bool,
    dev_login: bool,
    dev_login_needs_key: bool,
    /// Empty on previews until the key is given (`POST /api/auth/dev-personas`), so the page does not list them to everyone.
    dev_personas: Vec<PersonaDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersonaDto {
    andrew_id: &'static str,
    name: &'static str,
    label: &'static str,
}

fn personas() -> Vec<PersonaDto> {
    PERSONAS
        .iter()
        .map(|p| PersonaDto {
            andrew_id: p.andrew_id,
            name: p.name,
            label: p.label,
        })
        .collect()
}

async fn auth_config(State(state): State<AppState>) -> Response {
    let dev_login = state.config.dev_login_available();
    let body = AuthConfigResponse {
        oidc: state.oidc.is_some(),
        dev_login,
        dev_login_needs_key: dev_login && matches!(state.config.dev_login, DevLogin::Keyed(_)),
        dev_personas: if dev_login && state.config.dev_login == DevLogin::Open {
            personas()
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

/// Back to the sign-in page with a reason it explains.
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
    sign_in_callback_url(&config.app_url)
}

async fn login(State(state): State<AppState>, RawQuery(query): RawQuery) -> Response {
    let return_to = safe_return_to(query_param(query.as_deref(), "returnTo").as_deref());
    let Some(oidc) = &state.oidc else {
        return sign_in_error("not_configured");
    };
    let authorization = match oidc.authorize(&callback_url(&state.config)).await {
        Ok(authorization) => authorization,
        Err(error) => {
            error.log();
            return sign_in_error(error.code());
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
                    &session::session_cookie(&state.config, &session),
                );
                response
            }
            Err(error) => {
                error.log();
                record_refusal(&state, &error).await;
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

/// Writes the audit entry for a refused identity: Keycloak vouched for the account, but it could not be matched to the Andrew ID it claims. Other failures are only logged (see the module comment). Never a code or token.
async fn record_refusal(state: &AppState, error: &SignInError) {
    let summary = match error {
        SignInError::NotAndrewId {
            reason,
            claimed: Some(andrew_id),
        } => format!(
            "Refused a ScottyLabs sign-in as {andrew_id}: the account could not be matched to that CMU Andrew ID ({reason})"
        ),
        SignInError::NotAndrewId {
            reason,
            claimed: None,
        } => format!("Refused a ScottyLabs sign-in: the account named no CMU Andrew ID ({reason})"),
        SignInError::LinkedElsewhere { andrew_id } => format!(
            "Refused a ScottyLabs sign-in as {andrew_id}: that Andrew ID is linked to a different ScottyLabs account"
        ),
        _ => return,
    };
    record_auth_event(state, "auth.identity_refused", None, None, summary).await;
}

/// Audits a sign-in event outside any other transaction. A failure to write it is logged, never shown.
async fn record_auth_event(
    state: &AppState,
    action: &str,
    actor: Option<Uuid>,
    subject: Option<Uuid>,
    summary: String,
) {
    let written = async {
        let mut conn = state.db.acquire().await?;
        audit::record(
            &mut conn,
            audit::Entry {
                actor,
                action,
                subject_type: audit::subject::PERSON,
                subject_id: subject.map(|id| id.to_string()),
                committee: None,
                summary,
            },
        )
        .await
    }
    .await;
    if let Err(error) = written {
        tracing::warn!(error = ?error, action, "could not write a sign-in audit entry");
    }
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

    // RFC 9207: Keycloak names itself in `iss`, on errors too, whether through Ricochet or straight back. A callback without `iss` is still accepted.
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

    let signed_in = oidc
        .finish(code, attempt.pkce_verifier, attempt.nonce)
        .await?;
    let previous = session::token_from_headers(headers, &state.config);
    let session = sign_in(state, oidc, signed_in, previous).await?;
    Ok((safe_return_to(Some(&attempt.return_to)), session))
}

fn failed(reason: &str) -> SignInError {
    SignInError::Failed(anyhow::anyhow!(reason.to_owned()))
}

fn internal(error: AppError) -> SignInError {
    SignInError::Failed(anyhow::anyhow!(error.to_string()))
}

fn display_name(identity: &Identity) -> &str {
    identity.name.as_deref().unwrap_or(&identity.andrew_id)
}

/// Records the person, applies the admin group and starts their session, all in one transaction. The session this browser had before ends, so a shared computer does not keep the last person's session alive on the server.
async fn sign_in(
    state: &AppState,
    oidc: &OidcProvider,
    signed_in: SignedIn,
    previous_session: Option<String>,
) -> Result<NewSession, SignInError> {
    let identity = &signed_in.identity;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(AppError::from)
        .map_err(internal)?;
    if let Some(token) = previous_session {
        session::delete_by_token(&mut *tx, &token)
            .await
            .map_err(internal)?;
    }
    let user_id = upsert_user(&mut tx, identity).await?;
    sync_admin_group(
        &mut tx,
        user_id,
        display_name(identity),
        identity.groups.as_deref(),
        &oidc.settings.project_admin_group,
    )
    .await
    .map_err(internal)?;
    let session = session::create_with(
        &mut tx,
        user_id,
        AuthMode::Oidc,
        SessionExtras {
            refresh_token: signed_in.refresh_token,
            id_token: Some(signed_in.id_token),
            dev_key_fingerprint: None,
        },
    )
    .await
    .map_err(internal)?;
    audit::record(
        &mut tx,
        audit::Entry {
            actor: None,
            action: "auth.sign_in",
            subject_type: audit::subject::PERSON,
            subject_id: Some(user_id.to_string()),
            committee: None,
            summary: format!("{} signed in with ScottyLabs", display_name(identity)),
        },
    )
    .await
    .map_err(internal)?;
    tx.commit()
        .await
        .map_err(AppError::from)
        .map_err(internal)?;
    Ok(session)
}

/// Finds the person by Keycloak subject, then by Andrew ID (people can be added before they first sign in), else creates them. Name and email are refreshed from the token. `identity.andrew_id` has already been proven (see `oidc::andrew_id_of`).
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
        Some((_, Some(_))) => Err(SignInError::LinkedElsewhere {
            andrew_id: identity.andrew_id.clone(),
        }),
        Some((id, None)) => {
            sqlx::query(
                "UPDATE users SET keycloak_sub = $2, name = coalesce($3, name), email = coalesce($4, email),
                     keycloak_linked_at = coalesce(keycloak_linked_at, now())
                 WHERE id = $1",
            )
            .bind(id)
            .bind(&identity.subject)
            .bind(&identity.name)
            .bind(&identity.email)
            .execute(&mut *conn)
            .await
            .map_err(|e| internal(e.into()))?;
            // Whoever links a person added before they signed in takes over their roles, so say so.
            audit::record(
                conn,
                audit::Entry {
                    actor: None,
                    action: "auth.link",
                    subject_type: audit::subject::PERSON,
                    subject_id: Some(id.to_string()),
                    committee: None,
                    summary: format!(
                        "Linked the ScottyLabs account of {} to {}, who was added before signing in",
                        display_name(identity),
                        identity.andrew_id
                    ),
                },
            )
            .await
            .map_err(internal)?;
            Ok(id)
        }
        None => {
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO users (id, keycloak_sub, andrew_id, name, email, keycloak_linked_at)
                 VALUES ($1, $2, $3, $4, $5, now())",
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevLoginInput {
    andrew_id: String,
    #[serde(default)]
    key: Option<String>,
}

/// Refuses when dev sign-in is off, or when the preview's key is wrong.
fn check_dev_login(config: &Config, key: Option<&str>) -> Result<(), DevRefusal> {
    if !config.dev_login_available() {
        return Err(DevRefusal::Off);
    }
    if let DevLogin::Keyed(expected) = &config.dev_login
        && !secrets_match(key.unwrap_or_default(), expected)
    {
        return Err(DevRefusal::WrongKey);
    }
    Ok(())
}

enum DevRefusal {
    Off,
    WrongKey,
}

impl From<DevRefusal> for AppError {
    fn from(refusal: DevRefusal) -> Self {
        match refusal {
            DevRefusal::Off => AppError::Forbidden(
                "Dev sign-in is turned off on this site. Continue with ScottyLabs instead.".into(),
            ),
            DevRefusal::WrongKey => AppError::Forbidden(
                "That preview key is not right. Ask the UIA team for the current key.".into(),
            ),
        }
    }
}

#[derive(FromRow)]
struct DevTarget {
    id: Uuid,
    name: String,
    linked: bool,
}

async fn dev_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    crate::json::Json(input): crate::json::Json<DevLoginInput>,
) -> AppResult<Response> {
    if let Err(refusal) = check_dev_login(&state.config, input.key.as_deref()) {
        if matches!(refusal, DevRefusal::WrongKey) {
            // Logged, not audited: anyone can send a wrong key.
            tracing::warn!("refused a dev sign-in with a wrong preview key");
        }
        return Err(refusal.into());
    }
    let not_a_persona = || AppError::field("andrewId", "Choose one of the people listed.");
    let andrew_id = normalize_andrew_id(&input.andrew_id).ok_or_else(not_a_persona)?;
    persona(&andrew_id).ok_or_else(not_a_persona)?;
    let target = sqlx::query_as::<_, DevTarget>(
        "SELECT id, name, (keycloak_sub IS NOT NULL OR keycloak_linked_at IS NOT NULL) AS linked
         FROM users WHERE andrew_id = $1",
    )
    .bind(&andrew_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(not_a_persona)?;
    // Away from localhost, never act as someone who signed in here with their real account, even after an admin reset that link: on previews the key is shared, and their actions would be recorded under their name.
    let keyed = matches!(state.config.dev_login, DevLogin::Keyed(_));
    if keyed && target.linked {
        record_auth_event(
            &state,
            "auth.sign_in_failed",
            None,
            Some(target.id),
            format!("A dev sign-in as {andrew_id} was refused: that person signs in with ScottyLabs here (linked_account)"),
        )
        .await;
        return Err(AppError::field(
            "andrewId",
            "That person has signed in here with their ScottyLabs account, so dev sign-in can't act as them. Choose someone else.",
        ));
    }
    let fingerprint = match &state.config.dev_login {
        DevLogin::Keyed(key) => Some(session::dev_key_fingerprint(key)),
        DevLogin::Open | DevLogin::Off => None,
    };

    let mut tx = state.db.begin().await?;
    if let Some(old) = session::token_from_headers(&headers, &state.config) {
        session::delete_by_token(&mut *tx, &old).await?;
    }
    let session = session::create_with(
        &mut tx,
        target.id,
        AuthMode::Dev,
        SessionExtras {
            dev_key_fingerprint: fingerprint,
            ..SessionExtras::default()
        },
    )
    .await?;
    audit::record(
        &mut tx,
        audit::Entry {
            actor: None,
            action: "auth.sign_in",
            subject_type: audit::subject::PERSON,
            subject_id: Some(target.id.to_string()),
            committee: None,
            summary: format!("{} signed in with dev sign-in", target.name),
        },
    )
    .await?;
    tx.commit().await?;

    let mut response = no_store(StatusCode::NO_CONTENT.into_response());
    session::append_cookie(
        response.headers_mut(),
        &session::session_cookie(&state.config, &session),
    );
    Ok(response)
}

#[derive(Deserialize)]
struct DevPersonasInput {
    #[serde(default)]
    key: Option<String>,
}

#[derive(Serialize)]
struct DevPersonasResponse {
    personas: Vec<PersonaDto>,
}

/// The people dev sign-in can act as. On previews it needs the key, so the list is not public.
async fn dev_personas(
    State(state): State<AppState>,
    crate::json::Json(input): crate::json::Json<DevPersonasInput>,
) -> AppResult<Response> {
    check_dev_login(&state.config, input.key.as_deref())?;
    Ok(no_store(
        Json(DevPersonasResponse {
            personas: personas(),
        })
        .into_response(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutResponse {
    end_session_url: String,
}

/// Ends this browser's session. When it came from Keycloak and Keycloak lists an end-session endpoint, answers 200 with `endSessionUrl`, where the browser can go to sign out of Keycloak too; otherwise 204.
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let mut end_session_url = None;
    if let Some(token) = session::token_from_headers(&headers, &state.config) {
        let mut tx = state.db.begin().await?;
        let ended = session::end_by_token(&mut *tx, &token).await?;
        if let Some(ended) = &ended {
            let shown = if ended.name.trim().is_empty() {
                &ended.andrew_id
            } else {
                &ended.name
            };
            audit::record(
                &mut tx,
                audit::Entry {
                    actor: None,
                    action: "auth.sign_out",
                    subject_type: audit::subject::PERSON,
                    subject_id: Some(ended.user_id.to_string()),
                    committee: None,
                    summary: format!("{shown} signed out"),
                },
            )
            .await?;
        }
        tx.commit().await?;
        if let (Some(ended), Some(oidc)) = (ended, &state.oidc)
            && ended.auth_mode == AuthMode::Oidc
            && let Some(id_token) = ended.id_token
        {
            end_session_url = oidc.end_session_url(&id_token).await;
        }
    }
    let mut response = match end_session_url {
        Some(end_session_url) => Json(LogoutResponse { end_session_url }).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    };
    response = no_store(response);
    session::append_cookie(
        response.headers_mut(),
        &session::clear_session_cookie(&state.config),
    );
    Ok(response)
}

#[derive(FromRow)]
struct Person {
    name: String,
    andrew_id: String,
    linked: bool,
}

impl Person {
    fn shown(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.andrew_id
        } else {
            &self.name
        }
    }
}

/// The person behind `id`, locked for the change, after checking the viewer is an admin.
async fn person_for_sign_in_change(
    conn: &mut PgConnection,
    viewer: &PortalViewer,
    id: &str,
) -> AppResult<(Uuid, Person)> {
    if !permissions::can_manage_sign_in(viewer) {
        return Err(AppError::Forbidden(
            "Only admins can sign people out or reset their sign-in.".into(),
        ));
    }
    let user_id = parse_id(id, "person")?;
    let person = sqlx::query_as::<_, Person>(
        "SELECT name, andrew_id, keycloak_sub IS NOT NULL AS linked FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("person"))?;
    Ok((user_id, person))
}

/// Clears the cookie too when admins ended their own sessions.
fn after_ending_sessions(
    state: &AppState,
    viewer: &PortalViewer,
    user_id: Uuid,
    ended: bool,
    mut response: Response,
) -> Response {
    if ended && viewer.user_id == user_id {
        session::append_cookie(
            response.headers_mut(),
            &session::clear_session_cookie(&state.config),
        );
    }
    no_store(response)
}

#[derive(Serialize)]
struct EndSessionsResponse {
    ended: u64,
}

/// `DELETE /api/people/:id/sessions`: signs a person out everywhere. Admins only. Their roles stay; an admin role from the Keycloak group comes back at their next sign-in while they are in the group.
async fn end_sessions(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let mut tx = state.db.begin().await?;
    let (user_id, person) = person_for_sign_in_change(&mut tx, &viewer, &id).await?;
    let ended = session::end_all_for_user(&mut *tx, user_id).await?;
    audit::record(
        &mut tx,
        audit::Entry {
            actor: Some(viewer.user_id),
            action: "auth.sessions_end",
            subject_type: audit::subject::PERSON,
            subject_id: Some(user_id.to_string()),
            committee: None,
            summary: format!("Signed {} out everywhere", person.shown()),
        },
    )
    .await?;
    tx.commit().await?;
    let response = Json(EndSessionsResponse { ended }).into_response();
    Ok(after_ending_sessions(
        &state,
        &viewer,
        user_id,
        ended > 0,
        response,
    ))
}

/// `DELETE /api/people/:id/keycloak-link`: forgets which Keycloak account a person signed in with, for when Keycloak gave them a new one (docs/implementation/sign-in-recovery.md). Admins only. Their sessions end, and their next sign-in links the account again, which still needs proof of the Andrew ID.
async fn reset_keycloak_link(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let mut tx = state.db.begin().await?;
    let (user_id, person) = person_for_sign_in_change(&mut tx, &viewer, &id).await?;
    if person.linked {
        // keycloak_linked_at stays, so dev sign-in on a preview cannot act as them afterwards.
        sqlx::query(
            "UPDATE users SET keycloak_sub = NULL, keycloak_linked_at = coalesce(keycloak_linked_at, now())
             WHERE id = $1",
        )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        session::end_all_for_user(&mut *tx, user_id).await?;
        audit::record(
            &mut tx,
            audit::Entry {
                actor: Some(viewer.user_id),
                action: "auth.link_reset",
                subject_type: audit::subject::PERSON,
                subject_id: Some(user_id.to_string()),
                committee: None,
                summary: format!(
                    "Reset the ScottyLabs sign-in link of {}; their next sign-in links it again",
                    person.shown()
                ),
            },
        )
        .await?;
    }
    tx.commit().await?;
    let response = StatusCode::NO_CONTENT.into_response();
    Ok(after_ending_sessions(
        &state,
        &viewer,
        user_id,
        person.linked,
        response,
    ))
}

#[cfg(test)]
mod tests;
