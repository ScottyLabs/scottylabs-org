use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::*;
use crate::testing::{self, Req, TestUser, UserSpec};

const GOVERNANCE_SAMPLE: &str = r#"
# yaml-language-server: $schema=../schemas/committees.schema.json
director: tkanz
committees:
  events:
    name: Events
    director: evanfost
    roles:
      head-of-logistics:
        name: Head of Logistics
        members: [spchoksh, HelenF]
  tech:
    name: Tech
    director: apallati
    roles:
      head-of-infrastructure:
        name: Head of Infrastructure
        members: [techhead]
  design:
    name: Design
    director: jeanchen
    teams:
      branding:
        name: Branding
        roles:
          head-of-branding:
            name: Head of Branding
            members: [yelimk2, "not an id"]
      ui-ux:
        name: UI/UX
        roles:
          head-of-ui-ux:
            members: [yelimk2, bvu]
  marketing:
    name: Marketing
    director: someone
  finance:
    name: Finance
    director: "Not An ID"
"#;

fn head(committee: Committee, andrew_id: &str, role: &str) -> Head {
    Head {
        committee,
        andrew_id: andrew_id.into(),
        role: role.into(),
    }
}

/// Serves `yaml` like git.cmu.dev serves the raw file, and returns its URL.
async fn serve(yaml: &str) -> String {
    serve_with(StatusCode::OK, yaml.to_owned()).await
}

async fn serve_with(status: StatusCode, body: String) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/data/committees.yaml",
        listener.local_addr().unwrap()
    );
    let server = Router::new().route(
        "/data/committees.yaml",
        get(move || {
            let body = body.clone();
            async move { (status, body) }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });
    url
}

/// The shared client's settings: no redirects.
fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

async fn sync_yaml(db: &PgPool, yaml: &str, actor: Option<&Viewer>) -> GovernanceSyncResponse {
    sync_from(db, &http(), &serve(yaml).await, actor)
        .await
        .unwrap()
}

/// (andrew_id, role, committee, source, can_post_ticker, appointed_by is null), sorted.
async fn roles(db: &PgPool) -> Vec<(String, String, Option<String>, String, bool, bool)> {
    sqlx::query_as(
        "SELECT u.andrew_id, r.role, r.committee, r.source, r.can_post_ticker, r.appointed_by IS NULL
         FROM role_assignments r JOIN users u ON u.id = r.user_id
         ORDER BY u.andrew_id, r.committee",
    )
    .fetch_all(db)
    .await
    .unwrap()
}

async fn editors_of(db: &PgPool, andrew_id: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM project_editors pe JOIN users u ON u.id = pe.user_id WHERE u.andrew_id = $1",
    )
    .bind(andrew_id)
    .fetch_one(db)
    .await
    .unwrap()
}

/// A lead whose role an earlier sync added.
async fn governance_lead(db: &PgPool, andrew_id: &str, committee: Committee) -> TestUser {
    let person = testing::user(db, andrew_id, &[]).await;
    sqlx::query(
        "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, source)
         VALUES ($1, $2, 'lead', $3, true, 'governance')",
    )
    .bind(Uuid::now_v7())
    .bind(person.id)
    .bind(committee)
    .execute(db)
    .await
    .unwrap();
    person
}

async fn assignment_id(db: &PgPool, andrew_id: &str, committee: Committee) -> Uuid {
    sqlx::query_scalar(
        "SELECT r.id FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE u.andrew_id = $1 AND r.committee = $2",
    )
    .bind(andrew_id)
    .bind(committee)
    .fetch_one(db)
    .await
    .unwrap()
}

/// (andrew_id, committee) of everyone the People page opted out, sorted.
async fn opt_outs(db: &PgPool) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT u.andrew_id, o.committee FROM governance_opt_outs o JOIN users u ON u.id = o.user_id
         ORDER BY u.andrew_id, o.committee",
    )
    .fetch_all(db)
    .await
    .unwrap()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

async fn send_ok(app: &Router, req: Req) {
    let res = req.send(app).await;
    assert!(res.status.is_success(), "{} {}", res.status, res.text());
}

fn admin_viewer(user_id: Uuid) -> Viewer {
    Viewer {
        user_id,
        ..crate::permissions::tests::viewer(&[(Role::Admin, None, true)], vec![])
    }
}

#[test]
fn parses_directors_and_heads_from_the_governance_file() {
    let listed = parse_committees(GOVERNANCE_SAMPLE).unwrap();
    assert_eq!(
        listed.directors,
        [
            (Committee::Events, "evanfost".to_owned()),
            (Committee::Tech, "apallati".to_owned()),
            (Committee::Design, "jeanchen".to_owned()),
        ]
    );
    // Committee roles and team roles, Andrew IDs lowercased, each person once per committee, Tech left out.
    assert_eq!(
        listed.heads,
        [
            head(Committee::Events, "spchoksh", "Head of Logistics"),
            head(Committee::Events, "helenf", "Head of Logistics"),
            head(Committee::Design, "yelimk2", "Head of Branding"),
            head(Committee::Design, "bvu", "head-of-ui-ux"),
        ]
    );
    assert_eq!(
        listed.committees,
        [
            Committee::Events,
            Committee::Tech,
            Committee::Design,
            Committee::Finance
        ]
    );
    assert_eq!(
        listed.head_committees,
        [Committee::Events, Committee::Design, Committee::Finance]
    );
    assert_eq!(
        listed.people_in(Committee::Events),
        ["evanfost", "spchoksh", "helenf"]
    );
    assert_eq!(listed.people_in(Committee::Tech), ["apallati"]);
    assert!(listed.people_in(Committee::Finance).is_empty());
    assert_eq!(
        listed.notes,
        [
            "Skipped \u{201c}not an id\u{201d} in Design: it is not a valid Andrew ID.",
            "Skipped Finance: its director is not a valid Andrew ID.",
            "Skipped \u{201c}marketing\u{201d}: the site has no committee by that name.",
            "Skipped the heads listed under Tech. Tech leads come from the governance team files, so add them on this page.",
        ]
    );
    assert!(parse_committees("committees: [1, 2]").is_err());
    assert!(parse_committees(": : :").is_err());
    assert!(parse_committees("committees:\n  events:\n    roles: [1]\n").is_err());
}

#[test]
fn committees_without_roles_have_no_heads() {
    let listed =
        parse_committees("committees:\n  foundry:\n    name: Foundry\n    director: pkumara\n")
            .unwrap();
    assert!(listed.heads.is_empty());
    assert_eq!(listed.committees, [Committee::Foundry]);
    assert_eq!(listed.head_committees, [Committee::Foundry]);
    assert!(listed.notes.is_empty());
}

#[test]
fn the_response_keeps_its_old_fields_and_adds_heads() {
    let report = GovernanceSyncResponse {
        added: 1,
        heads_added: 2,
        ..GovernanceSyncResponse::default()
    };
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        json!({
            "added": 1, "updated": 0, "unchanged": 0,
            "headsAdded": 2, "headsRemoved": 0, "headsUnchanged": 0,
            "messages": [],
        })
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn governance_sync_adds_updates_and_replaces_directors(db: PgPool) {
    testing::ensure_committees(&db).await;
    let admin = testing::user(&db, "govadmin", &[(Role::Admin, None)]).await;
    // Already director of Events by hand: unchanged.
    testing::user(
        &db,
        "evanfost",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    // A lead in Design who governance now lists as director: upgraded.
    let projects = testing::project(&db, Committee::Design, "brand-kit").await;
    UserSpec::new("jeanchen")
        .lead(Committee::Design, true)
        .edits(projects)
        .create(&db)
        .await;
    // A director of Tech from an earlier sync whom governance no longer lists: removed.
    let old = testing::user(&db, "oldtech", &[]).await;
    sqlx::query("INSERT INTO role_assignments (id, user_id, role, committee, source) VALUES ($1, $2, 'director', 'tech', 'governance')")
        .bind(Uuid::now_v7())
        .bind(old.id)
        .execute(&db)
        .await
        .unwrap();

    let viewer = admin_viewer(admin.id);
    let directors = parse_committees(GOVERNANCE_SAMPLE).unwrap().directors;
    let mut report = GovernanceSyncResponse::default();
    let mut tx = db.begin().await.unwrap();
    apply_directors(&mut tx, Some(&viewer), &directors, &mut report)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!((report.added, report.updated, report.unchanged), (1, 2, 1));
    assert_eq!(
        report.messages,
        ["Test oldtech is no longer director of Tech. Governance lists apallati."]
    );
    let synced: Vec<_> = roles(&db)
        .await
        .into_iter()
        .map(|(andrew_id, role, committee, source, _, _)| (andrew_id, role, committee, source))
        .collect();
    assert_eq!(
        synced,
        [
            (
                "apallati".into(),
                "director".into(),
                Some("tech".into()),
                "governance".into()
            ),
            (
                "evanfost".into(),
                "director".into(),
                Some("events".into()),
                "portal".into()
            ),
            ("govadmin".into(), "admin".into(), None, "portal".into()),
            (
                "jeanchen".into(),
                "director".into(),
                Some("design".into()),
                "governance".into()
            ),
        ]
    );
    // The organization director in governance is never made an admin.
    let tkanz: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE andrew_id = 'tkanz'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(tkanz, 0);
    assert_eq!(editors_of(&db, "jeanchen").await, 0);
    let audited: Vec<Option<Uuid>> =
        sqlx::query_scalar("SELECT actor_id FROM audit_log WHERE action = 'role.sync'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(audited, [Some(admin.id); 3]);

    // Running it again changes nothing.
    let mut again = GovernanceSyncResponse::default();
    let mut tx = db.begin().await.unwrap();
    apply_directors(&mut tx, Some(&viewer), &directors, &mut again)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!((again.added, again.updated, again.unchanged), (0, 0, 3));
}

#[sqlx::test(migrations = "./migrations")]
async fn governance_heads_become_leads_without_touching_existing_roles(db: PgPool) {
    testing::ensure_committees(&db).await;
    testing::user(
        &db,
        "evanfost",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    // Already a lead in Events from the portal, with a project: left alone.
    let nova = testing::project(&db, Committee::Events, "nova").await;
    UserSpec::new("spchoksh")
        .lead(Committee::Events, false)
        .edits(nova)
        .create(&db)
        .await;
    // Added by an earlier sync and still listed: unchanged.
    governance_lead(&db, "yelimk2", Committee::Design).await;
    // Added by an earlier sync, no longer listed: removed with their projects.
    let tartanhacks = testing::project(&db, Committee::Events, "tartanhacks").await;
    let old = governance_lead(&db, "oldhead", Committee::Events).await;
    sqlx::query("INSERT INTO project_editors (project_id, user_id) VALUES ($1, $2)")
        .bind(tartanhacks)
        .bind(old.id)
        .execute(&db)
        .await
        .unwrap();
    governance_lead(&db, "finhead", Committee::Finance).await;
    // Committees whose heads are not read keep their governance leads: Foundry is missing from the file, Tech is skipped.
    governance_lead(&db, "foundryhead", Committee::Foundry).await;
    governance_lead(&db, "techlead", Committee::Tech).await;

    let report = sync_yaml(&db, GOVERNANCE_SAMPLE, None).await;
    assert_eq!((report.added, report.updated, report.unchanged), (2, 0, 1));
    assert_eq!(
        (
            report.heads_added,
            report.heads_removed,
            report.heads_unchanged
        ),
        (2, 2, 2)
    );
    assert_eq!(report.messages.len(), 6, "{:?}", report.messages);
    assert!(report.messages.contains(
        &"Test oldhead is no longer a lead in Events. Governance no longer lists them as a head."
            .to_owned()
    ));
    assert!(report.messages.contains(
        &"Test finhead is no longer a lead in Finance. Governance no longer lists them as a head."
            .to_owned()
    ));

    let lead = |andrew_id: &str, committee: &str, source: &str, ticker: bool, by_nobody: bool| {
        (
            andrew_id.to_owned(),
            "lead".to_owned(),
            Some(committee.to_owned()),
            source.to_owned(),
            ticker,
            by_nobody,
        )
    };
    let director = |andrew_id: &str, committee: &str, source: &str| {
        (
            andrew_id.to_owned(),
            "director".to_owned(),
            Some(committee.to_owned()),
            source.to_owned(),
            true,
            true,
        )
    };
    assert_eq!(
        roles(&db).await,
        [
            director("apallati", "tech", "governance"),
            lead("bvu", "design", "governance", true, true),
            director("evanfost", "events", "portal"),
            lead("foundryhead", "foundry", "governance", true, true),
            lead("helenf", "events", "governance", true, true),
            director("jeanchen", "design", "governance"),
            lead("spchoksh", "events", "portal", false, true),
            lead("techlead", "tech", "governance", true, true),
            lead("yelimk2", "design", "governance", true, true),
        ]
    );
    assert_eq!(editors_of(&db, "spchoksh").await, 1);
    assert_eq!(editors_of(&db, "oldhead").await, 0);
    assert_eq!(editors_of(&db, "helenf").await, 0);
    // Heads nobody had added yet are known by Andrew ID until they sign in.
    let helen: (String, Option<String>) =
        sqlx::query_as("SELECT name, keycloak_sub FROM users WHERE andrew_id = 'helenf'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(helen, (String::new(), None));

    // The service's own runs are logged without an actor.
    let summaries: Vec<(Option<Uuid>, String)> = sqlx::query_as(
        "SELECT actor_id, summary FROM audit_log WHERE action = 'role.sync' ORDER BY id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert!(summaries.iter().all(|(actor, _)| actor.is_none()));
    let summaries: Vec<String> = summaries.into_iter().map(|(_, s)| s).collect();
    assert!(
        summaries.contains(
            &"Made helenf a lead in Events from governance (Head of Logistics)".to_owned()
        )
    );
    assert!(
        summaries.contains(&"Made bvu a lead in Design from governance (head-of-ui-ux)".to_owned())
    );
    assert!(
        summaries.contains(
            &"Removed Test oldhead as a lead in Events; governance no longer lists them as a head"
                .to_owned()
        )
    );

    // Running it again changes nothing.
    let again = sync_yaml(&db, GOVERNANCE_SAMPLE, None).await;
    assert_eq!(
        (
            again.added,
            again.updated,
            again.unchanged,
            again.heads_added,
            again.heads_removed,
            again.heads_unchanged
        ),
        (0, 0, 3, 0, 0, 4)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_head_who_directs_the_committee_stays_director(db: PgPool) {
    testing::ensure_committees(&db).await;
    let yaml = r#"
committees:
  labrador:
    director: hbenko
    roles:
      head-of-product:
        name: Head of Product
        members: [hbenko, laki]
"#;
    let report = sync_yaml(&db, yaml, None).await;
    assert_eq!(
        (report.added, report.heads_added, report.heads_unchanged),
        (1, 1, 1)
    );
    let labrador: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.andrew_id, r.role FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE r.committee = 'labrador' ORDER BY u.andrew_id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        labrador,
        [
            ("hbenko".to_owned(), "director".to_owned()),
            ("laki".to_owned(), "lead".to_owned()),
        ]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn leads_edited_on_the_people_page_survive_later_syncs(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let director = UserSpec::new("evanfost")
        .director(Committee::Events)
        .create(&db)
        .await;
    // Added by hand and never listed by governance: nothing to say about them.
    UserSpec::new("handpicked")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    let yaml = r#"
committees:
  events:
    director: evanfost
    roles:
      head-of-logistics:
        name: Head of Logistics
        members: [helenf, dropme]
"#;
    let report = sync_yaml(&db, yaml, None).await;
    assert_eq!(report.heads_added, 2);

    let helen: Uuid = sqlx::query_scalar(
        "SELECT r.id FROM role_assignments r JOIN users u ON u.id = r.user_id WHERE u.andrew_id = 'helenf'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let res = Req::patch(&format!("/api/people/assignments/{helen}"))
        .user(&director)
        .json(json!({ "canPostTicker": false }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    let edited: &Value = &res.json["roles"][0];
    assert_eq!(edited["source"], "portal");
    assert_eq!(edited["canPostTicker"], false);

    // Governance drops both: the edited lead stays, the other goes.
    let yaml = r#"
committees:
  events:
    director: evanfost
"#;
    let report = sync_yaml(&db, yaml, None).await;
    assert_eq!((report.heads_removed, report.heads_unchanged), (1, 0));
    assert_eq!(
        report.messages,
        [
            "dropme is no longer a lead in Events. Governance no longer lists them as a head.",
            "helenf is still a lead in Events, but governance no longer lists them as a head. Remove them here if they should no longer be a lead.",
        ]
    );
    let leads: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.andrew_id, r.source FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE r.role = 'lead' ORDER BY u.andrew_id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        leads,
        pairs(&[("handpicked", "portal"), ("helenf", "portal")])
    );

    // Said once, and kept in the activity log for the committee.
    let report = sync_yaml(&db, yaml, None).await;
    assert!(report.messages.is_empty(), "{:?}", report.messages);
    let kept: Vec<(Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT actor_id, committee FROM audit_log
         WHERE action = 'role.sync' AND summary = 'Kept helenf as a lead in Events; governance no longer lists them as a head'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(kept, [(None, Some("events".to_owned()))]);
}

#[sqlx::test(migrations = "./migrations")]
async fn roles_removed_or_lowered_on_the_people_page_are_not_given_back(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("govadmin").admin().create(&db).await;
    let director = UserSpec::new("evanfost")
        .director(Committee::Events)
        .create(&db)
        .await;
    UserSpec::new("handpicked")
        .lead(Committee::Events, true)
        .create(&db)
        .await;
    let first = r#"
committees:
  events:
    director: evanfost
    roles:
      head-of-logistics:
        name: Head of Logistics
        members: [helenf, sam]
  design:
    director: jeanchen
    roles:
      head-of-branding:
        name: Head of Branding
        members: [yelimk2]
  finance:
    director: findir
"#;
    let report = sync_yaml(&db, first, None).await;
    assert_eq!((report.added, report.heads_added), (2, 3));

    // A director removes a head and a lead they added by hand (PE-2). An admin removes a director, turns another into a lead, and moves a head to another committee.
    for (andrew_id, committee) in [
        ("helenf", Committee::Events),
        ("handpicked", Committee::Events),
    ] {
        let id = assignment_id(&db, andrew_id, committee).await;
        send_ok(
            &app,
            Req::delete(&format!("/api/people/assignments/{id}")).user(&director),
        )
        .await;
    }
    let findir = assignment_id(&db, "findir", Committee::Finance).await;
    send_ok(
        &app,
        Req::delete(&format!("/api/people/assignments/{findir}")).user(&admin),
    )
    .await;
    let jeanchen = assignment_id(&db, "jeanchen", Committee::Design).await;
    send_ok(
        &app,
        Req::patch(&format!("/api/people/assignments/{jeanchen}"))
            .user(&admin)
            .json(json!({ "role": "lead" })),
    )
    .await;
    let sam = assignment_id(&db, "sam", Committee::Events).await;
    send_ok(
        &app,
        Req::patch(&format!("/api/people/assignments/{sam}"))
            .user(&admin)
            .json(json!({ "committee": "design" })),
    )
    .await;
    assert_eq!(
        opt_outs(&db).await,
        pairs(&[
            ("findir", "finance"),
            ("handpicked", "events"),
            ("helenf", "events"),
            ("jeanchen", "design"),
            ("sam", "events"),
        ])
    );

    // The next sync leaves all of that alone and says why.
    let report = sync_yaml(&db, first, None).await;
    assert_eq!(
        (
            report.added,
            report.updated,
            report.unchanged,
            report.heads_added,
            report.heads_removed,
            report.heads_unchanged
        ),
        (0, 0, 1, 0, 0, 1)
    );
    assert_eq!(
        report.messages,
        [
            "Governance lists jeanchen as director of Design, but their role there was removed or changed on this page, so it was not given back. Make them director here if they should be.",
            "Governance lists findir as director of Finance, but their role there was removed or changed on this page, so it was not given back. Make them director here if they should be.",
            "Governance lists helenf as a head in Events, but their role there was removed or changed on this page, so it was not given back. Add them as a lead here if they should be one.",
            "Governance lists sam as a head in Events, but their role there was removed or changed on this page, so it was not given back. Add them as a lead here if they should be one.",
        ]
    );
    let synced: Vec<_> = roles(&db)
        .await
        .into_iter()
        .map(|(andrew_id, role, committee, source, _, _)| (andrew_id, role, committee, source))
        .collect();
    let role = |andrew_id: &str, role: &str, committee: Option<&str>, source: &str| {
        (
            andrew_id.to_owned(),
            role.to_owned(),
            committee.map(str::to_owned),
            source.to_owned(),
        )
    };
    assert_eq!(
        synced,
        [
            role("evanfost", "director", Some("events"), "portal"),
            role("govadmin", "admin", None, "portal"),
            role("jeanchen", "lead", Some("design"), "portal"),
            role("sam", "lead", Some("design"), "portal"),
            role("yelimk2", "lead", Some("design"), "governance"),
        ]
    );
    // The opt-out of someone governance does not list is forgotten.
    assert_eq!(
        opt_outs(&db).await,
        pairs(&[
            ("findir", "finance"),
            ("helenf", "events"),
            ("jeanchen", "design"),
            ("sam", "events"),
        ])
    );

    // Once governance stops listing someone there, their opt-out goes, so a later listing counts again.
    let second = r#"
committees:
  events:
    director: evanfost
    roles:
      head-of-logistics:
        members: [sam]
  design:
    director: jeanchen
    roles:
      head-of-branding:
        members: [yelimk2]
  finance:
    director: otherfin
"#;
    let report = sync_yaml(&db, second, None).await;
    assert_eq!((report.added, report.heads_added), (1, 0));
    assert_eq!(
        opt_outs(&db).await,
        pairs(&[("jeanchen", "design"), ("sam", "events")])
    );
    let third = second.replace("members: [sam]", "members: [sam, helenf]");
    let report = sync_yaml(&db, &third, None).await;
    assert_eq!(report.heads_added, 1);
    let helen: (String, String) = sqlx::query_as(
        "SELECT r.role, r.source FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE u.andrew_id = 'helenf' AND r.committee = 'events'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(helen, ("lead".to_owned(), "governance".to_owned()));
}

#[sqlx::test(migrations = "./migrations")]
async fn promoting_a_lead_to_director_is_not_an_opt_out(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = UserSpec::new("govadmin").admin().create(&db).await;
    let yaml = r#"
committees:
  events:
    roles:
      head-of-logistics:
        members: [helenf]
"#;
    sync_yaml(&db, yaml, None).await;
    let helen = assignment_id(&db, "helenf", Committee::Events).await;
    send_ok(
        &app,
        Req::patch(&format!("/api/people/assignments/{helen}"))
            .user(&admin)
            .json(json!({ "role": "director" })),
    )
    .await;
    assert!(opt_outs(&db).await.is_empty());
    let report = sync_yaml(&db, yaml, None).await;
    assert_eq!(report.heads_unchanged, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn admins_who_sync_are_named_in_the_log(db: PgPool) {
    testing::ensure_committees(&db).await;
    let admin = testing::user(&db, "govadmin", &[(Role::Admin, None)]).await;
    let viewer = admin_viewer(admin.id);
    let report = sync_yaml(&db, GOVERNANCE_SAMPLE, Some(&viewer)).await;
    assert_eq!(report.heads_added, 4);
    let actors: Vec<Option<Uuid>> =
        sqlx::query_scalar("SELECT DISTINCT actor_id FROM audit_log WHERE action = 'role.sync'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(actors, [Some(admin.id)]);
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_fetches_change_nothing(db: PgPool) {
    testing::ensure_committees(&db).await;
    let http = http();

    let missing = serve_with(StatusCode::NOT_FOUND, "Not found".into()).await;
    let error = sync_from(&db, &http, &missing, None).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "We couldn't reach the governance repository. Try again in a few minutes."
    );
    let moved = serve_with(StatusCode::FOUND, String::new()).await;
    assert!(sync_from(&db, &http, &moved, None).await.is_err());
    let huge = serve_with(StatusCode::OK, "#".repeat(MAX_GOVERNANCE_BYTES + 1)).await;
    assert_eq!(
        sync_from(&db, &http, &huge, None)
            .await
            .unwrap_err()
            .to_string(),
        "The governance committees file is larger than expected, so it was not read."
    );
    let broken = serve(": : :").await;
    assert!(
        sync_from(&db, &http, &broken, None)
            .await
            .unwrap_err()
            .to_string()
            .starts_with("The governance committees file could not be read.")
    );

    // The background run only logs these.
    sync_in_background(&db, &http, &missing).await;
    sync_in_background(&db, &http, "http://127.0.0.1:9/committees.yaml").await;
    assert!(roles(&db).await.is_empty());
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(users, 0);

    let good = serve(GOVERNANCE_SAMPLE).await;
    sync_in_background(&db, &http, &good).await;
    assert_eq!(roles(&db).await.len(), 7);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_admins_sync_governance(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let director = testing::user(
        &db,
        "govdirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    let res = Req::post("/api/people/sync-governance")
        .user(&director)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(
        res.message(),
        "Only admins can sync directors and heads from governance."
    );
}
