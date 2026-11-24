//! GET /api/activity: recent audit entries in the viewer's scope.

use axum::extract::{RawQuery, State};
use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::audit::{self, ActivityEntry};
use crate::auth::PortalViewer;
use crate::error::AppResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/activity", get(list))
}

#[derive(Serialize)]
struct ActivityResponse {
    entries: Vec<ActivityEntry>,
}

/// `?limit=` defaults to 10 and is capped at 50. Values that are not numbers use the default.
fn parse_limit(query: Option<&str>) -> i64 {
    url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
        .find(|(key, _)| key == "limit")
        .and_then(|(_, value)| value.trim().parse::<i64>().ok())
        .unwrap_or(audit::DEFAULT_ACTIVITY)
        .clamp(1, audit::MAX_ACTIVITY)
}

async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(query): RawQuery,
) -> AppResult<Response> {
    let entries = audit::recent(&state.db, &viewer, parse_limit(query.as_deref())).await?;
    let mut response = Json(ActivityResponse { entries }).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use super::*;
    use crate::audit::{Entry, record, subject};
    use crate::models::{Committee, Role};
    use crate::testing::{self, Req};

    #[test]
    fn limits_are_clamped() {
        assert_eq!(parse_limit(None), 10);
        assert_eq!(parse_limit(Some("limit=3")), 3);
        assert_eq!(parse_limit(Some("limit=500")), 50);
        assert_eq!(parse_limit(Some("limit=0")), 1);
        assert_eq!(parse_limit(Some("limit=abc")), 10);
    }

    async fn log(
        db: &PgPool,
        actor: Option<uuid::Uuid>,
        committee: Option<Committee>,
        summary: &str,
    ) {
        let mut conn = db.acquire().await.unwrap();
        record(
            &mut conn,
            Entry {
                actor,
                action: "test.action",
                subject_type: subject::COMMITTEE_PAGE,
                subject_id: committee.map(|c| c.slug().to_owned()),
                committee,
                summary: summary.into(),
            },
        )
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn activity_is_scoped_and_newest_first(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let admin = testing::user(&db, "actadmin", &[(Role::Admin, None)]).await;
        let events = testing::user(
            &db,
            "actevents",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        let lead = testing::user(&db, "actlead", &[(Role::Lead, Some(Committee::Tech))]).await;
        let member = testing::user(&db, "actmember", &[]).await;

        log(
            &db,
            Some(admin.id),
            Some(Committee::Events),
            "Published the Events page",
        )
        .await;
        log(
            &db,
            Some(admin.id),
            Some(Committee::Design),
            "Published the Design page",
        )
        .await;
        log(&db, Some(lead.id), None, "Did something of their own").await;
        log(&db, None, Some(Committee::Tech), "Published the Tech page").await;

        let all = Req::get("/api/activity").user(&admin).send(&app).await;
        assert_eq!(all.status, StatusCode::OK);
        let summaries = |res: &testing::Res| -> Vec<String> {
            res.json["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["summary"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(
            summaries(&all),
            [
                "Published the Tech page",
                "Did something of their own",
                "Published the Design page",
                "Published the Events page"
            ]
        );
        assert_eq!(all.json["entries"][3]["actor"]["andrewId"], "actadmin");
        assert_eq!(all.json["entries"][3]["href"], "/portal/committees/events/");
        assert!(all.json["entries"][0]["actor"].is_null());

        let director = Req::get("/api/activity").user(&events).send(&app).await;
        assert_eq!(summaries(&director), ["Published the Events page"]);

        let lead_view = Req::get("/api/activity").user(&lead).send(&app).await;
        assert_eq!(
            summaries(&lead_view),
            ["Published the Tech page", "Did something of their own"]
        );

        let limited = Req::get("/api/activity?limit=1")
            .user(&admin)
            .send(&app)
            .await;
        assert_eq!(summaries(&limited).len(), 1);

        let denied = Req::get("/api/activity").user(&member).send(&app).await;
        assert_eq!(denied.status, StatusCode::FORBIDDEN);
        let anonymous = Req::get("/api/activity").send(&app).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    }
}
