//! Keycloak-backed sessions are confirmed with Keycloak again while they last: at most every 15 minutes, a request on such a session starts a refresh-token grant in the background. A refused refresh (`invalid_grant`: the account is disabled, the person signed out of Keycloak, or Keycloak's own session ended) ends the portal session at once, and the new ID token's groups add or remove the admin role from `PROJECT_ADMIN_GROUP`. When Keycloak cannot be reached the session is kept, but after 24 hours without a confirmation an admin role that came only from the group stops counting (see `extract::load_roles`) until a check succeeds. Dev sessions are never checked.

use sqlx::FromRow;
use uuid::Uuid;

use super::groups::sync_admin_group;
use super::oidc::{OidcProvider, RefreshError, clip};
use crate::audit;
use crate::error::AppResult;
use crate::state::AppState;

/// A Keycloak-backed session is checked again once this long has passed since the last confirmation.
pub const INTERVAL_MINUTES: i64 = 15;
/// An admin role from the Keycloak group counts for this long after the last confirmation.
pub const GROUP_ADMIN_GRACE_HOURS: i64 = 24;
/// While a check runs, no other process starts one for the same session for this long.
const RUNNING_SECONDS: f64 = 60.0;
/// After Keycloak could not be reached, the next check waits this long.
const RETRY_AFTER_FAILURE_SECONDS: f64 = 300.0;

/// What one check of a session did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Not due, already being checked, or not a Keycloak session with a refresh token.
    NotDue,
    /// Keycloak confirmed it; the admin role follows the new groups.
    Confirmed,
    /// Keycloak refused it, so the session ended.
    Ended,
    /// Keycloak could not be asked or gave no usable answer. The session is kept and asked again later.
    Unconfirmed,
}

/// Checks a session in the background, unless this process is already checking it.
pub fn spawn(state: &AppState, session_id: Uuid) {
    let Some(oidc) = state.oidc.clone() else {
        return;
    };
    let Some(guard) = oidc.begin_check(session_id) else {
        return;
    };
    let state = state.clone();
    tokio::spawn(async move {
        let _guard = guard;
        if let Err(error) = check(&state, &oidc, session_id).await {
            tracing::warn!(session = %session_id, error = ?error, "could not check a session with Keycloak");
        }
    });
}

#[derive(FromRow)]
struct Due {
    user_id: Uuid,
    refresh_token: String,
    keycloak_sub: Option<String>,
    andrew_id: String,
    name: String,
}

impl Due {
    fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.andrew_id
        } else {
            &self.name
        }
    }
}

/// Checks one session with Keycloak if it is due. The database decides who checks it, so two processes never use the same refresh token at once.
pub async fn check(state: &AppState, oidc: &OidcProvider, session_id: Uuid) -> AppResult<Outcome> {
    let due = sqlx::query_as::<_, Due>(
        "UPDATE sessions s SET validation_retry_at = now() + make_interval(secs => $2)
         FROM users u
         WHERE s.id = $1 AND u.id = s.user_id
           AND s.auth_mode = 'oidc' AND s.refresh_token IS NOT NULL AND s.expires_at > now()
           AND coalesce(s.validated_at, s.created_at) <= now() - make_interval(mins => $3)
           AND (s.validation_retry_at IS NULL OR s.validation_retry_at <= now())
         RETURNING s.user_id, s.refresh_token, u.keycloak_sub, u.andrew_id, u.name",
    )
    .bind(session_id)
    .bind(RUNNING_SECONDS)
    .bind(i32::try_from(INTERVAL_MINUTES).unwrap_or(15))
    .fetch_optional(&state.db)
    .await?;
    let Some(due) = due else {
        return Ok(Outcome::NotDue);
    };
    match oidc.refresh(&due.refresh_token).await {
        Ok(refreshed) if due.keycloak_sub.as_deref() == Some(refreshed.subject.as_str()) => {
            let mut tx = state.db.begin().await?;
            let kept = sqlx::query(
                "UPDATE sessions SET refresh_token = coalesce($2, refresh_token), id_token = $3,
                     validated_at = now(), validation_retry_at = NULL
                 WHERE id = $1",
            )
            .bind(session_id)
            .bind(&refreshed.refresh_token)
            .bind(&refreshed.id_token)
            .execute(&mut *tx)
            .await?;
            if kept.rows_affected() == 0 {
                // Signed out while Keycloak answered.
                return Ok(Outcome::Ended);
            }
            sync_admin_group(
                &mut tx,
                due.user_id,
                due.display_name(),
                refreshed.groups.as_deref(),
                &oidc.settings.project_admin_group,
            )
            .await?;
            tx.commit().await?;
            Ok(Outcome::Confirmed)
        }
        Ok(_) => {
            end(
                state,
                session_id,
                &due,
                "the new token names another Keycloak account",
            )
            .await
        }
        Err(RefreshError::Rejected { description }) => {
            let reason = description.unwrap_or_else(|| "invalid_grant".into());
            end(state, session_id, &due, &clip(&reason)).await
        }
        Err(RefreshError::Unavailable(cause) | RefreshError::Failed(cause)) => {
            sqlx::query(
                "UPDATE sessions SET validation_retry_at = now() + make_interval(secs => $2)
                 WHERE id = $1",
            )
            .bind(session_id)
            .bind(RETRY_AFTER_FAILURE_SECONDS)
            .execute(&state.db)
            .await?;
            tracing::warn!(
                session = %session_id,
                error = %format!("{cause:#}"),
                "could not confirm a session with Keycloak; keeping it for now"
            );
            Ok(Outcome::Unconfirmed)
        }
    }
}

async fn end(state: &AppState, session_id: Uuid, due: &Due, reason: &str) -> AppResult<Outcome> {
    let mut tx = state.db.begin().await?;
    let gone = sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    if gone.rows_affected() > 0 {
        audit::record(
            &mut tx,
            audit::Entry {
                actor: None,
                action: "auth.session_end",
                subject_type: audit::subject::PERSON,
                subject_id: Some(due.user_id.to_string()),
                committee: None,
                summary: format!(
                    "Ended a session of {} because ScottyLabs sign-in no longer accepts it",
                    due.display_name()
                ),
            },
        )
        .await?;
    }
    tx.commit().await?;
    tracing::info!(session = %session_id, reason, "ended a session that Keycloak no longer accepts");
    Ok(Outcome::Ended)
}
