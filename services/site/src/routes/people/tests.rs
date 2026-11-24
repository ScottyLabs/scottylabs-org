use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::*;
use crate::testing::{self, Req, Res, TestUser, UserSpec};

fn lead_input(andrew_id: &str, committee: &str, projects: &[Uuid]) -> Value {
    json!({
        "andrewId": andrew_id,
        "name": null,
        "role": "lead",
        "committee": committee,
        "canPostTicker": false,
        "projectIds": projects,
    })
}

async fn add(app: &Router, by: &TestUser, body: Value) -> Res {
    Req::post("/api/people").user(by).json(body).send(app).await
}

fn role_of<'a>(row: &'a Value, committee: &str) -> &'a Value {
    row["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["committee"] == committee)
        .unwrap_or(&Value::Null)
}

async fn assignment_id(db: &PgPool, andrew_id: &str, committee: Option<Committee>) -> Uuid {
    sqlx::query_scalar(
        "SELECT r.id FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE u.andrew_id = $1 AND r.committee IS NOT DISTINCT FROM $2",
    )
    .bind(andrew_id)
    .bind(committee)
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn directors_add_leads_with_their_projects(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let nova = testing::project(&db, Committee::Events, "nova").await;
    let courses = testing::project(&db, Committee::Tech, "cmu-courses").await;
    let director = UserSpec::new("evanfost")
        .name("Evan Foster")
        .director(Committee::Events)
        .create(&db)
        .await;

    let res = add(
        &app,
        &director,
        json!({
            "andrewId": " SPChoksh ", "name": "Sara Chokshi", "role": "lead", "committee": "events",
            "canPostTicker": true, "projectIds": [nova]
        }),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
    assert_eq!(res.json["person"]["andrewId"], "spchoksh");
    assert_eq!(res.json["person"]["name"], "Sara Chokshi");
    assert_eq!(res.json["person"]["hasSignedIn"], false);
    let role = role_of(&res.json, "events");
    assert_eq!(role["role"], "lead");
    assert_eq!(role["canPostTicker"], true);
    assert_eq!(role["projectIds"], json!([nova]));
    assert_eq!(role["appointedBy"]["andrewId"], "evanfost");
    assert_eq!(role["source"], "portal");
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Added Sara Chokshi as a lead in Events"]
    );

    // Projects must be owned by the lead's committee.
    let res = add(&app, &director, lead_input("someone", "events", &[courses])).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        res.json["error"]["fields"]["projectIds"]
            .as_str()
            .unwrap()
            .contains("owned by Events")
    );
    let res = add(
        &app,
        &director,
        lead_input("someone", "events", &[Uuid::now_v7()]),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Nothing was written for the rejected requests.
    let someone: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE andrew_id = 'someone'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(someone, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn directors_stay_in_their_lane(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let director = testing::user(
        &db,
        "laneevents",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;

    let other_committee = add(&app, &director, lead_input("x1", "tech", &[])).await;
    assert_eq!(other_committee.status, StatusCode::FORBIDDEN);
    assert!(other_committee.message().contains("don't direct Tech"));

    let director_role = add(&app, &director, json!({
        "andrewId": "x2", "name": null, "role": "director", "committee": "events", "canPostTicker": true, "projectIds": []
    }))
    .await;
    assert_eq!(director_role.status, StatusCode::FORBIDDEN);
    let admin_role = add(&app, &director, json!({
        "andrewId": "x3", "name": null, "role": "admin", "committee": null, "canPostTicker": true, "projectIds": []
    }))
    .await;
    assert_eq!(admin_role.status, StatusCode::FORBIDDEN);

    // Leads and members cannot manage anyone.
    let lead = testing::user(&db, "lanelead", &[(Role::Lead, Some(Committee::Events))]).await;
    assert_eq!(
        add(&app, &lead, lead_input("x4", "events", &[]))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let member = testing::user(&db, "lanemember", &[]).await;
    assert_eq!(
        add(&app, &member, lead_input("x5", "events", &[]))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        Req::post("/api/people")
            .json(lead_input("x6", "events", &[]))
            .send(&app)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn second_committees_need_an_admin(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let events = testing::user(
        &db,
        "secevents",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    let admin = testing::user(&db, "secadmin", &[(Role::Admin, None)]).await;
    testing::user(&db, "techlead", &[(Role::Lead, Some(Committee::Tech))]).await;

    let res = add(&app, &events, lead_input("techlead", "events", &[])).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(
        res.message(),
        "Test techlead already has a role in Tech. Only an admin can give someone a second committee."
    );
    let res = add(&app, &admin, lead_input("techlead", "events", &[])).await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.json["roles"].as_array().unwrap().len(), 2);

    // One role per committee, and no duplicates.
    let res = add(&app, &admin, lead_input("techlead", "events", &[])).await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    let res = add(&app, &admin, json!({
        "andrewId": "techlead", "name": null, "role": "director", "committee": "tech", "canPostTicker": true, "projectIds": []
    }))
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert!(res.message().contains("Change that role instead"));
}

#[sqlx::test(migrations = "./migrations")]
async fn input_is_validated(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = testing::user(&db, "valadmin", &[(Role::Admin, None)]).await;
    for (body, field) in [
        (lead_input("bad id!", "events", &[]), "andrewId"),
        (lead_input("ok@andrew.cmu.edu", "events", &[]), "andrewId"),
        (
            json!({ "andrewId": "ok", "name": "a\nb", "role": "lead", "committee": "events", "canPostTicker": true, "projectIds": [] }),
            "name",
        ),
        (
            json!({ "andrewId": "ok", "name": null, "role": "admin", "committee": "events", "canPostTicker": true, "projectIds": [] }),
            "committee",
        ),
        (
            json!({ "andrewId": "ok", "name": null, "role": "director", "committee": null, "canPostTicker": true, "projectIds": [] }),
            "committee",
        ),
    ] {
        let res = add(&app, &admin, body).await;
        assert_eq!(
            res.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field}: {}",
            res.text()
        );
        assert!(
            res.json["error"]["fields"][field].is_string(),
            "{field}: {}",
            res.text()
        );
    }
    let res = add(
        &app,
        &admin,
        json!({ "andrewId": "ok", "role": "chief", "committee": null }),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.code(), "bad_request");
}

#[sqlx::test(migrations = "./migrations")]
async fn nobody_removes_their_own_admin_role(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let first = testing::user(&db, "firstadmin", &[(Role::Admin, None)]).await;
    let second = testing::user(&db, "secondadmin", &[(Role::Admin, None)]).await;
    let own = assignment_id(&db, "firstadmin", None).await;

    let res = Req::delete(&format!("/api/people/assignments/{own}"))
        .user(&first)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert!(
        res.message()
            .starts_with("You can't remove your own admin role.")
    );
    let res = Req::patch(&format!("/api/people/assignments/{own}"))
        .user(&first)
        .json(json!({ "role": "director", "committee": "tech" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // Another admin can.
    let res = Req::delete(&format!("/api/people/assignments/{own}"))
        .user(&second)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Removed Test firstadmin as an admin"]
    );
    // And the remaining admin cannot remove themselves.
    let last = assignment_id(&db, "secondadmin", None).await;
    let res = Req::delete(&format!("/api/people/assignments/{last}"))
        .user(&second)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_last_admin_stays(db: PgPool) {
    testing::ensure_committees(&db).await;
    let only = testing::user(&db, "onlyadmin", &[(Role::Admin, None)]).await;
    // Someone else acting (for example a group sync) still cannot remove the last admin.
    let other = crate::permissions::tests::viewer(&[(Role::Admin, None, true)], vec![]);
    let mut conn = db.acquire().await.unwrap();
    let res = check_admin_removal(&mut conn, &other, only.id).await;
    assert!(matches!(res, Err(AppError::Conflict(_))));

    testing::user(&db, "spareadmin", &[(Role::Admin, None)]).await;
    assert!(
        check_admin_removal(&mut conn, &other, only.id)
            .await
            .is_ok()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn changing_a_leads_access(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let nova = testing::project(&db, Committee::Events, "nova").await;
    let tartanhacks = testing::project(&db, Committee::Events, "tartanhacks").await;
    let courses = testing::project(&db, Committee::Tech, "cmu-courses").await;
    let director = testing::user(
        &db,
        "chdirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    UserSpec::new("chlead")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;
    let id = assignment_id(&db, "chlead", Some(Committee::Events)).await;
    let path = format!("/api/people/assignments/{id}");

    let res = Req::patch(&path)
        .user(&director)
        .json(json!({ "canPostTicker": false, "projectIds": [tartanhacks] }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    let role = role_of(&res.json, "events");
    assert_eq!(role["canPostTicker"], false);
    assert_eq!(role["projectIds"], json!([tartanhacks]));
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Updated what Test chlead can do as a lead in Events"]
    );

    let res = Req::patch(&path)
        .user(&director)
        .json(json!({ "projectIds": [courses] }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Directors cannot promote, or move a lead to a committee they don't direct.
    let res = Req::patch(&path)
        .user(&director)
        .json(json!({ "role": "director" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::patch(&path)
        .user(&director)
        .json(json!({ "committee": "tech" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // Admins can; the lead's Events projects go with the old role.
    let admin = testing::user(&db, "chadmin", &[(Role::Admin, None)]).await;
    let res = Req::patch(&path)
        .user(&admin)
        .json(json!({ "role": "director" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    let role = role_of(&res.json, "events");
    assert_eq!(role["role"], "director");
    assert_eq!(role["canPostTicker"], true);
    assert_eq!(role["projectIds"], json!([]));
    let editors: i64 = sqlx::query_scalar("SELECT count(*) FROM project_editors")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(editors, 0);

    // Unknown ids are not found.
    let res = Req::patch(&format!("/api/people/assignments/{}", Uuid::now_v7()))
        .user(&admin)
        .json(json!({ "canPostTicker": true }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = Req::delete("/api/people/assignments/not-a-uuid")
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn removing_a_lead_removes_their_projects(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let nova = testing::project(&db, Committee::Events, "nova").await;
    let director = testing::user(
        &db,
        "rmdirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    let other = testing::user(&db, "rmtech", &[(Role::Director, Some(Committee::Tech))]).await;
    let lead = UserSpec::new("rmlead")
        .lead(Committee::Events, true)
        .edits(nova)
        .create(&db)
        .await;
    let id = assignment_id(&db, "rmlead", Some(Committee::Events)).await;

    let res = Req::delete(&format!("/api/people/assignments/{id}"))
        .user(&other)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::delete(&format!("/api/people/assignments/{id}"))
        .user(&director)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let editors: i64 = sqlx::query_scalar("SELECT count(*) FROM project_editors")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(editors, 0);
    // The change applies at the lead's next request.
    let res = Req::get("/api/me").user(&lead).send(&app).await;
    assert_eq!(res.json["permissions"]["canUsePortal"], false);
    assert_eq!(res.json["permissions"]["editableProjectIds"], json!([]));
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Removed Test rmlead as a lead in Events"]
    );
    let committee: Option<String> = sqlx::query_scalar("SELECT committee FROM audit_log")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(committee.as_deref(), Some("events"));
}

#[sqlx::test(migrations = "./migrations")]
async fn lists_are_scoped(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("lsadmin")
        .name("Ada Admin")
        .admin()
        .create(&db)
        .await;
    let events = UserSpec::new("lsevents")
        .name("Eve Events")
        .director(Committee::Events)
        .create(&db)
        .await;
    let lead = UserSpec::new("lslead")
        .name("Lee Lead")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    UserSpec::new("lstech")
        .name("Tim Tech")
        .director(Committee::Tech)
        .create(&db)
        .await;
    let member = testing::user(&db, "lsmember", &[]).await;

    let names = |res: &Res| -> Vec<String> {
        res.json["people"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["person"]["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let all = Req::get("/api/people").user(&admin).send(&app).await;
    assert_eq!(
        names(&all),
        ["Ada Admin", "Eve Events", "Lee Lead", "Tim Tech"]
    );
    assert_eq!(all.json["people"][0]["person"]["hasSignedIn"], true);
    assert!(all.json["people"][0]["person"]["lastSeenAt"].is_string());

    let tech = Req::get("/api/people?committee=tech")
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(names(&tech), ["Tim Tech"]);

    let scoped = Req::get("/api/people").user(&events).send(&app).await;
    assert_eq!(names(&scoped), ["Eve Events", "Lee Lead"]);
    let res = Req::get("/api/people?committee=tech")
        .user(&events)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::get("/api/people?committee=marketing")
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    assert_eq!(
        Req::get("/api/people").user(&lead).send(&app).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        Req::get("/api/people")
            .user(&member)
            .send(&app)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn search_matches_names_and_ids_literally(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let director = testing::user(
        &db,
        "srdirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    for (andrew_id, name) in [
        ("spchoksh", "Sara Chokshi"),
        ("sarah2", "Sarah Lee"),
        ("under_score", "Unity"),
        ("percent", "100% Pure"),
        ("asara", "Zed"),
    ] {
        UserSpec::new(andrew_id).name(name).create(&db).await;
    }
    let ids = |res: &Res| -> Vec<String> {
        res.json["people"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["andrewId"].as_str().unwrap().to_owned())
            .collect()
    };
    let res = Req::get("/api/people/search?q=sara")
        .user(&director)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(ids(&res), ["spchoksh", "sarah2", "asara"]);
    assert_eq!(res.json["people"][0]["hasSignedIn"], true);

    assert_eq!(
        ids(&Req::get("/api/people/search?q=_")
            .user(&director)
            .send(&app)
            .await),
        ["under_score"]
    );
    assert_eq!(
        ids(&Req::get("/api/people/search?q=%25")
            .user(&director)
            .send(&app)
            .await),
        ["percent"]
    );
    assert_eq!(
        ids(&Req::get("/api/people/search?q=CHOKSHI")
            .user(&director)
            .send(&app)
            .await),
        ["spchoksh"]
    );
    assert_eq!(
        ids(&Req::get("/api/people/search?q=")
            .user(&director)
            .send(&app)
            .await),
        Vec::<String>::new()
    );

    for n in 0..15 {
        UserSpec::new(&format!("bulk{n:02}"))
            .name("Bulk Person")
            .create(&db)
            .await;
    }
    assert_eq!(
        ids(&Req::get("/api/people/search?q=bulk")
            .user(&director)
            .send(&app)
            .await)
        .len(),
        10
    );

    let lead = testing::user(&db, "srlead", &[(Role::Lead, Some(Committee::Events))]).await;
    assert_eq!(
        Req::get("/api/people/search?q=sara")
            .user(&lead)
            .send(&app)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[test]
fn like_patterns_are_escaped() {
    assert_eq!(escape_like("a_b%c\\d"), "a\\_b\\%c\\\\d");
    assert_eq!(escape_like("plain"), "plain");
}
