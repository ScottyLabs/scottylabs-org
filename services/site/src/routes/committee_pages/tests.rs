//! The committee page endpoints against a real database, with the seeded pages and personas.

use axum::Router;
use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Committee, Role};
use crate::testing::{self, Req, TestUser, UserSpec};

/// The router with the seven seeded pages and the dev personas (Thomas admin, Evan director of Events, Anish director of Tech, Sara lead in Events, New Member).
async fn setup(db: &PgPool) -> Router {
    crate::seed::run(db, &testing::config()).await.unwrap();
    testing::app(db.clone()).await
}

async fn editor(app: &Router, user: &TestUser, slug: &str) -> Value {
    let (status, body) =
        testing::get(app, &format!("/api/committee-pages/{slug}"), Some(user)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn save(
    app: &Router,
    user: &TestUser,
    slug: &str,
    doc: &Value,
    version: &Value,
) -> (StatusCode, Value) {
    testing::put(
        app,
        &format!("/api/committee-pages/{slug}/draft"),
        Some(user),
        json!({ "doc": doc, "baseVersion": version }),
    )
    .await
}

/// The Tech draft with Stats moved above What we do.
fn stats_first(doc: &Value) -> Value {
    let mut doc = doc.clone();
    let blocks = doc["blocks"].as_array_mut().unwrap();
    let stats = blocks.iter().position(|b| b["type"] == "stats").unwrap();
    let block = blocks.remove(stats);
    blocks.insert(1, block);
    doc
}

#[sqlx::test(migrations = "./migrations")]
async fn the_public_index_lists_published_committees_in_order(db: PgPool) {
    let app = setup(&db).await;
    let res = Req::get("/api/public/committees").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.header("access-control-allow-origin"), Some("*"));
    assert_eq!(
        res.header("cache-control"),
        Some("public, max-age=60, stale-while-revalidate=600")
    );
    let committees = res.json["committees"].as_array().unwrap();
    let slugs: Vec<&str> = committees
        .iter()
        .map(|c| c["slug"].as_str().unwrap())
        .collect();
    assert_eq!(
        slugs,
        [
            "events", "outreach", "tech", "labrador", "design", "finance", "foundry"
        ]
    );
    let tech = &committees[2];
    assert_eq!(tech["name"], "Tech");
    assert_eq!(tech["color"], "#EC9798");
    assert_eq!(tech["tagline"], "We build the apps CMU opens every day.");
    assert_eq!(tech["cover"]["src"], "/seed/photos/c10.jpg");
    assert_eq!(tech["meetingRoom"], "Tepper 2700, 2701, 2613");
    assert_eq!(tech["slackChannel"], "tech");
    assert_eq!(tech["mapUrl"], "https://maps.scottylabs.org/TEP-2700");

    // A page that was never published stays off the index.
    sqlx::query("UPDATE committee_pages SET published = NULL WHERE committee = 'design'")
        .execute(&db)
        .await
        .unwrap();
    let (_, body) = testing::get(&app, "/api/public/committees", None).await;
    assert_eq!(body["committees"].as_array().unwrap().len(), 6);
    let (status, _) = testing::get(&app, "/api/public/committees/design", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_public_page_has_its_blocks_and_published_projects(db: PgPool) {
    let app = setup(&db).await;
    let maps = testing::published_project(
        &db,
        Committee::Tech,
        "test-maps",
        json!({
            "name": "Test Maps", "summary": "Find any room",
            "description": "<p>Room-level navigation of <strong>campus</strong>.</p>",
            "status": "live", "kind": "app", "repoUrl": "https://git.cmu.dev/x",
            "media": [{ "src": "/seed/projects/maps.jpg", "alt": "A map", "width": 1440, "height": 900 }]
        }),
    )
    .await;
    testing::project(&db, Committee::Tech, "test-draft").await;
    testing::published_project(
        &db,
        Committee::Events,
        "test-events",
        json!({ "name": "Events thing" }),
    )
    .await;

    let res = Req::get("/api/public/committees/tech").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.header("access-control-allow-origin"), Some("*"));
    let body = res.json;
    assert_eq!(body["committee"]["slug"], "tech");
    assert_eq!(body["blocks"][0]["type"], "cover");
    assert_eq!(body["blocks"][2]["type"], "work_sessions");
    assert!(body["publishedAt"].is_string());
    let projects = body["projects"].as_array().unwrap();
    let card = projects.iter().find(|p| p["slug"] == "test-maps").unwrap();
    assert_eq!(card["id"], maps.to_string());
    assert_eq!(card["excerpt"], "Room-level navigation of campus.");
    assert_eq!(card["cover"]["src"], "/seed/projects/maps.jpg");
    assert_eq!(card["committee"], "tech");
    assert_eq!(card["kind"], "app");
    assert!(
        projects
            .iter()
            .all(|p| p["slug"] != "test-draft" && p["slug"] != "test-events")
    );

    for path in [
        "/api/public/committees/marketing",
        "/api/public/committees/TECH",
    ] {
        let (status, body) = testing::get(&app, path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn the_portal_list_says_who_can_edit_what(db: PgPool) {
    let app = setup(&db).await;
    let thomas = testing::persona(&db, "tkanz").await;
    let evan = testing::persona(&db, "evanfost").await;
    let sara = testing::persona(&db, "spchoksh").await;
    let member = testing::persona(&db, "newmember").await;

    let (status, body) = testing::get(&app, "/api/committee-pages", Some(&thomas)).await;
    assert_eq!(status, StatusCode::OK);
    let pages = body["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 7);
    assert!(
        pages
            .iter()
            .all(|p| p["canEdit"] == true && p["status"] == "published")
    );
    let tech = &pages[2];
    assert_eq!(tech["directors"][0]["andrewId"], "anishp");
    assert_eq!(tech["cover"]["src"], "/seed/photos/c10.jpg");

    let (_, body) = testing::get(&app, "/api/committee-pages", Some(&evan)).await;
    let editable: Vec<&str> = body["pages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["canEdit"] == true)
        .map(|p| p["slug"].as_str().unwrap())
        .collect();
    assert_eq!(editable, ["events"]);

    let (_, body) = testing::get(&app, "/api/committee-pages", Some(&sara)).await;
    assert!(
        body["pages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["canEdit"] == false)
    );
    let page = editor(&app, &sara, "events").await;
    assert_eq!(page["canEdit"], false);
    assert_eq!(page["draft"]["blocks"][0]["type"], "cover");

    let (status, _) = testing::get(&app, "/api/committee-pages", Some(&member)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = testing::get(&app, "/api/committee-pages", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = testing::get(&app, "/api/committee-pages/nope", Some(&thomas)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_admins_and_the_committees_director_can_write(db: PgPool) {
    let app = setup(&db).await;
    let evan = testing::persona(&db, "evanfost").await;
    let sara = testing::persona(&db, "spchoksh").await;
    let anish = testing::persona(&db, "anishp").await;
    let page = editor(&app, &anish, "tech").await;
    let doc = &page["draft"];

    for user in [&evan, &sara] {
        let (status, body) = save(&app, user, "tech", doc, &page["draftVersion"]).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(
            body["error"]["message"],
            "Only the director of Tech or an admin can edit this page."
        );
        for action in ["lock", "publish", "discard"] {
            let (status, _) = testing::post(
                &app,
                &format!("/api/committee-pages/tech/{action}"),
                Some(user),
                json!({ "draftVersion": 1 }),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{action}");
        }
        let (status, _) = testing::get(&app, "/api/committee-pages/tech/changes", Some(user)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, _) = save(&app, &anish, "tech", doc, &page["draftVersion"]).await;
    assert_eq!(status, StatusCode::OK);

    // Writes need the CSRF token.
    let res = Req::put("/api/committee-pages/tech/draft")
        .cookie(&anish)
        .json(json!({ "doc": doc, "baseVersion": 2 }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn one_person_edits_at_a_time(db: PgPool) {
    let app = setup(&db).await;
    let anish = testing::persona(&db, "anishp").await;
    let thomas = testing::persona(&db, "tkanz").await;

    let (status, lock) = testing::post(
        &app,
        "/api/committee-pages/tech/lock",
        Some(&anish),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lock["mine"], true);
    assert_eq!(lock["holder"]["andrewId"], "anishp");
    let since = lock["since"].clone();

    // Refreshing keeps the start time.
    let (_, again) = testing::post(
        &app,
        "/api/committee-pages/tech/lock",
        Some(&anish),
        json!({}),
    )
    .await;
    assert_eq!(again["since"], since);

    let (status, body) = testing::post(
        &app,
        "/api/committee-pages/tech/lock",
        Some(&thomas),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Anish Pallati is editing this page")
    );
    let page = editor(&app, &thomas, "tech").await;
    assert_eq!(page["lock"]["holder"]["name"], "Anish Pallati");
    assert_eq!(page["lock"]["mine"], false);
    let (status, _) = save(&app, &thomas, "tech", &page["draft"], &page["draftVersion"]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = testing::post(
        &app,
        "/api/committee-pages/tech/discard",
        Some(&thomas),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Letting go only works for the holder.
    let (status, _) = testing::delete(&app, "/api/committee-pages/tech/lock", Some(&thomas)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        editor(&app, &thomas, "tech").await["lock"]["holder"]["andrewId"],
        "anishp"
    );
    let (status, _) = testing::delete(&app, "/api/committee-pages/tech/lock", Some(&anish)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(editor(&app, &thomas, "tech").await["lock"].is_null());
    let (status, _) = testing::post(
        &app,
        "/api/committee-pages/tech/lock",
        Some(&thomas),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // A lock with no heartbeat for five minutes is free.
    sqlx::query("UPDATE committee_pages SET lock_heartbeat_at = now() - interval '6 minutes' WHERE committee = 'tech'")
        .execute(&db)
        .await
        .unwrap();
    assert!(editor(&app, &anish, "tech").await["lock"].is_null());
    let (status, lock) = testing::post(
        &app,
        "/api/committee-pages/tech/lock",
        Some(&anish),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lock["holder"]["andrewId"], "anishp");
}

#[sqlx::test(migrations = "./migrations")]
async fn saves_need_the_latest_version_and_a_valid_page(db: PgPool) {
    let app = setup(&db).await;
    let anish = testing::persona(&db, "anishp").await;
    let page = editor(&app, &anish, "tech").await;
    assert_eq!(page["status"], "published");
    let moved = stats_first(&page["draft"]);

    let (status, saved) = save(&app, &anish, "tech", &moved, &page["draftVersion"]).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["draftVersion"], 2);
    assert_eq!(saved["status"], "unpublished_changes");
    assert!(saved["savedAt"].is_string());

    // The same base version again is stale.
    let (status, body) = save(&app, &anish, "tech", &moved, &page["draftVersion"]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Reload")
    );

    let mut invalid = moved.clone();
    invalid["blocks"][1]["width"] = json!(4);
    invalid["settings"]["mapUrl"] = json!("http://maps.example.com");
    let (status, body) = save(&app, &anish, "tech", &invalid, &json!(2)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["fields"]["blocks.1.width"].is_string());
    assert!(body["error"]["fields"]["settings.mapUrl"].is_string());

    let mut missing_photo = moved.clone();
    missing_photo["blocks"][0]["props"]["image"]["src"] =
        json!(format!("/media/{}", Uuid::now_v7()));
    let (status, body) = save(&app, &anish, "tech", &missing_photo, &json!(2)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["fields"]["blocks.0.image"].is_string());

    // A channel plays no single video, so the page could not show it.
    let mut channel = moved.clone();
    channel["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "recap", "type": "video", "width": 12,
            "props": { "url": "https://www.youtube.com/@scottylabs", "title": "Channel", "poster": null } }));
    let last = channel["blocks"].as_array().unwrap().len() - 1;
    let (status, body) = save(&app, &anish, "tech", &channel, &json!(2)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["error"]["fields"][format!("blocks.{last}.url")],
        "Use a link to one YouTube or Vimeo video, not a channel or playlist."
    );

    // Going back to the published version shows as published again.
    let (status, saved) = save(&app, &anish, "tech", &page["draft"], &json!(2)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["status"], "published");

    // Autosave leaves one entry in the log, not one per save.
    let edits: Vec<String> = testing::audit_summaries(&db)
        .await
        .into_iter()
        .filter(|s| s.starts_with("Edited the draft"))
        .collect();
    assert_eq!(edits, ["Edited the draft of the Tech page"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn uploaded_photos_take_their_stored_size(db: PgPool) {
    let app = setup(&db).await;
    let anish = testing::persona(&db, "anishp").await;
    let media = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO media (id, content_type, width, height, variants, bytes, storage)
         VALUES ($1, 'image/jpeg', 1920, 1280, '{480,960,1920}', 1000, 'database')",
    )
    .bind(media)
    .execute(&db)
    .await
    .unwrap();
    let page = editor(&app, &anish, "tech").await;
    let mut doc = page["draft"].clone();
    doc["blocks"][0]["props"]["image"] =
        json!({ "src": format!("/media/{media}"), "alt": "Hackers", "width": 1, "height": 1 });
    let (status, _) = save(&app, &anish, "tech", &doc, &page["draftVersion"]).await;
    assert_eq!(status, StatusCode::OK);
    let cover = &editor(&app, &anish, "tech").await["draft"]["blocks"][0]["props"]["image"];
    assert_eq!(cover["width"], 1920);
    assert_eq!(cover["height"], 1280);
}

#[sqlx::test(migrations = "./migrations")]
async fn publishing_shows_the_changes_and_makes_them_public(db: PgPool) {
    let app = setup(&db).await;
    let anish = testing::persona(&db, "anishp").await;
    let page = editor(&app, &anish, "tech").await;

    let (_, body) = testing::get(&app, "/api/committee-pages/tech/changes", Some(&anish)).await;
    assert_eq!(body["changes"], json!([]));

    let mut doc = stats_first(&page["draft"]);
    doc["blocks"][2]["width"] = json!(12);
    let (_, saved) = save(&app, &anish, "tech", &doc, &page["draftVersion"]).await;
    let (_, body) = testing::get(&app, "/api/committee-pages/tech/changes", Some(&anish)).await;
    assert_eq!(
        body["changes"],
        json!(["Moved Stats above What we do", "Made What we do full width"])
    );

    // Visitors still see the published version.
    let (_, public) = testing::get(&app, "/api/public/committees/tech", None).await;
    assert_eq!(public["blocks"][1]["type"], "text");

    // Publishing an older version than the latest save is refused.
    let (status, _) = testing::post(
        &app,
        "/api/committee-pages/tech/publish",
        Some(&anish),
        json!({ "draftVersion": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, published) = testing::post(
        &app,
        "/api/committee-pages/tech/publish",
        Some(&anish),
        json!({ "draftVersion": saved["draftVersion"] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    assert_eq!(published["status"], "published");
    assert_eq!(published["published"], published["draft"]);
    let (_, public) = testing::get(&app, "/api/public/committees/tech", None).await;
    assert_eq!(public["blocks"][1]["type"], "stats");
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Published the Tech page".to_owned())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn publishing_needs_alt_text_and_a_cover_photo(db: PgPool) {
    let app = setup(&db).await;
    let thomas = testing::persona(&db, "tkanz").await;
    let page = editor(&app, &thomas, "design").await;
    let mut doc = page["draft"].clone();
    doc["blocks"][0]["props"]["image"] = Value::Null;
    doc["blocks"][1]["props"]["image"]["alt"] = json!("");
    let (status, saved) = save(&app, &thomas, "design", &doc, &page["draftVersion"]).await;
    assert_eq!(status, StatusCode::OK, "drafts may be incomplete: {saved}");

    let (status, body) = testing::post(
        &app,
        "/api/committee-pages/design/publish",
        Some(&thomas),
        json!({ "draftVersion": saved["draftVersion"] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body["error"]["fields"].as_object().unwrap();
    assert!(fields.contains_key("blocks.0.image"));
    assert!(fields.contains_key("blocks.1.image.alt"));
    let (_, public) = testing::get(&app, "/api/public/committees/design", None).await;
    assert!(public["committee"]["cover"].is_object());
}

#[sqlx::test(migrations = "./migrations")]
async fn discarding_brings_back_the_published_version(db: PgPool) {
    let app = setup(&db).await;
    let anish = testing::persona(&db, "anishp").await;
    let page = editor(&app, &anish, "tech").await;
    let (_, saved) = save(
        &app,
        &anish,
        "tech",
        &stats_first(&page["draft"]),
        &page["draftVersion"],
    )
    .await;
    assert_eq!(saved["status"], "unpublished_changes");

    let (status, body) = testing::post(
        &app,
        "/api/committee-pages/tech/discard",
        Some(&anish),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "published");
    assert_eq!(body["draft"], page["draft"]);
    assert_eq!(body["draftVersion"], 3);
    assert!(
        testing::audit_summaries(&db)
            .await
            .contains(&"Discarded the unpublished changes to the Tech page".to_owned())
    );

    // A page that was never published has nothing to go back to.
    sqlx::query("UPDATE committee_pages SET published = NULL WHERE committee = 'tech'")
        .execute(&db)
        .await
        .unwrap();
    let (status, _) = testing::post(
        &app,
        "/api/committee-pages/tech/discard",
        Some(&anish),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        editor(&app, &anish, "tech").await["status"],
        "never_published"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_director_of_two_committees_edits_both(db: PgPool) {
    let app = setup(&db).await;
    let both = UserSpec::new("twohats")
        .name("Two Hats")
        .director(Committee::Design)
        .director(Committee::Finance)
        .create(&db)
        .await;
    let (_, body) = testing::get(&app, "/api/committee-pages", Some(&both)).await;
    let editable: Vec<&str> = body["pages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["canEdit"] == true)
        .map(|p| p["slug"].as_str().unwrap())
        .collect();
    assert_eq!(editable, ["design", "finance"]);
    let lead = testing::user(&db, "techlead", &[(Role::Lead, Some(Committee::Tech))]).await;
    let (status, _) = testing::post(
        &app,
        "/api/committee-pages/tech/lock",
        Some(&lead),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
