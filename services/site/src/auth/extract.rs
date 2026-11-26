//! Extractors for [`Viewer`] and [`PortalViewer`]. The viewer is loaded once per request: the CSRF middleware and every extractor share the result through request extensions.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{Extensions, HeaderMap};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::{Assignment, PortalViewer, Viewer};
use super::{revalidate, session};
use crate::config::DevLogin;
use crate::error::{AppError, AppResult};
use crate::models::AuthMode;
use crate::state::AppState;

/// The outcome of loading the viewer, cached for the rest of the request.
#[derive(Clone)]
struct ViewerSlot(Option<Viewer>);

/// The signed-in viewer for this request, or `None` without a live session.
pub async fn current_viewer(
    extensions: &mut Extensions,
    headers: &HeaderMap,
    state: &AppState,
) -> AppResult<Option<Viewer>> {
    if let Some(ViewerSlot(viewer)) = extensions.get::<ViewerSlot>() {
        return Ok(viewer.clone());
    }
    let viewer = load_viewer(state, headers).await?;
    extensions.insert(ViewerSlot(viewer.clone()));
    Ok(viewer)
}

#[derive(FromRow)]
struct SessionUser {
    session_id: Uuid,
    user_id: Uuid,
    auth_mode: AuthMode,
    last_seen_at: DateTime<Utc>,
    dev_key_fingerprint: Option<Vec<u8>>,
    /// Keycloak confirmed the session recently enough for an admin role from the Keycloak group to count.
    group_admin_trusted: bool,
    /// The session is due for a check with Keycloak.
    check_due: bool,
    andrew_id: String,
    name: String,
    email: Option<String>,
}

async fn load_viewer(state: &AppState, headers: &HeaderMap) -> AppResult<Option<Viewer>> {
    let Some(token) = session::token_from_headers(headers, &state.config) else {
        return Ok(None);
    };
    let found = sqlx::query_as::<_, SessionUser>(
        "SELECT s.id AS session_id, s.user_id, s.auth_mode, s.last_seen_at, s.dev_key_fingerprint,
                (s.auth_mode <> 'oidc'
                 OR coalesce(s.validated_at, s.created_at) > now() - make_interval(hours => $3))
                    AS group_admin_trusted,
                (s.auth_mode = 'oidc' AND s.refresh_token IS NOT NULL
                 AND coalesce(s.validated_at, s.created_at) <= now() - make_interval(mins => $4)
                 AND (s.validation_retry_at IS NULL OR s.validation_retry_at <= now()))
                    AS check_due,
                u.andrew_id, u.name, u.email
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1
           AND s.expires_at > now()
           AND s.last_seen_at > now() - make_interval(hours => $2)",
    )
    .bind(session::hash_token(&token))
    .bind(i32::try_from(session::IDLE_TIMEOUT_HOURS).unwrap_or(12))
    .bind(i32::try_from(revalidate::GROUP_ADMIN_GRACE_HOURS).unwrap_or(24))
    .bind(i32::try_from(revalidate::INTERVAL_MINUTES).unwrap_or(15))
    .fetch_optional(&state.db)
    .await?;
    let Some(found) = found else {
        return Ok(None);
    };
    if found.auth_mode == AuthMode::Dev
        && !dev_session_allowed(&state.config.dev_login, state.config.seed_demo, &found)
    {
        return Ok(None);
    }
    if found.check_due && state.oidc.is_some() {
        // In the background: this request goes on with what the database says now.
        revalidate::spawn(state, found.session_id);
    }

    let (roles, lead_projects) = tokio::try_join!(
        load_roles(state, found.user_id, found.group_admin_trusted),
        load_lead_projects(state, found.user_id)
    )?;

    if session::needs_touch(found.last_seen_at, Utc::now()) {
        session::touch(&state.db, found.session_id).await?;
    }

    Ok(Some(Viewer {
        user_id: found.user_id,
        andrew_id: found.andrew_id,
        name: found.name,
        email: found.email,
        session_id: found.session_id,
        csrf_token: session::csrf_token_for(&token),
        auth_mode: found.auth_mode,
        roles,
        lead_projects,
    }))
}

/// A dev session works only while dev sign-in is on, and on previews only with the key it was made with, so turning dev sign-in off or changing the key ends it.
fn dev_session_allowed(dev_login: &DevLogin, seed_demo: bool, session: &SessionUser) -> bool {
    if !seed_demo {
        return false;
    }
    match dev_login {
        DevLogin::Off => false,
        DevLogin::Open => true,
        DevLogin::Keyed(key) => session
            .dev_key_fingerprint
            .as_deref()
            .is_some_and(|stored| {
                session::secrets_match_bytes(stored, &session::dev_key_fingerprint(key))
            }),
    }
}

/// The person's roles. Without a recent confirmation from Keycloak, an admin role that came only from the Keycloak admin group does not count, so it lapses when Keycloak cannot say whether the person is still in the group.
async fn load_roles(
    state: &AppState,
    user_id: Uuid,
    group_admin_trusted: bool,
) -> AppResult<Vec<Assignment>> {
    let roles = sqlx::query_as::<_, Assignment>(
        "SELECT role, committee, can_post_ticker FROM role_assignments
         WHERE user_id = $1 AND ($2 OR NOT (role = 'admin' AND source = 'keycloak_group'))
         ORDER BY created_at, id",
    )
    .bind(user_id)
    .bind(group_admin_trusted)
    .fetch_all(&state.db)
    .await?;
    Ok(roles)
}

/// Projects the viewer may edit as a lead: listed as an editor, and still a lead of the owning committee.
async fn load_lead_projects(state: &AppState, user_id: Uuid) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT pe.project_id FROM project_editors pe
         JOIN projects p ON p.id = pe.project_id
         WHERE pe.user_id = $1
           AND EXISTS (
               SELECT 1 FROM role_assignments r
               WHERE r.user_id = pe.user_id AND r.role = 'lead' AND r.committee = p.committee
           )
         ORDER BY pe.project_id",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(ids)
}

impl FromRequestParts<AppState> for Viewer {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        current_viewer(&mut parts.extensions, &parts.headers, state)
            .await?
            .ok_or(AppError::Unauthenticated)
    }
}

impl FromRequestParts<AppState> for PortalViewer {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let viewer = Viewer::from_request_parts(parts, state).await?;
        if crate::permissions::can_use_portal(&viewer) {
            Ok(Self(viewer))
        } else {
            Err(AppError::Forbidden(
                "Your account has no role in the portal yet. Ask the director of your committee to add you.".into(),
            ))
        }
    }
}
