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
    use crate::testing::{self, Req, UserSpec};

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

    fn summaries(res: &testing::Res) -> Vec<String> {
        res.json["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["summary"].as_str().unwrap().to_owned())
            .collect()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn entries_never_name_what_the_viewer_may_not_open(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let director = UserSpec::new("evdir")
            .director(Committee::Events)
            .create(&db)
            .await;
        let sara = UserSpec::new("sara")
            .lead(Committee::Events, true)
            .create(&db)
            .await;
        let newcomer = UserSpec::new("newcomer")
            .lead(Committee::Events, false)
            .create(&db)
            .await;

        let expires = chrono::Utc::now() + chrono::TimeDelta::days(5);
        let sent = Req::post("/api/announcements")
            .user(&sara)
            .json(serde_json::json!({
                "kind": "event", "committee": "events",
                "message": "Private draft about the surprise speaker", "expiresAt": expires,
            }))
            .send(&app)
            .await;
        assert_eq!(sent.status, StatusCode::CREATED, "{}", sent.json);
        let added = Req::post("/api/projects")
            .user(&director)
            .json(serde_json::json!({ "name": "Unannounced sponsor hackathon", "committee": "events" }))
            .send(&app)
            .await;
        assert_eq!(added.status, StatusCode::CREATED, "{}", added.json);

        let waiting = "Sent \u{201c}Private draft about the surprise speaker\u{201d} for approval";
        let project = "Added the project Unannounced sponsor hackathon";
        let seen = |res: &testing::Res, summary: &str| summaries(res).iter().any(|s| s == summary);

        // Another lead of the committee sees neither, on the activity page or the overview.
        let other = Req::get("/api/activity").user(&newcomer).send(&app).await;
        assert!(
            !seen(&other, waiting) && !seen(&other, project),
            "{}",
            other.json
        );
        let overview = Req::get("/api/overview").user(&newcomer).send(&app).await;
        assert_eq!(overview.status, StatusCode::OK);
        let overview_summaries: Vec<&str> = overview.json["activity"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["summary"].as_str().unwrap())
            .collect();
        assert!(!overview_summaries.contains(&waiting) && !overview_summaries.contains(&project));

        // The author sees their own submission and the director sees both.
        let own = Req::get("/api/activity").user(&sara).send(&app).await;
        assert!(seen(&own, waiting) && !seen(&own, project));
        let full = Req::get("/api/activity").user(&director).send(&app).await;
        assert!(seen(&full, waiting) && seen(&full, project));

        // Once it is live, the announcement is for everyone to see.
        let approval: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM approvals WHERE status = 'waiting'")
                .fetch_one(&db)
                .await
                .unwrap();
        let approved = Req::post(&format!("/api/approvals/{approval}/approve"))
            .user(&director)
            .send(&app)
            .await;
        assert_eq!(approved.status, StatusCode::OK, "{}", approved.json);
        let other = Req::get("/api/activity").user(&newcomer).send(&app).await;
        assert!(
            seen(&other, waiting) && !seen(&other, project),
            "{}",
            other.json
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn links_only_to_pages_the_viewer_can_open(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let nova = testing::project(&db, Committee::Events, "nova").await;
        let other = testing::project(&db, Committee::Events, "other").await;
        let director = testing::user(
            &db,
            "linkdirector",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        let lead = UserSpec::new("linklead")
            .lead(Committee::Events, true)
            .edits(nova)
            .create(&db)
            .await;

        let mut conn = db.acquire().await.unwrap();
        for (subject_type, subject_id, summary) in [
            (subject::PERSON, lead.id, "Added a lead in Events"),
            (subject::PROJECT, nova, "Published Nova"),
            (subject::PROJECT, other, "Published Other"),
        ] {
            record(
                &mut conn,
                Entry {
                    actor: Some(director.id),
                    action: "test.action",
                    subject_type,
                    subject_id: Some(subject_id.to_string()),
                    committee: Some(Committee::Events),
                    summary: summary.into(),
                },
            )
            .await
            .unwrap();
        }
        let links = |res: &testing::Res| -> Vec<serde_json::Value> {
            res.json["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["href"].clone())
                .collect()
        };

        // A project the lead cannot open is left out, not only its link.
        let lead_view = Req::get("/api/activity").user(&lead).send(&app).await;
        assert_eq!(
            links(&lead_view),
            [
                serde_json::Value::from(format!("/portal/projects/{nova}/")),
                serde_json::Value::Null,
            ]
        );
        let people = Req::get("/api/people").user(&lead).send(&app).await;
        assert_eq!(people.status, StatusCode::FORBIDDEN);

        let director_view = Req::get("/api/activity").user(&director).send(&app).await;
        assert_eq!(
            links(&director_view),
            [
                serde_json::Value::from(format!("/portal/projects/{other}/")),
                format!("/portal/projects/{nova}/").into(),
                "/portal/people/".into(),
            ]
        );
    }
}
