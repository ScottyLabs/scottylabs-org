//! GET /api/me: who is signed in, their roles and permissions, and the CSRF token for writes.

use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::auth::Viewer;
use crate::auth::roles::{RoleAssignmentDto, assignments_for};
use crate::error::AppResult;
use crate::models::{AuthMode, PersonRef};
use crate::permissions::{self, PermissionsDto};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeUser {
    #[serde(flatten)]
    person: PersonRef,
    email: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    user: MeUser,
    roles: Vec<RoleAssignmentDto>,
    permissions: PermissionsDto,
    csrf_token: String,
    auth_mode: AuthMode,
}

async fn me(State(state): State<AppState>, viewer: Viewer) -> AppResult<Response> {
    let roles = assignments_for(&state.db, &[viewer.user_id])
        .await?
        .remove(&viewer.user_id)
        .unwrap_or_default();
    let body = MeResponse {
        user: MeUser {
            person: viewer.person(),
            email: viewer.email.clone(),
        },
        roles,
        permissions: permissions::summary(&viewer),
        csrf_token: viewer.csrf_token.clone(),
        auth_mode: viewer.auth_mode,
    };
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::models::{Committee, Role};
    use crate::testing::{self, Req, UserSpec};

    #[sqlx::test(migrations = "./migrations")]
    async fn signed_out_is_unauthenticated(db: PgPool) {
        let app = testing::app(db).await;
        let res = Req::get("/api/me").send(&app).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
        assert_eq!(res.code(), "unauthenticated");
        assert_eq!(res.header("cache-control"), Some("no-store"));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn describes_a_lead_with_projects(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let nova = testing::project(&db, Committee::Events, "nova").await;
        let other = testing::project(&db, Committee::Tech, "cmu-courses").await;
        let director = testing::user(
            &db,
            "medirector",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        let lead = UserSpec::new("melead")
            .name("Sara Chokshi")
            .lead(Committee::Events, true)
            .appointed_by(director.id)
            .edits(nova)
            .edits(other)
            .create(&db)
            .await;

        let res = Req::get("/api/me").user(&lead).send(&app).await;
        assert_eq!(res.status, StatusCode::OK);
        let body = &res.json;
        assert_eq!(body["user"]["andrewId"], "melead");
        assert_eq!(body["user"]["name"], "Sara Chokshi");
        assert_eq!(body["user"]["initials"], "SC");
        assert_eq!(body["csrfToken"], lead.csrf.as_str());
        assert_eq!(body["authMode"], "dev");

        let role = &body["roles"][0];
        assert_eq!(role["role"], "lead");
        assert_eq!(role["committee"], "events");
        assert_eq!(role["canPostTicker"], true);
        assert_eq!(role["source"], "portal");
        assert_eq!(role["appointedBy"]["andrewId"], "medirector");
        // Only projects of the lead's own committee count.
        assert_eq!(role["projectIds"], serde_json::json!([nova]));

        let permissions = &body["permissions"];
        assert_eq!(permissions["canUsePortal"], true);
        assert_eq!(permissions["isAdmin"], false);
        assert_eq!(permissions["leadOf"], serde_json::json!(["events"]));
        assert_eq!(permissions["tickerSources"], serde_json::json!(["events"]));
        assert_eq!(permissions["tickerNeedsApproval"], true);
        assert_eq!(permissions["editableProjectIds"], serde_json::json!([nova]));
        assert_eq!(permissions["peopleScope"], serde_json::json!([]));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn admins_have_scope_all_and_members_have_nothing(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "meadmin", &[(Role::Admin, None)]).await;
        let member = testing::user(&db, "memember", &[]).await;

        let res = Req::get("/api/me").user(&admin).send(&app).await;
        assert_eq!(res.json["permissions"]["peopleScope"], "all");
        assert_eq!(res.json["permissions"]["isAdmin"], true);
        assert_eq!(res.json["roles"][0]["committee"], serde_json::Value::Null);
        assert_eq!(res.json["roles"][0]["projectIds"], serde_json::json!([]));

        let res = Req::get("/api/me").user(&member).send(&app).await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(res.json["roles"], serde_json::json!([]));
        assert_eq!(res.json["permissions"]["canUsePortal"], false);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn role_changes_apply_on_the_next_request(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let member = testing::user(&db, "mepromoted", &[]).await;
        let res = Req::get("/api/me").user(&member).send(&app).await;
        assert_eq!(res.json["permissions"]["canUsePortal"], false);

        // Same session, no new sign-in.
        testing::grant(&db, member.id, Role::Director, Some(Committee::Design)).await;
        let res = Req::get("/api/me").user(&member).send(&app).await;
        assert_eq!(res.json["permissions"]["canUsePortal"], true);
        assert_eq!(
            res.json["permissions"]["directorOf"],
            serde_json::json!(["design"])
        );
    }
}
