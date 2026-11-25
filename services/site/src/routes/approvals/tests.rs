use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::*;
use crate::testing::{self, Req, Res, TestUser, UserSpec};

struct Setup {
    app: Router,
    admin: TestUser,
    events_director: TestUser,
    tech_director: TestUser,
    sara: TestUser,
    tech_lead: TestUser,
    /// Sara's announcement for Events and the approval it waits on.
    announcement: Uuid,
    announcement_approval: Uuid,
    /// The Tech lead's project and its approval.
    project: Uuid,
    project_approval: Uuid,
}

fn announcement_snapshot(committee: Committee, message: &str) -> Value {
    json!({
        "kind": "event",
        "committee": committee.slug(),
        "message": message,
        "linkUrl": "https://tartanhacks.com/",
        "expiresAt": (Utc::now() + chrono::Duration::days(5)).to_rfc3339(),
    })
}

/// An announcement waiting for approval, as the ticker stores a lead's post.
async fn pending_announcement(
    db: &PgPool,
    lead: &TestUser,
    committee: Committee,
    message: &str,
) -> (Uuid, Uuid) {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO announcements (id, kind, committee, message, link_url, state, expires_at, created_by)
         VALUES ($1, 'event', $2, $3, 'https://tartanhacks.com/', 'pending_approval', now() + interval '5 days', $4)",
    )
    .bind(id)
    .bind(committee)
    .bind(message)
    .bind(lead.id)
    .execute(db)
    .await
    .unwrap();
    let snapshot = announcement_snapshot(committee, message);
    let mut conn = db.acquire().await.unwrap();
    let approval = approvals_core::submit(
        &mut conn,
        SubjectType::Announcement,
        id,
        committee,
        lead.id,
        &snapshot,
    )
    .await
    .unwrap();
    (id, approval.id)
}

async fn project_submission(db: &PgPool, lead: &TestUser, project: Uuid, doc: Value) -> Uuid {
    let mut conn = db.acquire().await.unwrap();
    approvals_core::submit(
        &mut conn,
        SubjectType::Project,
        project,
        Committee::Tech,
        lead.id,
        &doc,
    )
    .await
    .unwrap()
    .id
}

async fn setup(db: &PgPool) -> Setup {
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
    let sara = UserSpec::new("spchoksh")
        .name("Sara Chokshi")
        .lead(Committee::Events, true)
        .create(db)
        .await;
    let project = testing::project(db, Committee::Tech, "cmu-maps").await;
    let tech_lead = UserSpec::new("sradhakr")
        .name("Siddarth Radhakrishnan")
        .lead(Committee::Tech, true)
        .edits(project)
        .create(db)
        .await;
    let (announcement, announcement_approval) =
        pending_announcement(db, &sara, Committee::Events, "Volunteer sign-ups are open.").await;
    let mut doc = testing::project_doc(Committee::Tech, "CMU Maps");
    doc["summary"] = json!("Find your way around campus");
    doc["description"] = json!("<p>Maps <script>alert(1)</script><strong>fast</strong></p>");
    doc["siteUrl"] = json!("javascript:alert(1)");
    doc["repoUrl"] = json!("https://github.com/ScottyLabs/cmumaps");
    doc["media"] = json!([
        { "src": "/seed/projects/cmu-maps.png", "alt": "The map", "width": 1200, "height": 900 },
        { "src": "https://evil.example/x.png", "alt": "", "width": null, "height": null }
    ]);
    let project_approval = project_submission(db, &tech_lead, project, doc).await;
    Setup {
        app,
        admin,
        events_director,
        tech_director,
        sara,
        tech_lead,
        announcement,
        announcement_approval,
        project,
        project_approval,
    }
}

async fn list(app: &Router, user: &TestUser, query: &str) -> Res {
    Req::get(&format!("/api/approvals{query}"))
        .user(user)
        .send(app)
        .await
}

fn ids(res: &Res) -> Vec<String> {
    res.json["approvals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect()
}

async fn status_of(db: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM approvals WHERE id = $1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

#[test]
fn queries_are_read_strictly() {
    assert_eq!(
        parse_query(None).unwrap(),
        ListQuery {
            status: StatusFilter::Waiting,
            mine: false
        }
    );
    assert_eq!(
        parse_query(Some("status=decided&mine=true")).unwrap(),
        ListQuery {
            status: StatusFilter::Decided,
            mine: true
        }
    );
    assert!(parse_query(Some("status=everything")).is_err());
    assert!(parse_query(Some("mine=maybe")).is_err());
    assert!(parse_query(Some("other=1")).is_ok());
}

#[test]
fn notes_are_trimmed_and_bounded() {
    assert_eq!(check_note(None, false).unwrap(), None);
    assert_eq!(check_note(Some("   "), false).unwrap(), None);
    assert!(check_note(Some(" "), true).is_err());
    assert_eq!(
        check_note(Some("  Add the date.\r\nThanks  "), true).unwrap(),
        Some("Add the date.\nThanks".into())
    );
    assert!(check_note(Some(&"x".repeat(MAX_NOTE_CHARS)), true).is_ok());
    assert!(check_note(Some(&"x".repeat(MAX_NOTE_CHARS + 1)), true).is_err());
    assert!(check_note(Some("bell\u{7}"), true).is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn approvers_see_waiting_submissions_for_their_committees(db: PgPool) {
    let s = setup(&db).await;

    let res = list(&s.app, &s.events_director, "").await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(ids(&res), [s.announcement_approval.to_string()]);
    let item = &res.json["approvals"][0];
    assert_eq!(item["subjectType"], "announcement");
    assert_eq!(item["subjectId"], s.announcement.to_string());
    assert_eq!(item["committee"], "events");
    assert_eq!(item["status"], "waiting");
    assert_eq!(item["canDecide"], true);
    assert_eq!(item["submittedBy"]["name"], "Sara Chokshi");
    assert_eq!(
        item["announcement"]["message"],
        "Volunteer sign-ups are open."
    );
    assert_eq!(item["announcement"]["linkUrl"], "https://tartanhacks.com/");
    assert_eq!(item["project"], Value::Null);
    assert_eq!(item["href"], format!("/portal/ticker/{}/", s.announcement));

    let res = list(&s.app, &s.tech_director, "?status=waiting").await;
    assert_eq!(ids(&res), [s.project_approval.to_string()]);
    let project = &res.json["approvals"][0]["project"];
    assert_eq!(project["name"], "CMU Maps");
    assert_eq!(project["summary"], "Find your way around campus");
    // What reaches the browser is cleaned again: no script, no unsafe link, no outside image.
    assert_eq!(project["description"], "<p>Maps <strong>fast</strong></p>");
    assert_eq!(project["siteUrl"], Value::Null);
    assert_eq!(project["repoUrl"], "https://github.com/ScottyLabs/cmumaps");
    assert_eq!(project["media"].as_array().unwrap().len(), 1);
    assert_eq!(
        res.json["approvals"][0]["href"],
        format!("/portal/projects/{}/", s.project)
    );

    let res = list(&s.app, &s.admin, "").await;
    assert_eq!(
        ids(&res),
        [
            s.announcement_approval.to_string(),
            s.project_approval.to_string()
        ]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn leads_see_only_their_own_submissions(db: PgPool) {
    let s = setup(&db).await;

    let res = list(&s.app, &s.sara, "").await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    let res = list(&s.app, &s.sara, "?mine=true").await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(ids(&res), [s.announcement_approval.to_string()]);
    assert_eq!(res.json["approvals"][0]["canDecide"], false);

    send_back_directly(&db, s.announcement_approval, &s.events_director).await;
    assert!(ids(&list(&s.app, &s.sara, "?mine=true").await).is_empty());
    let res = list(&s.app, &s.sara, "?mine=true&status=decided").await;
    assert_eq!(ids(&res), [s.announcement_approval.to_string()]);
    let decided = &res.json["approvals"][0];
    assert_eq!(decided["status"], "sent_back");
    assert_eq!(decided["note"], "Add the date.");
    assert_eq!(decided["decidedBy"]["name"], "Evan Foster");

    // The other lead's submission never shows up for Sara.
    let res = list(&s.app, &s.tech_lead, "?mine=true").await;
    assert_eq!(ids(&res), [s.project_approval.to_string()]);

    let res = list(&s.app, &s.sara, "?status=all").await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
}

async fn send_back_directly(db: &PgPool, approval: Uuid, director: &TestUser) {
    sqlx::query(
        "UPDATE approvals SET status = 'sent_back', note = 'Add the date.', decided_by = $2, decided_at = now()
         WHERE id = $1",
    )
    .bind(approval)
    .bind(director.id)
    .execute(db)
    .await
    .unwrap();
}

/// A lead who fixed and resent something sees only the new submission, not the old sent back one with its Edit button; approvers keep the whole history.
#[sqlx::test(migrations = "./migrations")]
async fn a_leads_list_keeps_only_the_latest_submission_of_each_subject(db: PgPool) {
    let s = setup(&db).await;
    send_back_directly(&db, s.announcement_approval, &s.events_director).await;
    let mut conn = db.acquire().await.unwrap();
    let resent = approvals_core::submit(
        &mut conn,
        SubjectType::Announcement,
        s.announcement,
        Committee::Events,
        s.sara.id,
        &announcement_snapshot(Committee::Events, "Volunteer sign-ups are open now."),
    )
    .await
    .unwrap()
    .id;
    drop(conn);

    let res = list(&s.app, &s.sara, "?mine=true").await;
    assert_eq!(ids(&res), [resent.to_string()]);
    let res = list(&s.app, &s.sara, "?mine=true&status=decided").await;
    assert!(ids(&res).is_empty(), "{:?}", ids(&res));
    let res = list(&s.app, &s.events_director, "?status=decided").await;
    assert_eq!(ids(&res), [s.announcement_approval.to_string()]);

    // A sent back announcement the lead then deleted has nothing left to edit.
    let (gone, gone_approval) =
        pending_announcement(&db, &s.sara, Committee::Events, "Bring a friend.").await;
    send_back_directly(&db, gone_approval, &s.events_director).await;
    let res = list(&s.app, &s.sara, "?mine=true&status=decided").await;
    assert_eq!(ids(&res), [gone_approval.to_string()]);
    sqlx::query("DELETE FROM announcements WHERE id = $1")
        .bind(gone)
        .execute(&db)
        .await
        .unwrap();
    let res = list(&s.app, &s.sara, "?mine=true&status=decided").await;
    assert!(ids(&res).is_empty(), "{:?}", ids(&res));
}

#[sqlx::test(migrations = "./migrations")]
async fn only_approvers_for_the_committee_can_decide(db: PgPool) {
    let s = setup(&db).await;
    let approve = format!("/api/approvals/{}/approve", s.announcement_approval);
    let send_back = format!("/api/approvals/{}/send-back", s.announcement_approval);

    for user in [&s.sara, &s.tech_director, &s.tech_lead] {
        let res = Req::post(&approve).user(user).send(&s.app).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", user.andrew_id);
        let res = Req::post(&send_back)
            .user(user)
            .json(json!({ "note": "No." }))
            .send(&s.app)
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", user.andrew_id);
    }
    assert!(
        Req::post(&approve)
            .user(&s.tech_director)
            .send(&s.app)
            .await
            .message()
            .contains("director of Events")
    );

    let missing = format!("/api/approvals/{}/approve", Uuid::now_v7());
    let res = Req::post(&missing).user(&s.admin).send(&s.app).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = Req::post("/api/approvals/not-a-uuid/approve")
        .user(&s.admin)
        .send(&s.app)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // The CSRF check still applies.
    let res = Req::post(&approve)
        .cookie(&s.events_director)
        .send(&s.app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    assert_eq!(status_of(&db, s.announcement_approval).await, "waiting");
    assert!(testing::audit_summaries(&db).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn decided_submissions_are_not_decided_again(db: PgPool) {
    let s = setup(&db).await;
    let approve = format!("/api/approvals/{}/approve", s.announcement_approval);
    for (status, message) in [
        ("approved", "already approved"),
        ("sent_back", "already sent back"),
        ("withdrawn", "nothing to decide"),
    ] {
        sqlx::query("UPDATE approvals SET status = $2 WHERE id = $1")
            .bind(s.announcement_approval)
            .bind(status)
            .execute(&db)
            .await
            .unwrap();
        let res = Req::post(&approve)
            .user(&s.events_director)
            .send(&s.app)
            .await;
        assert_eq!(res.status, StatusCode::CONFLICT, "{status}");
        assert!(res.message().contains(message), "{}", res.message());
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn sending_back_needs_a_note(db: PgPool) {
    let s = setup(&db).await;
    let path = format!("/api/approvals/{}/send-back", s.announcement_approval);

    for body in [
        json!({}),
        json!({ "note": "  " }),
        json!({ "note": "x".repeat(501) }),
    ] {
        let res = Req::post(&path)
            .user(&s.events_director)
            .json(body)
            .send(&s.app)
            .await;
        assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(res.json["error"]["fields"]["note"].is_string());
    }
    let res = Req::post(&path).user(&s.events_director).send(&s.app).await;
    assert_eq!(res.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let approve = format!("/api/approvals/{}/approve", s.announcement_approval);
    let res = Req::post(&approve)
        .user(&s.events_director)
        .json(json!({ "note": "x".repeat(501) }))
        .send(&s.app)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(status_of(&db, s.announcement_approval).await, "waiting");
}

/// When the subject cannot change (here, an announcement that is no longer waiting), nothing about the approval changes either.
#[sqlx::test(migrations = "./migrations")]
async fn a_failed_decision_changes_nothing(db: PgPool) {
    let s = setup(&db).await;
    sqlx::query("UPDATE announcements SET state = 'live' WHERE id = $1")
        .bind(s.announcement)
        .execute(&db)
        .await
        .unwrap();
    let res = Req::post(&format!(
        "/api/approvals/{}/approve",
        s.announcement_approval
    ))
    .user(&s.events_director)
    .send(&s.app)
    .await;
    assert!(!res.status.is_success());
    let res = Req::post(&format!(
        "/api/approvals/{}/send-back",
        s.announcement_approval
    ))
    .user(&s.events_director)
    .json(json!({ "note": "Please add the date." }))
    .send(&s.app)
    .await;
    assert!(!res.status.is_success());
    assert_eq!(status_of(&db, s.announcement_approval).await, "waiting");
    assert!(testing::audit_summaries(&db).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn approving_publishes_the_announcement(db: PgPool) {
    let s = setup(&db).await;
    let res = Req::post(&format!(
        "/api/approvals/{}/approve",
        s.announcement_approval
    ))
    .user(&s.events_director)
    .send(&s.app)
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json["status"], "approved");
    assert_eq!(res.json["decidedBy"]["name"], "Evan Foster");
    assert_eq!(res.json["canDecide"], false);
    let state: String = sqlx::query_scalar("SELECT state FROM announcements WHERE id = $1")
        .bind(s.announcement)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(state, "live");
    // The ticker's hook records the decision; this module adds no second entry.
    let summaries = testing::audit_summaries(&db).await;
    assert_eq!(summaries.len(), 1, "{summaries:?}");
    assert!(
        summaries[0].contains("Volunteer sign-ups are open."),
        "{summaries:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sending_back_returns_the_announcement_with_the_note(db: PgPool) {
    let s = setup(&db).await;
    let res = Req::post(&format!(
        "/api/approvals/{}/send-back",
        s.announcement_approval
    ))
    .user(&s.admin)
    .json(json!({ "note": " Add the date. " }))
    .send(&s.app)
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json["status"], "sent_back");
    assert_eq!(res.json["note"], "Add the date.");
    let state: String = sqlx::query_scalar("SELECT state FROM announcements WHERE id = $1")
        .bind(s.announcement)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(state, "sent_back");
    let summaries = testing::audit_summaries(&db).await;
    assert_eq!(summaries.len(), 1, "{summaries:?}");
}

#[sqlx::test(migrations = "./migrations")]
async fn approving_a_project_publishes_what_the_lead_sent(db: PgPool) {
    let s = setup(&db).await;
    let maps = testing::project(&db, Committee::Tech, "campus-maps").await;
    let mut doc = testing::project_doc(Committee::Tech, "Campus Maps");
    doc["summary"] = json!("Find your way around campus");
    doc["media"] = json!([
        { "src": "/seed/projects/cmu-maps.png", "alt": "The map", "width": 1200, "height": 900 }
    ]);
    let approval = project_submission(&db, &s.tech_lead, maps, doc).await;

    let res = Req::post(&format!("/api/approvals/{approval}/approve"))
        .user(&s.tech_director)
        .send(&s.app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json["status"], "approved");
    assert_eq!(res.json["href"], format!("/portal/projects/{maps}/"));

    let (status, body) = testing::get(&s.app, "/api/public/projects/campus-maps", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"]["summary"], "Find your way around campus");
    // The projects hook records the decision; this module adds no second entry.
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Approved and published Campus Maps"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sending_a_project_back_keeps_it_unpublished_with_the_note(db: PgPool) {
    let s = setup(&db).await;
    let res = Req::post(&format!("/api/approvals/{}/send-back", s.project_approval))
        .user(&s.admin)
        .json(json!({ "note": "Add a link to the live site." }))
        .send(&s.app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json["status"], "sent_back");

    let (status, _) = testing::get(&s.app, "/api/public/projects/cmu-maps", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mine = list(&s.app, &s.tech_lead, "?status=decided&mine=true").await;
    assert_eq!(
        mine.json["approvals"][0]["id"],
        s.project_approval.to_string()
    );
    assert_eq!(
        mine.json["approvals"][0]["note"],
        "Add a link to the live site."
    );
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Sent CMU Maps back with a note"]
    );
}
