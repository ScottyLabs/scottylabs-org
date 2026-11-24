use axum::Router;
use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::*;
use crate::approvals_core::{self, SubjectType};
use crate::testing::{self, Req, TestUser, UserSpec};

struct World {
    app: Router,
    admin: TestUser,
    events_director: TestUser,
    tech_director: TestUser,
    sara: TestUser,
    member: TestUser,
    nova: Uuid,
    terrier: Uuid,
    sara_waiting: Uuid,
}

fn page(cover: &str, tagline: &str) -> Value {
    json!({
        "blocks": [{
            "id": "cover",
            "type": "cover",
            "width": 12,
            "props": { "tagline": tagline, "image": { "src": cover, "alt": "Members", "width": 1600, "height": 900 } }
        }],
        "settings": {}
    })
}

async fn add_page(
    db: &PgPool,
    committee: Committee,
    published: Value,
    draft: Value,
    published_days_ago: i32,
    by: Uuid,
) {
    sqlx::query(
        "INSERT INTO committee_pages (committee, draft, published, draft_updated_at, draft_updated_by, published_at, published_by)
         VALUES ($1, $2, $3, now() - interval '2 hours', $5, now() - make_interval(days => $4), $5)",
    )
    .bind(committee)
    .bind(draft)
    .bind(published)
    .bind(published_days_ago)
    .bind(by)
    .execute(db)
    .await
    .unwrap();
}

async fn add_announcement(
    db: &PgPool,
    source: &str,
    message: &str,
    state: &str,
    expires_in_hours: i32,
    by: Uuid,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO announcements (id, kind, committee, message, state, starts_at, expires_at, created_by)
         VALUES ($1, 'event', $2, $3, $4, now(), now() + make_interval(hours => $5), $6)",
    )
    .bind(id)
    .bind(source)
    .bind(message)
    .bind(state)
    .bind(expires_in_hours)
    .bind(by)
    .execute(db)
    .await
    .unwrap();
    id
}

async fn world(db: &PgPool) -> World {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("tkanz")
        .name("Thomas Kanz")
        .admin()
        .create(db)
        .await;
    let events_director = UserSpec::new("evanfost")
        .name("Evan Foster")
        .director(Committee::Events)
        .create(db)
        .await;
    let tech_director = UserSpec::new("anishp")
        .name("Anish Pallati")
        .director(Committee::Tech)
        .create(db)
        .await;
    let nova = testing::published_project(
        db,
        Committee::Events,
        "nova",
        json!({
            "name": "Nova",
            "summary": "A hackathon for first-timers",
            "media": [{ "src": "/seed/projects/nova.png", "alt": "Nova", "width": 1200, "height": 900 }]
        }),
    )
    .await;
    testing::published_project(
        db,
        Committee::Events,
        "tartanhacks",
        json!({
            "name": "TartanHacks",
            "summary": "CMU's largest hackathon",
            "siteUrl": "https://tartanhacks.com/",
            "media": [{ "src": "/seed/projects/tartanhacks.png", "alt": "TartanHacks", "width": 1200, "height": 900 }]
        }),
    )
    .await;
    let terrier = testing::project(db, Committee::Tech, "terrier").await;
    sqlx::query(
        "UPDATE projects SET draft = jsonb_set(draft, '{name}', '\"Terrier\"') WHERE id = $1",
    )
    .bind(terrier)
    .execute(db)
    .await
    .unwrap();
    let sara = UserSpec::new("spchoksh")
        .name("Sara Chokshi")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(db)
        .await;
    let member = UserSpec::new("newmember").create(db).await;

    add_announcement(db, "club", "Giving Day is tomorrow", "live", 25, admin.id).await;
    add_announcement(
        db,
        "events",
        "TartanHacks passed 1,000 participants",
        "live",
        49,
        events_director.id,
    )
    .await;
    add_announcement(
        db,
        "tech",
        "CMU Maps has a new look",
        "live",
        240,
        tech_director.id,
    )
    .await;
    let pending = add_announcement(
        db,
        "events",
        "Volunteer sign-ups are open.",
        "pending_approval",
        72,
        sara.id,
    )
    .await;

    let events = page("/seed/committees/events.jpg", "We run TartanHacks");
    add_page(
        db,
        Committee::Events,
        events.clone(),
        events,
        21,
        events_director.id,
    )
    .await;
    add_page(
        db,
        Committee::Tech,
        page("/seed/committees/tech.jpg", "Old"),
        page("/seed/committees/tech.jpg", "New"),
        200,
        tech_director.id,
    )
    .await;
    let finance = page("/seed/committees/finance.jpg", "Money");
    add_page(
        db,
        Committee::Finance,
        finance.clone(),
        finance,
        120,
        admin.id,
    )
    .await;

    let mut conn = db.acquire().await.unwrap();
    let snapshot = json!({
        "kind": "event", "committee": "events", "message": "Volunteer sign-ups are open.",
        "linkUrl": null, "expiresAt": "2030-01-01T00:00:00Z"
    });
    let sara_waiting = approvals_core::submit(
        &mut conn,
        SubjectType::Announcement,
        pending,
        Committee::Events,
        sara.id,
        &snapshot,
    )
    .await
    .unwrap()
    .id;
    let mut doc = testing::project_doc(Committee::Events, "Nova");
    doc["summary"] = json!("A hackathon for first-timers");
    let sent_back = approvals_core::submit(
        &mut conn,
        SubjectType::Project,
        nova,
        Committee::Events,
        sara.id,
        &doc,
    )
    .await
    .unwrap()
    .id;
    sqlx::query(
        "UPDATE approvals SET status = 'sent_back', note = 'Add a repository link.', decided_by = $2, decided_at = now()
         WHERE id = $1",
    )
    .bind(sent_back)
    .bind(events_director.id)
    .execute(&mut *conn)
    .await
    .unwrap();

    World {
        app,
        admin,
        events_director,
        tech_director,
        sara,
        member,
        nova,
        terrier,
        sara_waiting,
    }
}

async fn overview_for(app: &Router, user: &TestUser) -> Value {
    let res = Req::get("/api/overview").user(user).send(app).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    res.json
}

fn kinds(overview: &Value) -> Vec<&str> {
    overview["attention"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["kind"].as_str().unwrap())
        .collect()
}

fn titles(overview: &Value) -> Vec<&str> {
    overview["attention"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap())
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn admins_see_everything_that_needs_them(db: PgPool) {
    let w = world(&db).await;
    let o = overview_for(&w.app, &w.admin).await;

    assert_eq!(o["ticker"]["live"], 3);
    assert_eq!(o["ticker"]["slots"], 6);
    assert_eq!(o["ticker"]["mineLive"], 3);
    assert_eq!(
        o["ticker"]["nextToExpire"]["message"],
        "Giving Day is tomorrow"
    );

    assert_eq!(o["committees"]["total"], 7);
    assert_eq!(o["committees"]["published"], 3);
    assert_eq!(o["committees"]["withUnpublishedChanges"], json!(["tech"]));
    assert_eq!(o["committees"]["editable"].as_array().unwrap().len(), 7);
    let covers = o["committees"]["covers"].as_array().unwrap();
    assert_eq!(covers.len(), 7);
    assert_eq!(covers[0]["slug"], "events");
    assert_eq!(covers[0]["cover"]["src"], "/seed/committees/events.jpg");
    assert_eq!(covers[1]["cover"], Value::Null);

    assert_eq!(o["projects"]["total"], 3);
    assert_eq!(o["projects"]["inScope"], 3);
    assert_eq!(o["projects"]["published"], 2);
    assert_eq!(o["projects"]["drafts"], 1);
    assert_eq!(o["projects"]["covers"].as_array().unwrap().len(), 2);

    assert_eq!(
        kinds(&o),
        [
            "approval",
            "expiring",
            "expiring",
            "unpublished_page",
            "project_incomplete",
            "project_incomplete",
            "stale_page"
        ]
    );
    assert_eq!(
        titles(&o),
        [
            "Sara Chokshi wants to post to the ticker",
            "\u{201c}Giving Day is tomorrow\u{201d} leaves the ticker tomorrow",
            "\u{201c}TartanHacks passed 1,000 participants\u{201d} leaves the ticker in 2 days",
            "The Tech page has changes that are not published",
            "Nova has no links",
            "Terrier has no cover image, summary or links",
            &format!(
                "The Finance page has not changed since {}",
                when::month(Utc::now() - chrono::Duration::days(120), Utc::now())
            ),
        ]
    );
    let first = &o["attention"][0];
    assert_eq!(first["severity"], "info");
    assert_eq!(first["action"]["label"], "Review");
    assert_eq!(
        first["action"]["href"],
        format!("/portal/?review={}", w.sara_waiting)
    );
    assert!(
        first["detail"]
            .as_str()
            .unwrap()
            .starts_with("Waiting for your approval since ")
    );
    assert_eq!(o["attention"][1]["severity"], "warning");
    assert_eq!(o["attention"][1]["detail"], "Ticker, posted by you");
    assert_eq!(o["attention"][1]["action"]["label"], "Extend");
    assert_eq!(
        o["attention"][3]["detail"],
        "Committee pages, edited by Anish Pallati 2 hours ago"
    );
    assert_eq!(
        o["attention"][3]["action"]["href"],
        "/portal/committees/tech/"
    );
    assert_eq!(o["attention"][4]["action"]["label"], "Add link");
    assert_eq!(
        o["attention"][4]["action"]["href"],
        format!("/portal/projects/{}/", w.nova)
    );
    assert!(
        o["attention"][4]["detail"]
            .as_str()
            .unwrap()
            .starts_with("Projects, published ")
    );
    assert_eq!(o["attention"][5]["action"]["label"], "Add image");
    assert_eq!(
        o["attention"][5]["action"]["href"],
        format!("/portal/projects/{}/", w.terrier)
    );
    assert!(
        o["attention"][5]["detail"]
            .as_str()
            .unwrap()
            .starts_with("Projects, draft since ")
    );
    assert_eq!(
        o["attention"][6]["detail"],
        "Committee pages, last published by you"
    );
    assert_eq!(o["attention"][6]["severity"], "neutral");
    assert!(o["activity"].is_array());
}

#[sqlx::test(migrations = "./migrations")]
async fn directors_see_their_committee(db: PgPool) {
    let w = world(&db).await;

    let o = overview_for(&w.app, &w.events_director).await;
    assert_eq!(o["ticker"]["live"], 3);
    assert_eq!(o["ticker"]["mineLive"], 1);
    assert_eq!(
        o["ticker"]["nextToExpire"]["message"],
        "TartanHacks passed 1,000 participants"
    );
    assert_eq!(o["committees"]["editable"], json!(["events"]));
    assert_eq!(o["committees"]["withUnpublishedChanges"], json!([]));
    assert_eq!(o["projects"]["inScope"], 2);
    assert_eq!(o["projects"]["published"], 2);
    assert_eq!(o["projects"]["drafts"], 0);
    assert_eq!(kinds(&o), ["approval", "expiring", "project_incomplete"]);
    assert_eq!(o["attention"][1]["detail"], "Ticker, posted by you");
    assert_eq!(o["attention"][2]["title"], "Nova has no links");

    let o = overview_for(&w.app, &w.tech_director).await;
    assert_eq!(o["ticker"]["mineLive"], 1);
    // The Tech page is both changed and stale; it is listed once.
    assert_eq!(kinds(&o), ["unpublished_page", "project_incomplete"]);
    assert_eq!(
        o["attention"][0]["detail"],
        "Committee pages, edited by you 2 hours ago"
    );
    assert_eq!(
        o["attention"][1]["title"],
        "Terrier has no cover image, summary or links"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn leads_see_their_own_work(db: PgPool) {
    let w = world(&db).await;
    let o = overview_for(&w.app, &w.sara).await;

    assert_eq!(o["ticker"]["live"], 3);
    assert_eq!(o["ticker"]["mineLive"], 0);
    assert_eq!(o["ticker"]["nextToExpire"], Value::Null);
    assert_eq!(o["committees"]["editable"], json!([]));
    assert_eq!(o["projects"]["total"], 3);
    assert_eq!(o["projects"]["inScope"], 1);
    assert_eq!(o["projects"]["published"], 1);

    assert_eq!(kinds(&o), ["sent_back", "project_incomplete"]);
    let sent_back = &o["attention"][0];
    assert_eq!(
        sent_back["title"],
        "Evan Foster sent back your changes to Nova"
    );
    assert_eq!(sent_back["detail"], "Note: Add a repository link.");
    assert_eq!(sent_back["severity"], "warning");
    assert_eq!(sent_back["action"]["label"], "Edit");
    assert_eq!(
        sent_back["action"]["href"],
        format!("/portal/projects/{}/", w.nova)
    );

    // Sending it again replaces the sent back item with a waiting one for the director.
    let mut conn = db.acquire().await.unwrap();
    approvals_core::submit(
        &mut conn,
        SubjectType::Project,
        w.nova,
        Committee::Events,
        w.sara.id,
        &testing::project_doc(Committee::Events, "Nova"),
    )
    .await
    .unwrap();
    let o = overview_for(&w.app, &w.sara).await;
    assert_eq!(kinds(&o), ["project_incomplete"]);
    let o = overview_for(&w.app, &w.events_director).await;
    assert_eq!(
        titles(&o)[..2],
        [
            "Sara Chokshi wants to post to the ticker",
            "Sara Chokshi wants to publish Nova"
        ]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn people_without_a_role_get_no_overview(db: PgPool) {
    let w = world(&db).await;
    let res = Req::get("/api/overview").user(&w.member).send(&w.app).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::get("/api/overview").send(&w.app).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}
