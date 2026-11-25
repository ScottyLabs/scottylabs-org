//! The ticker endpoints and approval hooks, driven through the real router for each role.

use axum::Router;
use axum::http::StatusCode;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{apply_approval, apply_send_back};
use crate::approvals_core::{self, ApprovalRow, SubjectType};
use crate::auth::{Assignment, Viewer};
use crate::models::{AuthMode, Committee, Role, TickerSource};
use crate::testing::{self, Req, Res, TestUser, UserSpec};

/// Everyone the rules talk about.
struct Cast {
    app: Router,
    admin: TestUser,
    events_director: TestUser,
    tech_director: TestUser,
    lead: TestUser,
    quiet_lead: TestUser,
    member: TestUser,
}

async fn cast(db: &PgPool) -> Cast {
    let app = testing::app(db.clone()).await;
    Cast {
        app,
        admin: UserSpec::new("admin")
            .name("Ada Admin")
            .admin()
            .create(db)
            .await,
        events_director: UserSpec::new("evdir")
            .name("Evan Foster")
            .director(Committee::Events)
            .create(db)
            .await,
        tech_director: UserSpec::new("techdir")
            .name("Anish Pallati")
            .director(Committee::Tech)
            .create(db)
            .await,
        lead: UserSpec::new("lead")
            .name("Sara Chokshi")
            .lead(Committee::Events, true)
            .create(db)
            .await,
        quiet_lead: UserSpec::new("quiet")
            .lead(Committee::Events, false)
            .create(db)
            .await,
        member: UserSpec::new("member").create(db).await,
    }
}

fn input(committee: &str, message: &str, expires_at: DateTime<Utc>) -> Value {
    json!({
        "kind": "event",
        "committee": committee,
        "message": message,
        "linkUrl": "https://tartanhacks.com",
        "expiresAt": expires_at,
    })
}

fn in_days(days: i64) -> DateTime<Utc> {
    Utc::now() + TimeDelta::days(days)
}

async fn post(app: &Router, user: &TestUser, body: Value) -> Res {
    Req::post("/api/announcements")
        .user(user)
        .json(body)
        .send(app)
        .await
}

/// Posts and returns the new announcement's id, failing the test unless it was created.
async fn posted(app: &Router, user: &TestUser, committee: &str, message: &str) -> Uuid {
    let res = post(app, user, input(committee, message, in_days(7))).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
    id_of(&res.json["announcement"])
}

fn id_of(announcement: &Value) -> Uuid {
    announcement["id"].as_str().unwrap().parse().unwrap()
}

/// A live announcement written straight to the database, for setting up a full ticker quickly.
async fn live_row(db: &PgPool, committee: &str, message: &str, position: i32) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO announcements (id, kind, committee, message, state, starts_at, expires_at, position)
         VALUES ($1, 'event', $2, $3, 'live', now(), now() + interval '5 days', $4)",
    )
    .bind(id)
    .bind(committee)
    .bind(message)
    .bind(position)
    .execute(db)
    .await
    .unwrap();
    id
}

async fn set_column(db: &PgPool, id: Uuid, assignment: &str) {
    sqlx::query(&format!(
        "UPDATE announcements SET {assignment} WHERE id = $1"
    ))
    .bind(id)
    .execute(db)
    .await
    .unwrap();
}

async fn state_of(db: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT state FROM announcements WHERE id = $1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn approvals_for(db: &PgPool, id: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT status FROM approvals WHERE subject_type = 'announcement' AND subject_id = $1 ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .unwrap()
}

async fn tab(app: &Router, user: &TestUser, name: &str) -> Value {
    let res = Req::get(&format!("/api/announcements?tab={name}"))
        .user(user)
        .send(app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    res.json
}

fn messages(list: &Value) -> Vec<&str> {
    list["announcements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["message"].as_str().unwrap())
        .collect()
}

async fn public_messages(app: &Router) -> Vec<String> {
    let res = Req::get("/api/public/ticker").send(app).await;
    res.json["announcements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["message"].as_str().unwrap().to_owned())
        .collect()
}

/// The request-scoped viewer that routes/approvals.rs hands to the hooks.
fn viewer(user: &TestUser, role: Role, committee: Option<Committee>) -> Viewer {
    Viewer {
        user_id: user.id,
        andrew_id: user.andrew_id.clone(),
        name: user.andrew_id.clone(),
        email: None,
        session_id: Uuid::now_v7(),
        csrf_token: user.csrf.clone(),
        auth_mode: AuthMode::Dev,
        roles: vec![Assignment {
            role,
            committee,
            can_post_ticker: true,
        }],
        lead_projects: Vec::new(),
    }
}

async fn waiting_approval(db: &PgPool, id: Uuid) -> ApprovalRow {
    let mut conn = db.acquire().await.unwrap();
    approvals_core::latest(&mut conn, SubjectType::Announcement, id)
        .await
        .unwrap()
        .unwrap()
}

/// Who an approval is credited to.
async fn submitter(db: &PgPool, approval: &ApprovalRow) -> Option<Uuid> {
    sqlx::query_scalar("SELECT submitted_by FROM approvals WHERE id = $1")
        .bind(approval.id)
        .fetch_one(db)
        .await
        .unwrap()
}

/// Decides on the latest approval the way routes/approvals.rs does: the hook and the approval's own status in one transaction, committed only when the hook succeeds.
async fn decide(
    db: &PgPool,
    id: Uuid,
    approver: &Viewer,
    approve: bool,
) -> Result<(), crate::error::AppError> {
    let approval = waiting_approval(db, id).await;
    let mut tx = db.begin().await.unwrap();
    let result = if approve {
        apply_approval(&mut tx, &approval, approver).await
    } else {
        apply_send_back(&mut tx, &approval, approver).await
    };
    if result.is_ok() {
        sqlx::query(
            "UPDATE approvals SET status = $2, decided_by = $3, decided_at = now() WHERE id = $1",
        )
        .bind(approval.id)
        .bind(if approve { "approved" } else { "sent_back" })
        .bind(approver.user_id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    result
}

// ---------------------------------------------------------------------------
// Public feed

#[sqlx::test(migrations = "./migrations")]
async fn the_public_feed_shows_only_live_announcements_in_order(db: PgPool) {
    let c = cast(&db).await;
    let second = live_row(&db, "tech", "Second", 1).await;
    live_row(&db, "club", "First", 0).await;
    let expired = live_row(&db, "events", "Expired", 2).await;
    set_column(&db, expired, "expires_at = now() - interval '1 minute'").await;
    let ended = live_row(&db, "events", "Ended", 3).await;
    set_column(&db, ended, "state = 'ended', ended_at = now()").await;
    posted(&c.app, &c.lead, "events", "Waiting for approval").await;

    let res = Req::get("/api/public/ticker").send(&c.app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(
        res.header("cache-control"),
        Some("public, max-age=60, stale-while-revalidate=600")
    );
    assert_eq!(res.header("access-control-allow-origin"), Some("*"));
    let items = res.json["announcements"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|a| a["message"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["First", "Second"]
    );
    let mut keys: Vec<&str> = items[1]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["committee", "id", "kind", "linkUrl", "message"]);
    assert_eq!(items[1]["id"], second.to_string());
    assert_eq!(items[1]["committee"], "tech");
}

// ---------------------------------------------------------------------------
// Posting

#[sqlx::test(migrations = "./migrations")]
async fn admins_and_directors_post_live_and_leads_send_for_approval(db: PgPool) {
    let c = cast(&db).await;

    let res = post(
        &c.app,
        &c.admin,
        input("club", "  Club news for everyone.  ", in_days(3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
    let club = &res.json["announcement"];
    assert_eq!(club["state"], "live");
    assert_eq!(club["message"], "Club news for everyone.");
    assert_eq!(club["linkUrl"], "https://tartanhacks.com/");
    assert_eq!(club["position"], 0);
    assert_eq!(club["createdBy"]["name"], "Ada Admin");
    assert_eq!(club["approval"], Value::Null);
    assert_eq!(club["canEdit"], true);
    assert_eq!(club["canManage"], true);
    assert!(club["startsAt"].is_string());

    let res = post(
        &c.app,
        &c.events_director,
        input("events", "Events news.", in_days(3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.json["announcement"]["state"], "live");
    assert_eq!(
        res.json["announcement"]["position"], 1,
        "joins the end of the loop"
    );

    let res = post(
        &c.app,
        &c.events_director,
        input("tech", "Not my committee.", in_days(3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert!(res.message().contains("Tech"), "{}", res.message());
    let res = post(
        &c.app,
        &c.events_director,
        input("club", "Not mine either.", in_days(3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    let res = post(&c.app, &c.lead, input("events", "Lead news.", in_days(3))).await;
    assert_eq!(res.status, StatusCode::CREATED);
    let pending = &res.json["announcement"];
    assert_eq!(pending["state"], "pending_approval");
    assert_eq!(pending["startsAt"], Value::Null);
    assert_eq!(pending["approval"]["status"], "waiting");
    assert_eq!(pending["canEdit"], true);
    assert_eq!(pending["canManage"], false);
    let approval = waiting_approval(&db, id_of(pending)).await;
    assert_eq!(approval.committee, Committee::Events);
    assert_eq!(submitter(&db, &approval).await, Some(c.lead.id));
    assert_eq!(approval.snapshot["message"], "Lead news.");
    assert_eq!(approval.snapshot["linkUrl"], "https://tartanhacks.com/");

    let res = post(
        &c.app,
        &c.lead,
        input("tech", "Wrong committee.", in_days(3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = post(
        &c.app,
        &c.quiet_lead,
        input("events", "Not allowed.", in_days(3)),
    )
    .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = post(&c.app, &c.member, input("events", "No role.", in_days(3))).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::post("/api/announcements")
        .json(input("events", "Signed out.", in_days(3)))
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = Req::post("/api/announcements")
        .cookie(&c.admin)
        .json(input("events", "No CSRF token.", in_days(3)))
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    assert_eq!(
        public_messages(&c.app).await,
        ["Club news for everyone.", "Events news."]
    );
    let summaries = testing::audit_summaries(&db).await;
    assert_eq!(
        summaries,
        [
            "Posted \u{201C}Club news for everyone.\u{201D} to the ticker",
            "Posted \u{201C}Events news.\u{201D} to the ticker",
            "Sent \u{201C}Lead news.\u{201D} for approval",
        ]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn announcements_are_validated(db: PgPool) {
    let c = cast(&db).await;
    let cases = [
        (input("events", &"x".repeat(91), in_days(3)), "message"),
        (input("events", "", in_days(3)), "message"),
        (input("events", "Two\nlines", in_days(3)), "message"),
        // Tabs, bells and bidirectional overrides could make the home page show something else.
        (input("events", "Tab\there", in_days(3)), "message"),
        (input("events", "Bell \u{7}", in_days(3)), "message"),
        (
            input("events", "Reversed \u{202E}txet", in_days(3)),
            "message",
        ),
        (
            json!({ "kind": "event", "committee": "events", "message": "Hi", "linkUrl": "http://tartanhacks.com", "expiresAt": in_days(3) }),
            "linkUrl",
        ),
        (
            input("events", "Past", Utc::now() - TimeDelta::minutes(1)),
            "expiresAt",
        ),
        (
            input("events", "Too far", in_days(30) + TimeDelta::minutes(5)),
            "expiresAt",
        ),
    ];
    for (body, field) in cases {
        let res = post(&c.app, &c.events_director, body).await;
        assert_eq!(
            res.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field}: {}",
            res.text()
        );
        assert_eq!(res.code(), "validation_failed");
        assert!(
            res.json["error"]["fields"][field].is_string(),
            "{field}: {}",
            res.text()
        );
    }
    let res = post(
        &c.app,
        &c.events_director,
        json!({ "kind": "rumor", "committee": "events" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Postgres cannot store NUL; it is refused instead of failing as an internal error.
    let res = post(
        &c.app,
        &c.lead,
        input("events", "null \u{0} byte", in_days(3)),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        res.text()
    );
    assert_eq!(res.code(), "validation_failed");

    let res = post(
        &c.app,
        &c.events_director,
        json!({ "kind": "milestone", "committee": "events", "message": "No link.", "linkUrl": "", "expiresAt": in_days(30) - TimeDelta::minutes(1) }),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
    assert_eq!(res.json["announcement"]["linkUrl"], Value::Null);
}

#[sqlx::test(migrations = "./migrations")]
async fn mangled_ids_and_methods_answer_in_the_error_shape(db: PgPool) {
    let c = cast(&db).await;
    for (method, path) in [
        ("GET", "/api/announcements/not-a-uuid"),
        ("PATCH", "/api/announcements/not-a-uuid"),
        ("DELETE", "/api/announcements/not-a-uuid"),
        ("POST", "/api/announcements/not-a-uuid/end"),
    ] {
        let request = match method {
            "GET" => Req::get(path),
            "PATCH" => Req::patch(path).json(json!({})),
            "DELETE" => Req::delete(path),
            _ => Req::post(path).json(json!({})),
        };
        let res = request.user(&c.events_director).send(&c.app).await;
        assert_eq!(
            res.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            res.text()
        );
        assert_eq!(res.message(), "We couldn't find that announcement.");
    }

    let res = Req::put("/api/me")
        .user(&c.events_director)
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.code(), "method_not_allowed");
    assert_eq!(res.header("allow"), Some("GET,HEAD"));
}

#[sqlx::test(migrations = "./migrations")]
async fn at_most_six_are_live(db: PgPool) {
    let c = cast(&db).await;
    for position in 0..5 {
        live_row(&db, "tech", &format!("Tech {position}"), position).await;
    }
    let expired = live_row(&db, "tech", "Expired one", 9).await;
    set_column(&db, expired, "expires_at = now() - interval '1 hour'").await;

    posted(&c.app, &c.admin, "club", "The sixth").await;
    let res = post(
        &c.app,
        &c.events_director,
        input("events", "A seventh", in_days(2)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(
        res.message(),
        "All 6 ticker slots are in use. End an announcement first, then post this one."
    );
    // Leads can still send theirs; it waits for a free slot.
    let res = post(
        &c.app,
        &c.lead,
        input("events", "Waiting its turn", in_days(2)),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);

    let live = tab(&c.app, &c.events_director, "live").await;
    assert_eq!(live["slots"], json!({ "used": 6, "total": 6 }));
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_posts_cannot_take_the_same_last_slot(db: PgPool) {
    let c = cast(&db).await;
    for position in 0..5 {
        live_row(&db, "tech", &format!("Tech {position}"), position).await;
    }
    let attempts = (0..4).map(|n| {
        post(
            &c.app,
            &c.admin,
            input("club", &format!("Race {n}"), in_days(2)),
        )
    });
    let results = futures::future::join_all(attempts).await;
    let created = results
        .iter()
        .filter(|res| res.status == StatusCode::CREATED)
        .count();
    let refused = results
        .iter()
        .filter(|res| res.status == StatusCode::CONFLICT)
        .count();
    assert_eq!((created, refused), (1, 3));
    assert_eq!(public_messages(&c.app).await.len(), 6);
}

// ---------------------------------------------------------------------------
// Listing

#[sqlx::test(migrations = "./migrations")]
async fn tabs_show_what_each_person_may_see(db: PgPool) {
    let c = cast(&db).await;
    let other_lead = UserSpec::new("otherlead")
        .lead(Committee::Tech, true)
        .create(&db)
        .await;
    let live = posted(&c.app, &c.tech_director, "tech", "Tech is live").await;
    let mine = posted(&c.app, &c.lead, "events", "Sara waits").await;
    let theirs = posted(&c.app, &other_lead, "tech", "Tech lead waits").await;
    let ended = live_row(&db, "events", "Ended last week", 5).await;
    set_column(
        &db,
        ended,
        "state = 'ended', ended_at = now() - interval '7 days'",
    )
    .await;
    let old = live_row(&db, "events", "Expired long ago", 6).await;
    set_column(&db, old, "expires_at = now() - interval '91 days'").await;
    let expired = live_row(&db, "events", "Expired yesterday", 7).await;
    set_column(&db, expired, "expires_at = now() - interval '1 day'").await;

    let list = tab(&c.app, &c.lead, "live").await;
    assert_eq!(messages(&list), ["Tech is live"]);
    assert_eq!(list["announcements"][0]["canEdit"], false);
    assert_eq!(list["announcements"][0]["canManage"], false);
    assert_eq!(
        list["counts"],
        json!({ "live": 1, "waiting": 1, "expired": 2 })
    );
    assert_eq!(list["approvers"][0]["committee"], "events");
    assert_eq!(list["approvers"][0]["people"][0]["name"], "Evan Foster");

    assert_eq!(
        messages(&tab(&c.app, &c.lead, "waiting").await),
        ["Sara waits"]
    );
    assert_eq!(
        messages(&tab(&c.app, &c.events_director, "waiting").await),
        ["Sara waits"]
    );
    assert_eq!(
        messages(&tab(&c.app, &c.tech_director, "waiting").await),
        ["Tech lead waits"]
    );
    assert_eq!(
        messages(&tab(&c.app, &c.admin, "waiting").await),
        ["Tech lead waits", "Sara waits"]
    );
    let director_live = tab(&c.app, &c.tech_director, "live").await;
    assert_eq!(director_live["announcements"][0]["canManage"], true);
    assert_eq!(director_live["approvers"], json!([]));

    let expired_tab = tab(&c.app, &c.lead, "expired").await;
    assert_eq!(
        messages(&expired_tab),
        ["Expired yesterday", "Ended last week"]
    );
    assert_eq!(expired_tab["announcements"][0]["state"], "expired");
    assert_eq!(expired_tab["announcements"][1]["state"], "ended");
    assert_eq!(expired_tab["announcements"][0]["canEdit"], false);

    // One announcement, for the edit page. Waiting ones are private.
    let (status, body) =
        testing::get(&c.app, &format!("/api/announcements/{mine}"), Some(&c.lead)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["announcement"]["message"], "Sara waits");
    let (status, _) = testing::get(
        &c.app,
        &format!("/api/announcements/{theirs}"),
        Some(&c.lead),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) =
        testing::get(&c.app, &format!("/api/announcements/{live}"), Some(&c.lead)).await;
    assert_eq!(status, StatusCode::OK);

    let res = Req::get("/api/announcements?tab=scheduled")
        .user(&c.admin)
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    let res = Req::get("/api/announcements")
        .user(&c.member)
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::get("/api/announcements").send(&c.app).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_expired_tab_comes_in_pages_of_fifty(db: PgPool) {
    let c = cast(&db).await;
    // 120 that left the ticker in the last few days, mostly three at the same time, so a page ends inside a tie; every fourth was ended early. One left 91 days ago and is no longer listed.
    sqlx::query(
        "INSERT INTO announcements (id, kind, committee, message, state, starts_at, expires_at, ended_at)
         SELECT gen_random_uuid(), 'event', 'events', 'Left ' || g,
                CASE WHEN g % 4 = 0 THEN 'ended' ELSE 'live' END,
                now() - interval '30 days',
                CASE WHEN g % 4 = 0 THEN now() + interval '1 day' ELSE left_at END,
                CASE WHEN g % 4 = 0 THEN left_at END
         FROM generate_series(1, 120) AS g,
              LATERAL (SELECT date_trunc('second', now()) - interval '1 hour' * ((g + 1) / 3) AS left_at) AS t",
    )
    .execute(&db)
    .await
    .unwrap();
    let old = live_row(&db, "events", "Expired long ago", 0).await;
    set_column(&db, old, "expires_at = now() - interval '91 days'").await;
    live_row(&db, "events", "Still live", 1).await;

    let page = |before: Option<String>| {
        let app = c.app.clone();
        let user = c.lead.clone();
        async move {
            let mut path = "/api/announcements?tab=expired".to_owned();
            if let Some(before) = before {
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("before", &before)
                    .finish();
                path = format!("{path}&{query}");
            }
            let res = Req::get(&path).user(&user).send(&app).await;
            assert_eq!(res.status, StatusCode::OK, "{}", res.text());
            res.json
        }
    };
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut before = None;
    let mut sizes = Vec::new();
    loop {
        let list = page(before.clone()).await;
        assert_eq!(list["counts"]["expired"], 120);
        let rows = list["announcements"].as_array().unwrap();
        sizes.push(rows.len());
        for row in rows {
            let left = row["endedAt"]
                .as_str()
                .or(row["expiresAt"].as_str())
                .unwrap();
            seen.push((left.to_owned(), row["id"].as_str().unwrap().to_owned()));
        }
        match list["nextBefore"].as_str() {
            Some(next) => before = Some(next.to_owned()),
            None => break,
        }
    }
    assert_eq!(sizes, [50, 50, 20]);
    let ids: std::collections::HashSet<&str> = seen.iter().map(|(_, id)| id.as_str()).collect();
    assert_eq!(ids.len(), 120, "no announcement is listed twice");
    assert!(!ids.contains(old.to_string().as_str()));
    // Newest first by when each one left; ties keep a stable order across pages.
    let times: Vec<DateTime<Utc>> = seen.iter().map(|(left, _)| left.parse().unwrap()).collect();
    assert!(times.windows(2).all(|pair| pair[0] >= pair[1]));

    // The other tabs are not paged, and a cursor that is not one is refused.
    let list = tab(&c.app, &c.lead, "live").await;
    assert_eq!(messages(&list), ["Still live"]);
    assert_eq!(list["nextBefore"], Value::Null);
    let res = Req::get("/api/announcements?tab=live&before=nonsense")
        .user(&c.lead)
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let res = Req::get("/api/announcements?tab=expired&before=yesterday")
        .user(&c.lead)
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(res.json["error"]["fields"]["before"].is_string());
}

// ---------------------------------------------------------------------------
// Editing

#[sqlx::test(migrations = "./migrations")]
async fn directors_edit_live_announcements_at_once(db: PgPool) {
    let c = cast(&db).await;
    let id = posted(&c.app, &c.events_director, "events", "Old words").await;
    let path = format!("/api/announcements/{id}");

    let (status, body) = testing::patch(
        &c.app,
        &path,
        Some(&c.events_director),
        json!({ "message": "New words", "linkUrl": null }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["announcement"]["message"], "New words");
    assert_eq!(body["announcement"]["linkUrl"], Value::Null);
    assert_eq!(body["announcement"]["state"], "live");
    assert_eq!(public_messages(&c.app).await, ["New words"]);

    let (status, _) = testing::patch(
        &c.app,
        &path,
        Some(&c.lead),
        json!({ "message": "Lead edit" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = testing::patch(
        &c.app,
        &path,
        Some(&c.tech_director),
        json!({ "message": "Tech edit" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = testing::patch(
        &c.app,
        &path,
        Some(&c.events_director),
        json!({ "committee": "tech" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["fields"]["committee"].is_string());
    let (status, _) = testing::patch(
        &c.app,
        &path,
        Some(&c.events_director),
        json!({ "expiresAt": in_days(31) }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, body) = testing::patch(
        &c.app,
        &path,
        Some(&c.admin),
        json!({ "committee": "club", "kind": "club_news" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["announcement"]["committee"], "club");

    set_column(&db, id, "state = 'ended', ended_at = now()").await;
    let (status, _) = testing::patch(
        &c.app,
        &path,
        Some(&c.admin),
        json!({ "message": "Too late" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn leads_resubmit_when_they_edit(db: PgPool) {
    let c = cast(&db).await;
    let id = posted(&c.app, &c.lead, "events", "First try").await;
    let path = format!("/api/announcements/{id}");

    let (status, body) = testing::patch(
        &c.app,
        &path,
        Some(&c.lead),
        json!({ "message": "Second try" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["announcement"]["state"], "pending_approval");
    assert_eq!(approvals_for(&db, id).await, ["withdrawn", "waiting"]);
    assert_eq!(
        waiting_approval(&db, id).await.snapshot["message"],
        "Second try"
    );

    let director = viewer(&c.events_director, Role::Director, Some(Committee::Events));
    decide(&db, id, &director, false).await.unwrap();
    assert_eq!(state_of(&db, id).await, "sent_back");
    let (_, body) = testing::get(&c.app, &path, Some(&c.lead)).await;
    assert_eq!(body["announcement"]["state"], "sent_back");
    assert_eq!(body["announcement"]["approval"]["status"], "sent_back");
    assert_eq!(
        body["announcement"]["approval"]["decidedBy"]["name"],
        "Evan Foster"
    );
    let (status, _) = testing::patch(
        &c.app,
        &path,
        Some(&c.quiet_lead),
        json!({ "message": "Not mine" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = testing::patch(
        &c.app,
        &path,
        Some(&c.lead),
        json!({ "message": "Third try", "expiresAt": in_days(4) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["announcement"]["state"], "pending_approval");
    assert_eq!(body["announcement"]["approval"]["status"], "waiting");
    assert_eq!(state_of(&db, id).await, "pending_approval");
    assert_eq!(
        approvals_for(&db, id).await,
        ["withdrawn", "sent_back", "waiting"]
    );

    let (status, body) =
        testing::patch(&c.app, &path, Some(&c.lead), json!({ "committee": "club" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let summaries = testing::audit_summaries(&db).await;
    assert!(summaries.contains(&"Sent \u{201C}Third try\u{201D} for approval again".to_owned()));
    assert!(summaries.contains(&"Sent \u{201C}Second try\u{201D} back for changes".to_owned()));
}

// ---------------------------------------------------------------------------
// Extend, end, post again

#[sqlx::test(migrations = "./migrations")]
async fn managers_extend_and_end_live_announcements(db: PgPool) {
    let c = cast(&db).await;
    let id = posted(&c.app, &c.events_director, "events", "Lasting news").await;
    let extend = format!("/api/announcements/{id}/extend");
    let before: DateTime<Utc> =
        sqlx::query_scalar("SELECT expires_at FROM announcements WHERE id = $1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();

    let (status, body) = testing::post(
        &c.app,
        &extend,
        Some(&c.events_director),
        json!({ "days": 7 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let after: DateTime<Utc> = body["announcement"]["expiresAt"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(after - before, TimeDelta::days(7));

    // Never past 30 days from now.
    let (status, body) =
        testing::post(&c.app, &extend, Some(&c.admin), json!({ "days": 30 })).await;
    assert_eq!(status, StatusCode::OK);
    let capped: DateTime<Utc> = body["announcement"]["expiresAt"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((capped - in_days(30)).num_seconds().abs() < 5);
    let (status, body) = testing::post(&c.app, &extend, Some(&c.admin), json!({ "days": 1 })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    for days in [0, 31] {
        let (status, _) =
            testing::post(&c.app, &extend, Some(&c.admin), json!({ "days": days })).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    for user in [&c.lead, &c.tech_director] {
        let (status, _) = testing::post(&c.app, &extend, Some(user), json!({ "days": 7 })).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = testing::post(
            &c.app,
            &format!("/api/announcements/{id}/end"),
            Some(user),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    let end = format!("/api/announcements/{id}/end");
    let res = Req::post(&end).user(&c.events_director).send(&c.app).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json["announcement"]["state"], "ended");
    assert!(res.json["announcement"]["endedAt"].is_string());
    assert!(public_messages(&c.app).await.is_empty());
    let res = Req::post(&end).user(&c.events_director).send(&c.app).await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    let (status, _) = testing::post(
        &c.app,
        &extend,
        Some(&c.events_director),
        json!({ "days": 7 }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        messages(&tab(&c.app, &c.events_director, "expired").await),
        ["Lasting news"]
    );

    let summaries = testing::audit_summaries(&db).await;
    assert!(summaries.contains(&"Extended \u{201C}Lasting news\u{201D} by 7 days".to_owned()));
    assert!(
        summaries.contains(&"Extended \u{201C}Lasting news\u{201D} to the 30-day limit".to_owned())
    );
    assert!(summaries.contains(&"Took \u{201C}Lasting news\u{201D} off the ticker".to_owned()));
}

#[sqlx::test(migrations = "./migrations")]
async fn announcements_that_left_can_be_posted_again(db: PgPool) {
    let c = cast(&db).await;
    let expired = live_row(&db, "events", "Back again", 0).await;
    set_column(&db, expired, "expires_at = now() - interval '2 days'").await;
    let live = live_row(&db, "events", "Still here", 1).await;

    let res = Req::post(&format!("/api/announcements/{expired}/repost"))
        .user(&c.events_director)
        .json(json!({ "expiresAt": in_days(5) }))
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
    let copy = &res.json["announcement"];
    assert_eq!(copy["state"], "live");
    assert_eq!(copy["message"], "Back again");
    assert_ne!(copy["id"], expired.to_string());
    assert_eq!(public_messages(&c.app).await, ["Still here", "Back again"]);

    let res = Req::post(&format!("/api/announcements/{expired}/repost"))
        .user(&c.lead)
        .json(json!({ "expiresAt": in_days(5) }))
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.json["announcement"]["state"], "pending_approval");

    for (user, id, status) in [
        (&c.events_director, live, StatusCode::CONFLICT),
        (&c.tech_director, expired, StatusCode::FORBIDDEN),
        (&c.quiet_lead, expired, StatusCode::FORBIDDEN),
    ] {
        let res = Req::post(&format!("/api/announcements/{id}/repost"))
            .user(user)
            .json(json!({ "expiresAt": in_days(5) }))
            .send(&c.app)
            .await;
        assert_eq!(res.status, status, "{}", res.text());
    }
    let res = Req::post(&format!("/api/announcements/{expired}/repost"))
        .user(&c.events_director)
        .json(json!({ "expiresAt": in_days(45) }))
        .send(&c.app)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// Order and deleting

#[sqlx::test(migrations = "./migrations")]
async fn directors_reorder_the_live_set(db: PgPool) {
    let c = cast(&db).await;
    let a = live_row(&db, "tech", "A", 0).await;
    let b = live_row(&db, "events", "B", 1).await;
    let z = live_row(&db, "club", "C", 2).await;
    let gone = live_row(&db, "club", "Gone", 3).await;
    set_column(&db, gone, "expires_at = now() - interval '1 minute'").await;

    let (status, body) = testing::put(
        &c.app,
        "/api/announcements/order",
        Some(&c.events_director),
        json!({ "ids": [z, a, b] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(messages(&body), ["C", "A", "B"]);
    assert_eq!(public_messages(&c.app).await, ["C", "A", "B"]);

    for ids in [json!([a, b]), json!([a, b, z, gone]), json!([a, a, b, z])] {
        let (status, _) = testing::put(
            &c.app,
            "/api/announcements/order",
            Some(&c.admin),
            json!({ "ids": ids }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }
    let (status, _) = testing::put(
        &c.app,
        "/api/announcements/order",
        Some(&c.lead),
        json!({ "ids": [a, b, z] }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Changed the order of the ticker".to_owned())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn submissions_are_deleted_by_their_author_or_a_manager(db: PgPool) {
    let c = cast(&db).await;
    let mine = posted(&c.app, &c.lead, "events", "Never mind").await;
    let live = posted(&c.app, &c.events_director, "events", "Live one").await;

    let (status, _) =
        testing::delete(&c.app, &format!("/api/announcements/{live}"), Some(&c.lead)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = testing::delete(
        &c.app,
        &format!("/api/announcements/{mine}"),
        Some(&c.quiet_lead),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = testing::delete(
        &c.app,
        &format!("/api/announcements/{mine}"),
        Some(&c.tech_director),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) =
        testing::delete(&c.app, &format!("/api/announcements/{mine}"), Some(&c.lead)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(approvals_for(&db, mine).await, ["withdrawn"]);
    let (status, _) = testing::delete(
        &c.app,
        &format!("/api/announcements/{live}"),
        Some(&c.events_director),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = testing::delete(
        &c.app,
        &format!("/api/announcements/{live}"),
        Some(&c.events_director),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Deleted \u{201C}Never mind\u{201D}".to_owned())
    );
}

// ---------------------------------------------------------------------------
// Approval hooks

#[sqlx::test(migrations = "./migrations")]
async fn approving_publishes_the_snapshot_at_the_end_of_the_loop(db: PgPool) {
    let c = cast(&db).await;
    live_row(&db, "tech", "Already live", 4).await;
    let id = posted(&c.app, &c.lead, "events", "Approve me").await;
    // The row changing after submission does not change what was approved.
    set_column(&db, id, "message = 'Changed behind the approval'").await;

    let tech = viewer(&c.tech_director, Role::Director, Some(Committee::Tech));
    let err = decide(&db, id, &tech, true).await.unwrap_err();
    assert_eq!(err.code(), "forbidden");

    let director = viewer(&c.events_director, Role::Director, Some(Committee::Events));
    decide(&db, id, &director, true).await.unwrap();
    let row: (String, String, Option<Uuid>, i32, bool) = sqlx::query_as(
        "SELECT state, message, approved_by, position, starts_at IS NOT NULL FROM announcements WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "live".into(),
            "Approve me".into(),
            Some(c.events_director.id),
            5,
            true
        )
    );
    assert_eq!(
        public_messages(&c.app).await,
        ["Already live", "Approve me"]
    );

    let err = decide(&db, id, &director, true).await.unwrap_err();
    assert_eq!(err.code(), "conflict");
    let list = tab(&c.app, &c.lead, "live").await;
    assert_eq!(
        list["announcements"][1]["approvedBy"]["name"],
        "Evan Foster"
    );
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Approved \u{201C}Approve me\u{201D} for the ticker".to_owned())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn approving_needs_a_free_slot_and_a_future_expiry(db: PgPool) {
    let c = cast(&db).await;
    let director = viewer(&c.events_director, Role::Director, Some(Committee::Events));
    let full = posted(&c.app, &c.lead, "events", "No room yet").await;
    for position in 0..6 {
        live_row(&db, "tech", &format!("Tech {position}"), position).await;
    }
    let err = decide(&db, full, &director, true).await.unwrap_err();
    assert_eq!(err.code(), "conflict");
    assert_eq!(
        err.to_string(),
        "All 6 ticker slots are in use. End an announcement first, then approve this one."
    );
    assert_eq!(state_of(&db, full).await, "pending_approval");

    sqlx::query("DELETE FROM announcements WHERE committee = 'tech'")
        .execute(&db)
        .await
        .unwrap();
    let late = posted(&c.app, &c.lead, "events", "Too late now").await;
    sqlx::query(
        "UPDATE approvals SET snapshot = jsonb_set(snapshot, '{expiresAt}', to_jsonb($2::text))
         WHERE subject_id = $1 AND status = 'waiting'",
    )
    .bind(late)
    .bind((Utc::now() + TimeDelta::minutes(2)).to_rfc3339())
    .execute(&db)
    .await
    .unwrap();
    let err = decide(&db, late, &director, true).await.unwrap_err();
    assert_eq!(err.code(), "validation_failed");
    // Two minutes are left, so the message must not say the time has passed.
    assert_eq!(
        err.to_string(),
        "This announcement leaves the ticker in less than 5 minutes, or already has. Send it back and ask the lead to pick a later time."
    );

    let admin = viewer(&c.admin, Role::Admin, None);
    decide(&db, full, &admin, true).await.unwrap();
    assert_eq!(state_of(&db, full).await, "live");
}

/// Runs a hook with an approval exactly as given, then rolls back.
async fn decide_with(
    db: &PgPool,
    approval: &ApprovalRow,
    approver: &Viewer,
    approve: bool,
) -> Result<(), crate::error::AppError> {
    let mut tx = db.begin().await.unwrap();
    if approve {
        apply_approval(&mut tx, approval, approver).await
    } else {
        apply_send_back(&mut tx, approval, approver).await
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn an_approval_replaced_by_an_edit_cannot_be_decided(db: PgPool) {
    let c = cast(&db).await;
    let director = viewer(&c.events_director, Role::Director, Some(Committee::Events));
    let id = posted(&c.app, &c.lead, "events", "First version").await;
    let first = waiting_approval(&db, id).await;
    let (status, body) = testing::patch(
        &c.app,
        &format!("/api/announcements/{id}"),
        Some(&c.lead),
        json!({ "message": "Second version" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(state_of(&db, id).await, "pending_approval");

    // As routes/approvals.rs loaded it before the edit, and as it is stored now.
    let mut withdrawn = first.clone();
    withdrawn.status = approvals_core::ApprovalStatus::Withdrawn;
    for stale in [&first, &withdrawn] {
        for approve in [true, false] {
            let err = decide_with(&db, stale, &director, approve)
                .await
                .unwrap_err();
            assert_eq!(err.code(), "conflict", "{err}");
        }
    }
    assert_eq!(state_of(&db, id).await, "pending_approval");

    let mut project = waiting_approval(&db, id).await;
    project.subject_type = SubjectType::Project;
    let err = decide_with(&db, &project, &director, true)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "internal");

    // The decision may be recorded before the hook runs.
    let mut current = waiting_approval(&db, id).await;
    sqlx::query("UPDATE approvals SET status = 'approved' WHERE id = $1")
        .bind(current.id)
        .execute(&db)
        .await
        .unwrap();
    current.status = approvals_core::ApprovalStatus::Approved;
    let mut tx = db.begin().await.unwrap();
    apply_approval(&mut tx, &current, &director).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(public_messages(&c.app).await, ["Second version"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn approvers_edit_a_submission_without_taking_it_over(db: PgPool) {
    let c = cast(&db).await;
    let id = posted(&c.app, &c.lead, "events", "Volunter sign-ups").await;
    let (status, body) = testing::patch(
        &c.app,
        &format!("/api/announcements/{id}"),
        Some(&c.events_director),
        json!({ "message": "Volunteer sign-ups" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["announcement"]["state"], "pending_approval");
    assert_eq!(body["announcement"]["createdBy"]["name"], "Sara Chokshi");
    let approval = waiting_approval(&db, id).await;
    assert_eq!(submitter(&db, &approval).await, Some(c.lead.id));
    assert_eq!(approval.snapshot["message"], "Volunteer sign-ups");
    assert!(testing::audit_summaries(&db).await.contains(
        &"Edited \u{201C}Volunteer sign-ups\u{201D} while it waits for approval".to_owned()
    ));

    let director = viewer(&c.events_director, Role::Director, Some(Committee::Events));
    decide(&db, id, &director, true).await.unwrap();
    assert_eq!(public_messages(&c.app).await, ["Volunteer sign-ups"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_seeded_submission_can_be_approved(db: PgPool) {
    testing::seed_personas(&db).await;
    crate::seed::run(&db, &testing::config()).await.unwrap();
    let app = testing::app(db.clone()).await;
    let evan = testing::persona(&db, "evanfost").await;
    let sara = testing::persona(&db, "spchoksh").await;

    let waiting = tab(&app, &sara, "waiting").await;
    assert_eq!(
        messages(&waiting),
        ["Volunteer sign-ups for TartanHacks are open."]
    );
    let id = id_of(&waiting["announcements"][0]);
    decide(
        &db,
        id,
        &viewer(&evan, Role::Director, Some(Committee::Events)),
        true,
    )
    .await
    .unwrap();
    let live = public_messages(&app).await;
    assert_eq!(live.len(), 6);
    assert_eq!(
        live[0],
        "Check out CollegeCart, new from the Labrador committee."
    );
    assert_eq!(live[5], "Volunteer sign-ups for TartanHacks are open.");
    let sources: Vec<TickerSource> =
        sqlx::query_scalar("SELECT committee FROM announcements ORDER BY position")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(sources[0], TickerSource::Labrador);
}
