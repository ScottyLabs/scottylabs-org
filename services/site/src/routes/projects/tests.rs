//! Projects through the real router: who can do what, the rules for saving and publishing, addresses and redirects, approvals, the public answers and the seed.

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::doc::{self, ProjectDoc, Purpose};
use crate::approvals_core::{self, SubjectType};
use crate::auth::Viewer;
use crate::models::{Committee, Role};
use crate::permissions::tests::viewer;
use crate::testing::{self, Req, TestUser, UserSpec};

fn cover() -> Value {
    json!({ "src": "/seed/projects/maps.jpg", "alt": "A map of campus", "width": 1440, "height": 900 })
}

/// A complete doc for a published project, merged with `fields`.
fn full(committee: Committee, name: &str, fields: Value) -> Value {
    let mut doc = testing::project_doc(committee, name);
    doc["summary"] = json!(format!("{name} in one line"));
    doc["description"] = json!(format!("<p>All about {name}.</p>"));
    doc["media"] = json!([cover()]);
    if let (Some(doc), Some(fields)) = (doc.as_object_mut(), fields.as_object()) {
        for (key, value) in fields {
            doc.insert(key.clone(), value.clone());
        }
    }
    doc
}

/// `base` with some fields replaced.
fn with(base: &Value, fields: Value) -> Value {
    let mut doc = base.clone();
    if let (Some(doc), Some(fields)) = (doc.as_object_mut(), fields.as_object()) {
        for (key, value) in fields {
            doc.insert(key.clone(), value.clone());
        }
    }
    doc
}

async fn published(db: &PgPool, committee: Committee, slug: &str, name: &str) -> Uuid {
    testing::published_project(db, committee, slug, full(committee, name, json!({}))).await
}

async fn editor(app: &axum::Router, user: &TestUser, id: Uuid) -> Value {
    let (status, body) = testing::get(app, &format!("/api/projects/{id}"), Some(user)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["project"].clone()
}

async fn save(app: &axum::Router, user: &TestUser, id: Uuid, doc: Value) -> (StatusCode, Value) {
    testing::put(
        app,
        &format!("/api/projects/{id}/draft"),
        Some(user),
        json!({ "doc": doc }),
    )
    .await
}

async fn act(app: &axum::Router, user: &TestUser, id: Uuid, action: &str) -> (StatusCode, Value) {
    testing::post(
        app,
        &format!("/api/projects/{id}/{action}"),
        Some(user),
        json!({}),
    )
    .await
}

/// A viewer for calling `apply_approval` directly, as routes/approvals.rs does.
fn as_viewer(user: &TestUser, roles: &[(Role, Option<Committee>, bool)]) -> Viewer {
    let mut viewer = viewer(roles, vec![]);
    viewer.user_id = user.id;
    viewer
}

fn names(body: &Value) -> Vec<String> {
    body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

struct People {
    admin: TestUser,
    director: TestUser,
    tech_director: TestUser,
    member: TestUser,
}

async fn people(db: &PgPool) -> People {
    People {
        admin: UserSpec::new("admin").admin().create(db).await,
        director: UserSpec::new("evdir")
            .name("Evan Foster")
            .director(Committee::Events)
            .create(db)
            .await,
        tech_director: UserSpec::new("techdir")
            .director(Committee::Tech)
            .create(db)
            .await,
        member: UserSpec::new("member").create(db).await,
    }
}

// ---------------------------------------------------------------------------
// Public

#[sqlx::test(migrations = "./migrations")]
async fn visitors_see_published_projects_featured_first(db: PgPool) {
    let app = testing::app(db.clone()).await;
    published(&db, Committee::Tech, "zeta", "Zeta").await;
    let featured = published(&db, Committee::Events, "tartanhacks", "TartanHacks").await;
    sqlx::query("UPDATE projects SET featured = true WHERE id = $1")
        .bind(featured)
        .execute(&db)
        .await
        .unwrap();
    published(&db, Committee::Design, "alpha", "alpha").await;
    testing::project(&db, Committee::Tech, "draft-only").await;
    let archived = published(&db, Committee::Tech, "old", "Old").await;
    sqlx::query("UPDATE projects SET state = 'archived' WHERE id = $1")
        .bind(archived)
        .execute(&db)
        .await
        .unwrap();

    let res = Req::get("/api/public/projects").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(names(&res.json), ["TartanHacks", "alpha", "Zeta"]);
    assert_eq!(
        res.header("cache-control"),
        Some("public, max-age=60, stale-while-revalidate=600")
    );
    assert_eq!(res.header("access-control-allow-origin"), Some("*"));

    let card = &res.json["projects"][0];
    assert_eq!(card["featured"], true);
    assert_eq!(card["excerpt"], "All about TartanHacks.");
    assert_eq!(card["cover"]["src"], "/seed/projects/maps.jpg");
    assert_eq!(card["committee"], "events");
    assert_eq!(card["videoUrl"], Value::Null);
    assert!(card.get("description").is_none());

    let (status, body) = testing::get(&app, "/api/public/projects?committee=tech", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), ["Zeta"]);
    let (status, body) = testing::get(&app, "/api/public/projects?committee=marketing", None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"]["fields"]["committee"],
        "Choose one of the seven committees."
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_move_shows_on_committee_lists_only_once_published(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("admin").admin().create(&db).await;
    published(&db, Committee::Events, "tartanhacks", "TartanHacks").await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let lists = |app: axum::Router| async move {
        let (_, events) = testing::get(&app, "/api/public/projects?committee=events", None).await;
        let (_, tech) = testing::get(&app, "/api/public/projects?committee=tech", None).await;
        (names(&events), names(&tech))
    };

    let (status, body) = save(&app, &admin, nova, full(Committee::Tech, "Nova", json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        lists(app.clone()).await,
        (vec!["Nova".to_owned(), "TartanHacks".to_owned()], vec![])
    );
    let cards = super::public::published_cards(&db, Some(Committee::Tech))
        .await
        .unwrap();
    assert!(cards.is_empty());

    let (status, _) = act(&app, &admin, nova, "publish").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        lists(app.clone()).await,
        (vec!["TartanHacks".to_owned()], vec!["Nova".to_owned()])
    );
    let (_, page) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(page["project"]["committee"], "tech");
}

#[sqlx::test(migrations = "./migrations")]
async fn visitors_open_project_pages_and_follow_renames(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("admin").admin().create(&db).await;
    let id = testing::published_project(
        &db,
        Committee::Tech,
        "cmu-maps",
        full(
            Committee::Tech,
            "CMU Maps",
            json!({ "builtWith": ["design"], "videoUrl": "https://youtu.be/dQw4w9WgXcQ", "siteUrl": "https://cmumaps.com/" }),
        ),
    )
    .await;
    testing::project(&db, Committee::Tech, "secret-draft").await;

    let res = Req::get("/api/public/projects/cmu-maps").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.header("access-control-allow-origin"), Some("*"));
    assert!(res.header("cache-control").unwrap().starts_with("public"));
    let project = &res.json["project"];
    assert_eq!(project["name"], "CMU Maps");
    assert_eq!(project["description"], "<p>All about CMU Maps.</p>");
    assert_eq!(project["builtWith"], json!(["design"]));
    assert_eq!(project["videoUrl"], "https://youtu.be/dQw4w9WgXcQ");
    // The card brings the video link, so it is sent once.
    assert_eq!(res.text().matches("\"videoUrl\"").count(), 1);
    assert_eq!(project["media"].as_array().unwrap().len(), 1);
    let (_, list) = testing::get(&app, "/api/public/projects", None).await;
    assert_eq!(
        list["projects"][0]["videoUrl"],
        "https://youtu.be/dQw4w9WgXcQ"
    );
    assert!(project["publishedAt"].is_string());
    assert_eq!(res.json["redirectTo"], Value::Null);

    for missing in ["secret-draft", "nothing-here", "Bad%20Slug", "UPPER"] {
        let res = Req::get(&format!("/api/public/projects/{missing}"))
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(res.code(), "not_found");
        assert_eq!(res.header("access-control-allow-origin"), Some("*"));
    }

    let (status, _) = testing::patch(
        &app,
        &format!("/api/projects/{id}/settings"),
        Some(&admin),
        json!({ "slug": "maps" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = testing::get(&app, "/api/public/projects/cmu-maps", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"], Value::Null);
    assert_eq!(body["redirectTo"], "/projects/maps/");
    let (status, body) = testing::get(&app, "/api/public/projects/maps", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"]["slug"], "maps");
}

// ---------------------------------------------------------------------------
// The portal list

#[sqlx::test(migrations = "./migrations")]
async fn the_list_is_scoped_to_what_people_can_open(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    published(&db, Committee::Tech, "cmu-maps", "CMU Maps").await;
    testing::project(&db, Committee::Events, "giving-day").await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;

    let (status, body) = testing::get(&app, "/api/projects", Some(&p.admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["projects"].as_array().unwrap().len(), 3);
    assert!(
        body["projects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["canEdit"] == true)
    );

    let (_, body) = testing::get(&app, "/api/projects", Some(&p.director)).await;
    let rows = body["projects"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for row in rows {
        let events = row["committee"] == "events";
        assert_eq!(row["canEdit"], events, "{row}");
        assert_eq!(row["canPublish"], events, "{row}");
    }

    let (_, body) = testing::get(&app, "/api/projects", Some(&lead)).await;
    assert_eq!(names(&body), ["Nova"]);
    let row = &body["projects"][0];
    assert_eq!(row["canEdit"], true);
    assert_eq!(row["canPublish"], false);
    assert_eq!(row["state"], "published");
    assert_eq!(row["hasRepo"], false);
    assert_eq!(row["cover"]["src"], "/seed/projects/maps.jpg");

    let (status, _) = testing::get(&app, "/api/projects", Some(&p.member)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = testing::get(&app, "/api/projects", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, body) = testing::get(&app, "/api/projects?committee=tech", Some(&p.admin)).await;
    assert_eq!(names(&body), ["CMU Maps"]);
    let (_, body) = testing::get(&app, "/api/projects?state=draft", Some(&p.admin)).await;
    assert_eq!(names(&body), ["giving-day"]);
    let (_, body) = testing::get(&app, "/api/projects?q=MAPS", Some(&p.admin)).await;
    assert_eq!(names(&body), ["CMU Maps"]);
    let (_, body) = testing::get(&app, "/api/projects?q=100%25", Some(&p.admin)).await;
    assert!(body["projects"].as_array().unwrap().is_empty());
    let (status, body) = testing::get(&app, "/api/projects?state=deleted", Some(&p.admin)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["fields"]["state"].is_string());

    // Unpublished changes and waiting approvals show on the row.
    let (status, _) = save(
        &app,
        &lead,
        nova,
        full(Committee::Events, "Nova 2", json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = act(&app, &lead, nova, "publish").await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = testing::get(
        &app,
        "/api/projects?committee=events&state=published",
        Some(&p.director),
    )
    .await;
    let row = &body["projects"][0];
    assert_eq!(row["name"], "Nova 2");
    assert_eq!(row["hasUnpublishedChanges"], true);
    assert_eq!(row["pendingApproval"], true);
    assert_eq!(row["updatedBy"]["andrewId"], "sara");
}

// ---------------------------------------------------------------------------
// Adding projects

#[sqlx::test(migrations = "./migrations")]
async fn directors_and_admins_add_projects_with_unique_addresses(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    let taken = published(&db, Committee::Events, "nova", "Nova").await;
    sqlx::query("INSERT INTO project_slug_redirects (old_slug, project_id) VALUES ('nova-2', $1)")
        .bind(taken)
        .execute(&db)
        .await
        .unwrap();

    let res = Req::post("/api/projects")
        .user(&p.director)
        .json(json!({ "name": "  Nova  ", "committee": "events" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let project = &res.json["project"];
    // nova is taken and nova-2 used to belong to another project.
    assert_eq!(project["slug"], "nova-3");
    assert_eq!(project["state"], "draft");
    assert_eq!(project["draft"]["name"], "Nova");
    assert_eq!(project["draft"]["committee"], "events");
    assert_eq!(project["published"], Value::Null);
    assert_eq!(project["canPublish"], true);
    assert_eq!(project["checklist"]["cover"], false);
    assert_eq!(project["approvers"][0]["name"], "Evan Foster");

    let (status, body) = testing::post(
        &app,
        "/api/projects",
        Some(&p.director),
        json!({ "name": "Maps", "committee": "tech" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("director of Tech")
    );
    for (user, committee) in [(&lead, "events"), (&p.member, "events")] {
        let (status, _) = testing::post(
            &app,
            "/api/projects",
            Some(user),
            json!({ "name": "Maps", "committee": committee }),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, body) = testing::post(
        &app,
        "/api/projects",
        Some(&p.admin),
        json!({ "name": "Sara\u{2019}s CMU Maps!", "committee": "tech" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["project"]["slug"], "saras-cmu-maps");

    for name in ["", "   ", &"x".repeat(61)] {
        let (status, body) = testing::post(
            &app,
            "/api/projects",
            Some(&p.admin),
            json!({ "name": name, "committee": "tech" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{name:?}");
        assert!(body["error"]["fields"]["name"].is_string());
    }
    let (status, _) = testing::post(
        &app,
        "/api/projects",
        Some(&p.admin),
        json!({ "name": "X", "committee": "marketing" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let res = Req::post("/api/projects")
        .cookie(&p.admin)
        .json(json!({ "name": "X", "committee": "tech" }))
        .send(&app)
        .await;
    assert_eq!(
        res.status,
        StatusCode::FORBIDDEN,
        "writes need the CSRF token"
    );

    let summaries = testing::audit_summaries(&db).await;
    assert!(summaries.contains(&"Added the project Nova".to_owned()));
}

// ---------------------------------------------------------------------------
// Opening and editing

#[sqlx::test(migrations = "./migrations")]
async fn opening_a_project_follows_the_view_rules(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let maps = published(&db, Committee::Tech, "cmu-maps", "CMU Maps").await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;

    let project = editor(&app, &p.director, maps).await;
    assert_eq!(project["canEdit"], false);
    assert_eq!(project["canArchive"], false);
    assert_eq!(project["canManageEditors"], false);
    let project = editor(&app, &p.director, nova).await;
    assert_eq!(project["canEdit"], true);
    assert_eq!(project["canManageEditors"], true);
    assert_eq!(project["editors"][0]["andrewId"], "sara");
    assert_eq!(project["hasUnpublishedChanges"], false);
    assert_eq!(
        project["checklist"],
        json!({
            "cover": true, "coverWide": true, "summary": true, "description": true,
            "links": false, "altText": true
        })
    );

    let project = editor(&app, &lead, nova).await;
    assert_eq!(project["canEdit"], true);
    assert_eq!(project["canPublish"], false);
    assert_eq!(project["canArchive"], false);
    let (status, _) = testing::get(&app, &format!("/api/projects/{maps}"), Some(&lead)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = testing::get(&app, &format!("/api/projects/{nova}"), Some(&p.member)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = testing::get(
        &app,
        &format!("/api/projects/{}", Uuid::now_v7()),
        Some(&p.admin),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = testing::get(&app, "/api/projects/not-an-id", Some(&p.admin)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[sqlx::test(migrations = "./migrations")]
async fn drafts_are_checked_cleaned_and_kept_from_visitors(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let base = full(Committee::Events, "Nova", json!({}));

    let (status, body) = save(
        &app,
        &p.director,
        nova,
        with(
            &base,
            json!({
                "name": " Nova ",
                "description": "<p onclick=\"x()\">Teams <strong>build</strong><script>alert(1)</script></p><img src=x>",
                "builtWith": ["events", "tech", "design", "tech"],
                "repoUrl": "https://git.cmu.dev/ScottyLabs/whistle",
                "siteUrl": " ",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let draft = &body["project"]["draft"];
    assert_eq!(draft["name"], "Nova");
    assert_eq!(draft["description"], "<p>Teams <strong>build</strong></p>");
    assert_eq!(draft["builtWith"], json!(["tech", "design"]));
    assert_eq!(draft["siteUrl"], Value::Null);
    assert_eq!(body["project"]["hasUnpublishedChanges"], true);
    assert_eq!(body["project"]["checklist"]["links"], true);

    // Visitors still see the published version until it is published again.
    let (_, public) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(public["project"]["description"], "<p>All about Nova.</p>");

    let (status, body) = save(
        &app,
        &p.director,
        nova,
        with(
            &base,
            json!({
                "name": "",
                "summary": "x".repeat(49),
                "siteUrl": "http://nova.example",
                "videoUrl": "https://example.com/v.mp4",
                "media": [{ "src": format!("/media/{}", Uuid::now_v7()), "alt": "" }],
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body["error"]["fields"].as_object().unwrap();
    for field in ["name", "summary", "siteUrl", "videoUrl", "media.0"] {
        assert!(fields.contains_key(field), "{field} in {body}");
    }
    let many: Vec<Value> = (0..9).map(|_| cover()).collect();
    let (status, body) = save(
        &app,
        &p.director,
        nova,
        with(&base, json!({ "media": many })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["fields"]["media"].is_string());
    let (status, _) = save(&app, &p.director, nova, json!({ "name": "Nova" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Uploaded photos must exist; their size comes from the media table.
    let media_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO media (id, content_type, width, height, variants, bytes, storage)
         VALUES ($1, 'image/jpeg', 1200, 800, '{480,960,1200}', 1000, 'database')",
    )
    .bind(media_id)
    .execute(&db)
    .await
    .unwrap();
    let upload =
        json!({ "src": format!("/media/{media_id}"), "alt": "Hackers", "width": 5, "height": 5 });
    let (status, body) = save(
        &app,
        &p.director,
        nova,
        with(&base, json!({ "media": [upload] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"]["draft"]["media"][0]["width"], 1200);
    assert_eq!(body["project"]["draft"]["media"][0]["height"], 800);

    // Discard brings back the published version.
    let (status, body) = act(&app, &p.director, nova, "discard").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["project"]["draft"]["description"],
        "<p>All about Nova.</p>"
    );
    assert_eq!(body["project"]["hasUnpublishedChanges"], false);
    let draft_only = testing::project(&db, Committee::Events, "fresh").await;
    let (status, _) = act(&app, &p.director, draft_only, "discard").await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn descriptions_with_too_much_markup_are_refused_at_once(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let base = full(Committee::Events, "Nova", json!({}));

    // Thousands of unclosed tables take the HTML parser time that grows with the square of their number, so they are refused before parsing.
    let started = std::time::Instant::now();
    let (status, body) = save(
        &app,
        &p.director,
        nova,
        with(&base, json!({ "description": "<table>x".repeat(31_000) })),
    )
    .await;
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["error"]["fields"]["description"],
        "The description has too much formatting to save. Remove some of it and try again."
    );

    // Someone who may not edit the project still gets 403 first.
    let (status, _) = save(
        &app,
        &p.member,
        nova,
        with(&base, json!({ "description": "<table>x".repeat(31_000) })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_editors_save_and_only_publishers_move_projects(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;
    let other_lead = UserSpec::new("other")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    let base = full(Committee::Events, "Nova", json!({}));

    for user in [&p.tech_director, &other_lead, &p.member] {
        let (status, _) = save(&app, user, nova, base.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, _) = save(
        &app,
        &lead,
        nova,
        with(&base, json!({ "summary": "Teams and demos" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let moved = with(&base, json!({ "committee": "tech" }));
    for user in [&lead, &p.director] {
        let (status, body) = save(&app, user, nova, moved.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let (status, body) = save(&app, &p.admin, nova, moved).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"]["draft"]["committee"], "tech");
    let committee: String = sqlx::query_scalar("SELECT committee FROM projects WHERE id = $1")
        .bind(nova)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(committee, "tech");
    // Sara is a lead in Events, so a Tech project is no longer hers to edit.
    let (status, _) = testing::get(&app, &format!("/api/projects/{nova}"), Some(&lead)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let project = editor(&app, &p.tech_director, nova).await;
    assert_eq!(project["canPublish"], true);
}

#[sqlx::test(migrations = "./migrations")]
async fn discarding_a_move_is_for_those_who_can_move_it_back(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let owner = |db: PgPool| async move {
        sqlx::query_scalar::<_, String>("SELECT committee FROM projects WHERE id = $1")
            .bind(nova)
            .fetch_one(&db)
            .await
            .unwrap()
    };
    let (status, _) = save(
        &app,
        &p.admin,
        nova,
        full(Committee::Tech, "Nova", json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The Tech director has the project now, but cannot hand it back to Events.
    let (status, body) = act(&app, &p.tech_director, nova, "discard").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["error"]["message"],
        "Discarding would move Nova back to Events, and only admins and the director of Events can move a project there."
    );
    assert_eq!(owner(db.clone()).await, "tech");

    // An admin can move it back.
    let (status, body) = act(&app, &p.admin, nova, "discard").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"]["draft"]["committee"], "events");
    assert_eq!(owner(db.clone()).await, "events");

    // Discarding edits that keep the committee needs only edit access.
    let (status, _) = save(
        &app,
        &p.director,
        nova,
        full(
            Committee::Events,
            "Nova",
            json!({ "summary": "Teams and demos" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = act(&app, &p.director, nova, "discard").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Publishing

#[sqlx::test(migrations = "./migrations")]
async fn publishing_needs_a_described_cover_and_a_summary(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let (_, body) = testing::post(
        &app,
        "/api/projects",
        Some(&p.director),
        json!({ "name": "Giving Day", "committee": "events" }),
    )
    .await;
    let id: Uuid = body["project"]["id"].as_str().unwrap().parse().unwrap();
    let draft = body["project"]["draft"].clone();

    let (status, body) = act(&app, &p.director, id, "publish").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body["error"]["fields"].as_object().unwrap();
    assert!(fields.contains_key("media") && fields.contains_key("summary"));

    let undescribed = with(
        &draft,
        json!({ "summary": "Support for builders", "media": [{ "src": "/seed/photos/c15.jpg", "alt": " " }] }),
    );
    let (status, _) = save(&app, &p.director, id, undescribed).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = act(&app, &p.director, id, "publish").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["fields"]["media.0.alt"].is_string());
    assert_eq!(body["error"]["fields"].as_object().unwrap().len(), 1);

    // An uploaded cover must be at least 1,200 pixels wide; bundled images are exempt.
    let narrow = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO media (id, content_type, width, height, variants, bytes, storage)
         VALUES ($1, 'image/jpeg', 600, 400, '{480,600}', 1000, 'database')",
    )
    .bind(narrow)
    .execute(&db)
    .await
    .unwrap();
    let small = with(
        &draft,
        json!({ "summary": "Support for builders", "media": [{ "src": format!("/media/{narrow}"), "alt": "Five members" }] }),
    );
    let (status, body) = save(&app, &p.director, id, small).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "drafts may hold a small photo: {body}"
    );
    assert_eq!(body["project"]["checklist"]["coverWide"], false);
    let (status, body) = act(&app, &p.director, id, "publish").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"]["fields"]["media.0"],
        "Use a cover photo at least 1,200 pixels wide. This one is 600 pixels wide."
    );

    let ready = with(
        &draft,
        json!({ "summary": "Support for builders", "media": [{ "src": "/seed/photos/c15.jpg", "alt": "Five members" }] }),
    );
    let (status, _) = save(&app, &p.director, id, ready).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = act(&app, &p.director, id, "publish").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"]["state"], "published");
    assert_eq!(
        body["project"]["published"]["summary"],
        "Support for builders"
    );
    assert_eq!(body["project"]["hasUnpublishedChanges"], false);

    let (status, public) = testing::get(&app, "/api/public/projects/giving-day", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(public["project"]["name"], "Giving Day");

    // Publishing again without changes is harmless.
    let (status, _) = act(&app, &p.director, id, "publish").await;
    assert_eq!(status, StatusCode::OK);
    for user in [&p.tech_director, &p.member] {
        let (status, _) = act(&app, user, id, "publish").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let summaries = testing::audit_summaries(&db).await;
    assert_eq!(
        summaries
            .iter()
            .filter(|s| *s == "Published Giving Day")
            .count(),
        1
    );
    // The run of edits is one entry.
    assert_eq!(
        summaries
            .iter()
            .filter(|s| *s == "Edited Giving Day")
            .count(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn archiving_hides_a_project_without_deleting_it(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;

    for user in [&lead, &p.tech_director] {
        let (status, _) = act(&app, user, nova, "archive").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, body) = act(&app, &p.director, nova, "archive").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"]["state"], "archived");
    let (status, _) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = testing::get(&app, "/api/public/projects", None).await;
    assert!(list["projects"].as_array().unwrap().is_empty());

    for user in [&p.director, &lead] {
        let (status, body) = act(&app, user, nova, "publish").await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
    }
    let (status, _) = act(&app, &lead, nova, "unarchive").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = act(&app, &p.admin, nova, "unarchive").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"]["state"], "published");
    let (status, _) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(status, StatusCode::OK);

    let draft = testing::project(&db, Committee::Events, "fresh").await;
    act(&app, &p.director, draft, "archive").await;
    let (_, body) = act(&app, &p.director, draft, "unarchive").await;
    assert_eq!(body["project"]["state"], "draft");
    let summaries = testing::audit_summaries(&db).await;
    assert!(summaries.contains(&"Archived Nova".to_owned()));
    assert!(summaries.contains(&"Unarchived Nova".to_owned()));
}

// ---------------------------------------------------------------------------
// Leads and approvals

#[sqlx::test(migrations = "./migrations")]
async fn a_lead_sends_for_approval_and_a_director_publishes_the_snapshot(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;
    let base = full(Committee::Events, "Nova", json!({}));

    // Nothing changed yet, so there is nothing to send.
    let (status, _) = act(&app, &lead, nova, "publish").await;
    assert_eq!(status, StatusCode::CONFLICT);

    let sent = with(&base, json!({ "summary": "Fall hackathon with demos" }));
    save(&app, &lead, nova, sent).await;
    let (status, body) = act(&app, &lead, nova, "publish").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let project = &body["project"];
    assert_eq!(project["state"], "published");
    assert_eq!(project["published"]["summary"], "Nova in one line");
    assert_eq!(project["pendingApproval"]["status"], "waiting");
    assert_eq!(project["hasUnpublishedChanges"], true);

    let approval =
        approvals_core::latest(&mut db.acquire().await.unwrap(), SubjectType::Project, nova)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(approval.committee, Committee::Events);
    let submitted_by: Option<Uuid> =
        sqlx::query_scalar("SELECT submitted_by FROM approvals WHERE id = $1")
            .bind(approval.id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(submitted_by, Some(lead.id));
    assert_eq!(approval.snapshot["summary"], "Fall hackathon with demos");

    // Editing again withdraws it; sending again opens a new one.
    save(
        &app,
        &lead,
        nova,
        with(&base, json!({ "summary": "Fall hackathon, demos" })),
    )
    .await;
    let status: String = sqlx::query_scalar("SELECT status FROM approvals WHERE id = $1")
        .bind(approval.id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "withdrawn");
    assert_eq!(
        editor(&app, &lead, nova).await["pendingApproval"],
        Value::Null
    );
    act(&app, &lead, nova, "publish").await;
    let mut conn = db.acquire().await.unwrap();
    let approval = approvals_core::latest(&mut conn, SubjectType::Project, nova)
        .await
        .unwrap()
        .unwrap();

    // The director edits the draft afterwards; approving publishes what Sara sent and keeps the edit.
    save(
        &app,
        &p.director,
        nova,
        with(&base, json!({ "summary": "Director edit" })),
    )
    .await;
    let mut tx = db.begin().await.unwrap();
    let wrong = as_viewer(
        &p.tech_director,
        &[(Role::Director, Some(Committee::Tech), true)],
    );
    let refused = super::apply_approval(&mut tx, &approval, &wrong).await;
    assert!(matches!(refused, Err(crate::error::AppError::Forbidden(_))));
    let approver = as_viewer(
        &p.director,
        &[(Role::Director, Some(Committee::Events), true)],
    );
    super::apply_approval(&mut tx, &approval, &approver)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (_, public) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(public["project"]["summary"], "Fall hackathon, demos");
    let project = editor(&app, &p.director, nova).await;
    assert_eq!(project["draft"]["summary"], "Director edit");
    assert_eq!(project["hasUnpublishedChanges"], true);
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Approved and published Nova".to_owned())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_sent_back_project_keeps_its_draft_and_shows_the_note(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    let id = testing::project(&db, Committee::Events, "nova").await;
    sqlx::query("INSERT INTO project_editors (project_id, user_id) VALUES ($1, $2)")
        .bind(id)
        .bind(lead.id)
        .execute(&db)
        .await
        .unwrap();
    let ready = full(Committee::Events, "Nova", json!({}));
    save(&app, &lead, id, ready.clone()).await;
    let (status, body) = act(&app, &lead, id, "publish").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"]["state"], "draft");

    let mut tx = db.begin().await.unwrap();
    let approval = approvals_core::latest(&mut tx, SubjectType::Project, id)
        .await
        .unwrap()
        .unwrap();
    let approver = as_viewer(
        &p.director,
        &[(Role::Director, Some(Committee::Events), true)],
    );
    super::apply_send_back(&mut tx, &approval, &approver)
        .await
        .unwrap();
    // routes/approvals.rs records the decision on the approval.
    sqlx::query(
        "UPDATE approvals SET status = 'sent_back', note = 'Add the repository link.', decided_by = $2, decided_at = now()
         WHERE id = $1",
    )
    .bind(approval.id)
    .bind(p.director.id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let project = editor(&app, &lead, id).await;
    assert_eq!(project["state"], "draft");
    assert_eq!(project["draft"]["name"], "Nova");
    assert_eq!(project["pendingApproval"]["status"], "sent_back");
    assert_eq!(
        project["pendingApproval"]["note"],
        "Add the repository link."
    );
    assert_eq!(
        project["pendingApproval"]["decidedBy"]["name"],
        "Evan Foster"
    );
    let (status, _) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Sent Nova back with a note".to_owned())
    );

    // Once published, the old note no longer shows.
    act(&app, &p.director, id, "publish").await;
    assert_eq!(
        editor(&app, &lead, id).await["pendingApproval"],
        Value::Null
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn approving_checks_the_snapshot_again(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let id = published(&db, Committee::Events, "nova", "Nova").await;
    let lead = UserSpec::new("sara")
        .lead(Committee::Events, true)
        .edits(id)
        .create(&db)
        .await;
    let snapshot = full(Committee::Events, "Nova", json!({ "media": [] }));
    let mut conn = db.acquire().await.unwrap();
    let approval = approvals_core::submit(
        &mut conn,
        SubjectType::Project,
        id,
        Committee::Events,
        lead.id,
        &snapshot,
    )
    .await
    .unwrap();
    let approver = as_viewer(&p.admin, &[(Role::Admin, None, true)]);
    let result = super::apply_approval(&mut conn, &approval, &approver).await;
    assert!(matches!(
        result,
        Err(crate::error::AppError::Validation { .. })
    ));

    act(&app, &p.director, id, "archive").await;
    let good = approvals_core::submit(
        &mut conn,
        SubjectType::Project,
        id,
        Committee::Events,
        lead.id,
        &full(Committee::Events, "Nova", json!({})),
    )
    .await
    .unwrap();
    let result = super::apply_approval(&mut conn, &good, &approver).await;
    assert!(matches!(result, Err(crate::error::AppError::Conflict(_))));
}

// ---------------------------------------------------------------------------
// Settings and editors

#[sqlx::test(migrations = "./migrations")]
async fn admins_change_addresses_and_feature_one_project(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let maps = published(&db, Committee::Tech, "cmu-maps", "CMU Maps").await;
    let path = format!("/api/projects/{nova}/settings");

    let (status, _) =
        testing::patch(&app, &path, Some(&p.director), json!({ "featured": true })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) =
        testing::patch(&app, &path, Some(&p.admin), json!({ "slug": "cmu-maps" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["error"]["fields"]["slug"]
            .as_str()
            .unwrap()
            .contains("CMU Maps")
    );
    let (status, _) =
        testing::patch(&app, &path, Some(&p.admin), json!({ "slug": "Not a slug" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, body) = testing::patch(
        &app,
        &path,
        Some(&p.admin),
        json!({ "slug": "nova-hackathon", "featured": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"]["slug"], "nova-hackathon");
    assert_eq!(body["project"]["featured"], true);
    let redirect: Uuid =
        sqlx::query_scalar("SELECT project_id FROM project_slug_redirects WHERE old_slug = 'nova'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(redirect, nova);

    // Another project cannot take Nova's old address; Nova can go back to it.
    let (status, body) = testing::patch(
        &app,
        &format!("/api/projects/{maps}/settings"),
        Some(&p.admin),
        json!({ "slug": "nova", "featured": true }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, _) = testing::patch(
        &app,
        &format!("/api/projects/{maps}/settings"),
        Some(&p.admin),
        json!({ "featured": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let featured: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE featured")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(featured, vec![maps]);

    let (status, body) =
        testing::patch(&app, &path, Some(&p.admin), json!({ "slug": "nova" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"]["slug"], "nova");
    let (_, public) = testing::get(&app, "/api/public/projects/nova-hackathon", None).await;
    assert_eq!(public["redirectTo"], "/projects/nova/");
    let (status, public) = testing::get(&app, "/api/public/projects/nova", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(public["project"]["slug"], "nova");

    let summaries = testing::audit_summaries(&db).await;
    assert!(summaries.contains(&"Moved Nova to /projects/nova-hackathon/".to_owned()));
    assert!(summaries.contains(&"Featured Nova on the Projects page".to_owned()));
}

#[sqlx::test(migrations = "./migrations")]
async fn featuring_two_projects_at_once_leaves_one_featured(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("admin").admin().create(&db).await;
    let other_admin = UserSpec::new("admin2").admin().create(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let maps = published(&db, Committee::Tech, "cmu-maps", "CMU Maps").await;
    async fn feature(app: &axum::Router, user: &TestUser, id: Uuid) -> StatusCode {
        let path = format!("/api/projects/{id}/settings");
        testing::patch(app, &path, Some(user), json!({ "featured": true }))
            .await
            .0
    }

    for _ in 0..5 {
        let (first, second) = tokio::join!(
            feature(&app, &admin, nova),
            feature(&app, &other_admin, maps)
        );
        assert_eq!((first, second), (StatusCode::OK, StatusCode::OK));
        let featured: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE featured")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(featured, 1);
        sqlx::query("UPDATE projects SET featured = false")
            .execute(&db)
            .await
            .unwrap();
    }

    // The database refuses a second featured project however it is written.
    sqlx::query("UPDATE projects SET featured = true WHERE id = $1")
        .bind(nova)
        .execute(&db)
        .await
        .unwrap();
    let second = sqlx::query("UPDATE projects SET featured = true WHERE id = $1")
        .bind(maps)
        .execute(&db)
        .await;
    assert!(second.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn directors_choose_leads_of_their_committee_as_editors(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let p = people(&db).await;
    let nova = published(&db, Committee::Events, "nova", "Nova").await;
    let sara = UserSpec::new("sara")
        .name("Sara Chokshi")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    let tech_lead = UserSpec::new("techlead")
        .lead(Committee::Tech, true)
        .create(&db)
        .await;
    let path = format!("/api/projects/{nova}/editors");

    let (status, _) = testing::get(&app, &format!("/api/projects/{nova}"), Some(&sara)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    for user in [&p.tech_director, &sara, &p.member] {
        let (status, _) =
            testing::put(&app, &path, Some(user), json!({ "userIds": [sara.id] })).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    for user_id in [tech_lead.id, p.member.id, Uuid::now_v7()] {
        let (status, body) = testing::put(
            &app,
            &path,
            Some(&p.director),
            json!({ "userIds": [user_id] }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"]["fields"]["userIds"].is_string());
    }
    let (status, body) = testing::put(
        &app,
        &path,
        Some(&p.director),
        json!({ "userIds": [sara.id, sara.id] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["project"]["editors"][0]["name"], "Sara Chokshi");
    let project = editor(&app, &sara, nova).await;
    assert_eq!(project["canEdit"], true);

    let (status, body) = testing::put(&app, &path, Some(&p.admin), json!({ "userIds": [] })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["project"]["editors"].as_array().unwrap().is_empty());
    let (status, _) = testing::get(&app, &format!("/api/projects/{nova}"), Some(&sara)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Changed who can edit Nova".to_owned())
    );
}

// ---------------------------------------------------------------------------
// Audit

#[sqlx::test(migrations = "./migrations")]
async fn changes_are_audited_against_the_project(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let director = UserSpec::new("evdir")
        .director(Committee::Events)
        .create(&db)
        .await;
    let (_, body) = testing::post(
        &app,
        "/api/projects",
        Some(&director),
        json!({ "name": "Nova", "committee": "events" }),
    )
    .await;
    let id = body["project"]["id"].as_str().unwrap().to_owned();
    let rows: Vec<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT subject_type, subject_id, committee FROM audit_log")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![("project".to_owned(), Some(id), Some("events".to_owned()))]
    );
}

// ---------------------------------------------------------------------------
// Seed

#[sqlx::test(migrations = "./migrations")]
async fn the_seed_publishes_valid_projects_once(db: PgPool) {
    let config = testing::config();
    crate::seed::run(&db, &config).await.unwrap();

    let docs: Vec<(String, bool, sqlx::types::Json<ProjectDoc>)> = sqlx::query_as(
        "SELECT slug, featured, published FROM projects WHERE state = 'published' ORDER BY slug",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(docs.len(), 15);
    let mut conn = db.acquire().await.unwrap();
    for (slug, _, doc) in &docs {
        // Terrier has no picture yet; like the design, its card shows the monogram.
        let purpose = if doc.media.is_empty() {
            Purpose::Save
        } else {
            Purpose::Publish
        };
        let checked = doc::validate(&mut conn, doc.0.clone(), purpose).await;
        assert!(checked.is_ok(), "{slug}: {checked:?}");
        assert_eq!(&checked.unwrap(), &doc.0, "{slug} is stored clean");
    }
    let featured: Vec<&str> = docs
        .iter()
        .filter(|(_, f, _)| *f)
        .map(|(s, _, _)| s.as_str())
        .collect();
    assert_eq!(featured, ["tartanhacks"]);
    let tartanhacks = &docs.iter().find(|(s, _, _)| s == "tartanhacks").unwrap().2;
    assert_eq!(tartanhacks.media.len(), 6);

    let sara_edits_nova: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM project_editors pe JOIN projects p ON p.id = pe.project_id JOIN users u ON u.id = pe.user_id
             WHERE p.slug = 'nova' AND u.andrew_id = 'spchoksh'
         )",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(sara_edits_nova);

    // A renamed project is not seeded again under its old address, and edits are kept.
    sqlx::query("UPDATE projects SET slug = 'nova-hackathon' WHERE slug = 'nova'")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO project_slug_redirects (old_slug, project_id) SELECT 'nova', id FROM projects WHERE slug = 'nova-hackathon'",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("UPDATE projects SET draft = jsonb_set(draft, '{summary}', '\"Edited\"') WHERE slug = 'cmu-maps'")
        .execute(&db)
        .await
        .unwrap();
    crate::seed::run(&db, &config).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 15);
    let summary: String =
        sqlx::query_scalar("SELECT draft->>'summary' FROM projects WHERE slug = 'cmu-maps'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(summary, "Edited");
}

#[sqlx::test(migrations = "./migrations")]
async fn the_seed_features_tartanhacks_only_when_nothing_else_is(db: PgPool) {
    let mine = published(&db, Committee::Tech, "my-app", "My app").await;
    sqlx::query("UPDATE projects SET featured = true WHERE id = $1")
        .bind(mine)
        .execute(&db)
        .await
        .unwrap();
    crate::seed::run(&db, &testing::config()).await.unwrap();
    let featured: Vec<String> = sqlx::query_scalar("SELECT slug FROM projects WHERE featured")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(featured, ["my-app"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_seed_leaves_out_demo_editors_without_demo_content(db: PgPool) {
    let mut config = testing::config();
    config.seed_demo = false;
    crate::seed::run(&db, &config).await.unwrap();
    let editors: i64 = sqlx::query_scalar("SELECT count(*) FROM project_editors")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(editors, 0);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 15);
}

#[test]
fn the_doc_type_reads_what_the_testing_helpers_write() {
    let doc: ProjectDoc =
        serde_json::from_value(testing::project_doc(Committee::Tech, "x")).unwrap();
    assert_eq!(doc.committee, Committee::Tech);
}
