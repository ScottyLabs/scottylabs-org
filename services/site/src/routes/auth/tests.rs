use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Form;
use axum::extract::State as MockState;
use axum::http::StatusCode;
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use openidconnect::core::{
    CoreGenderClaim, CoreHmacKey, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
};
use openidconnect::{
    Audience, EndUserEmail, EndUserName, EndUserUsername, IdToken, IdTokenClaims, IssuerUrl,
    LocalizedClaim, StandardClaims, SubjectIdentifier,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::*;
use crate::auth::oidc::GroupClaims;
use crate::config::OidcSettings;
use crate::models::Role;
use crate::testing::{self, Req, Res};

// ---------------------------------------------------------------------------
// Pieces that need no network

#[test]
fn return_to_stays_in_the_portal() {
    assert_eq!(safe_return_to(None), "/portal/");
    assert_eq!(safe_return_to(Some("/portal")), "/portal");
    assert_eq!(safe_return_to(Some("/portal/ticker/")), "/portal/ticker/");
    assert_eq!(
        safe_return_to(Some("/portal/projects/?q=nova#top")),
        "/portal/projects/?q=nova#top"
    );
    for bad in [
        "",
        "/",
        "https://evil.example/portal/",
        "//evil.example/portal/",
        "/portalevil",
        "/portal//evil.example",
        "/portal\\evil.example",
        "/portal/../api/me",
        "/portal/\r\nSet-Cookie: x=1",
        "/portal/caf\u{e9}",
        "javascript:alert(1)",
        "/projects/nova/",
    ] {
        assert_eq!(safe_return_to(Some(bad)), "/portal/", "{bad:?}");
    }
    assert_eq!(
        safe_return_to(Some(&format!("/portal/{}", "a".repeat(600)))),
        "/portal/"
    );
}

#[test]
fn callback_url_is_on_the_app_origin() {
    let mut config = testing::config();
    assert_eq!(
        callback_url(&config),
        "http://localhost:5173/api/auth/callback"
    );
    config.app_url = url::Url::parse("https://scottylabs-org-site-pr-3.scottylabs.net/").unwrap();
    assert_eq!(
        callback_url(&config),
        "https://scottylabs-org-site-pr-3.scottylabs.net/api/auth/callback"
    );
}

#[test]
fn groups_match_with_or_without_a_leading_slash() {
    assert!(same_group(
        "/projects/scottylabs-org/admins",
        "projects/scottylabs-org/admins"
    ));
    assert!(!same_group(
        "/projects/scottylabs-org",
        "/projects/scottylabs-org/admins"
    ));
}

// ---------------------------------------------------------------------------
// Auth config and dev login

#[sqlx::test(migrations = "./migrations")]
async fn config_lists_personas_when_dev_login_is_on(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let res = Req::get("/api/auth/config").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.json["oidc"], false);
    assert_eq!(res.json["devLogin"], true);
    assert_eq!(res.json["devLoginNeedsKey"], false);
    assert_eq!(res.json["devPersonas"].as_array().unwrap().len(), 5);
    assert_eq!(
        res.json["devPersonas"][1],
        json!({ "andrewId": "evanfost", "name": "Evan Foster", "label": "Director of Events" })
    );

    let mut off = testing::config();
    off.dev_login = DevLogin::Off;
    let app = testing::app_with(db, off).await;
    let res = Req::get("/api/auth/config").send(&app).await;
    assert_eq!(res.json["devLogin"], false);
    assert_eq!(res.json["devPersonas"], json!([]));
}

async fn dev_login_as(app: &Router, andrew_id: &str, key: Option<&str>) -> Res {
    Req::post("/api/auth/dev-login")
        .json(json!({ "andrewId": andrew_id, "key": key }))
        .send(app)
        .await
}

fn session_cookie_from(res: &Res) -> String {
    let set_cookie = res
        .headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("sl_session=") && !value.starts_with("sl_session=;"))
        .expect("a session cookie");
    set_cookie.split(';').next().unwrap().to_owned()
}

#[sqlx::test(migrations = "./migrations")]
async fn dev_login_signs_in_as_each_persona(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app(db.clone()).await;
    for (andrew_id, can_use_portal) in [
        ("tkanz", true),
        ("evanfost", true),
        ("anishp", true),
        ("spchoksh", true),
        ("newmember", false),
    ] {
        let res = dev_login_as(&app, andrew_id, None).await;
        assert_eq!(
            res.status,
            StatusCode::NO_CONTENT,
            "{andrew_id}: {}",
            res.text()
        );
        let cookie = session_cookie_from(&res);
        let raw = res.header("set-cookie").unwrap();
        assert!(raw.contains("HttpOnly") && raw.contains("SameSite=Lax") && raw.contains("Path=/"));

        let me = Req::get("/api/me")
            .header("cookie", &cookie)
            .send(&app)
            .await;
        assert_eq!(me.status, StatusCode::OK);
        assert_eq!(me.json["user"]["andrewId"], andrew_id);
        assert_eq!(me.json["authMode"], "dev");
        assert_eq!(me.json["permissions"]["canUsePortal"], can_use_portal);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn dev_login_refuses_people_who_are_not_personas(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app(db.clone()).await;
    testing::user(&db, "realperson", &[(Role::Admin, None)]).await;
    for andrew_id in ["realperson", "nobody", "", "tkanz@andrew.cmu.edu"] {
        let res = dev_login_as(&app, andrew_id, None).await;
        assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{andrew_id}");
        assert_eq!(
            res.json["error"]["fields"]["andrewId"],
            "Choose one of the people listed."
        );
    }
    // Upper case is the same Andrew ID.
    assert_eq!(
        dev_login_as(&app, "TKanz", None).await.status,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn dev_login_is_refused_when_off_or_unseeded(db: PgPool) {
    testing::seed_personas(&db).await;
    let mut off = testing::config();
    off.dev_login = DevLogin::Off;
    let app = testing::app_with(db.clone(), off).await;
    let res = dev_login_as(&app, "tkanz", None).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    let mut unseeded = testing::config();
    unseeded.seed_demo = false;
    let app = testing::app_with(db, unseeded).await;
    assert_eq!(
        dev_login_as(&app, "tkanz", None).await.status,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn keyed_dev_login_needs_the_exact_key(db: PgPool) {
    testing::seed_personas(&db).await;
    let mut preview = testing::config();
    preview.app_url = url::Url::parse("https://scottylabs-org-site-pr-3.scottylabs.net").unwrap();
    preview.dev_login = DevLogin::Keyed("preview-key-0123456789".into());
    let app = testing::app_with(db, preview).await;

    let config = Req::get("/api/auth/config").send(&app).await;
    assert_eq!(config.json["devLoginNeedsKey"], true);

    for key in [
        None,
        Some(""),
        Some("preview-key-012345678"),
        Some("preview-key-0123456789x"),
    ] {
        let res = dev_login_as(&app, "tkanz", key).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{key:?}");
    }
    let res = dev_login_as(&app, "tkanz", Some("preview-key-0123456789")).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let raw = res.header("set-cookie").unwrap();
    assert!(raw.starts_with("__Host-sl_session=") && raw.contains("Secure"));
}

#[sqlx::test(migrations = "./migrations")]
async fn dev_login_replaces_an_existing_session(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app(db.clone()).await;
    let first = session_cookie_from(&dev_login_as(&app, "tkanz", None).await);
    let second = Req::post("/api/auth/dev-login")
        .header("cookie", &first)
        .json(json!({ "andrewId": "evanfost", "key": null }))
        .send(&app)
        .await;
    assert_eq!(second.status, StatusCode::NO_CONTENT);
    let old = Req::get("/api/me")
        .header("cookie", &first)
        .send(&app)
        .await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn dev_login_is_rate_limited(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app(db).await;
    for _ in 0..20 {
        assert_eq!(
            dev_login_as(&app, "tkanz", None).await.status,
            StatusCode::NO_CONTENT
        );
    }
    let limited = dev_login_as(&app, "tkanz", None).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.code(), "rate_limited");
    assert!(limited.header("retry-after").is_some());
}

// ---------------------------------------------------------------------------
// Sessions

#[sqlx::test(migrations = "./migrations")]
async fn sessions_expire_when_idle_or_old(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let idle = testing::user(&db, "idleperson", &[(Role::Admin, None)]).await;
    let old = testing::user(&db, "oldperson", &[(Role::Admin, None)]).await;

    sqlx::query("UPDATE sessions SET last_seen_at = now() - interval '11 hours 59 minutes' WHERE user_id = $1")
        .bind(idle.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        Req::get("/api/me").user(&idle).send(&app).await.status,
        StatusCode::OK
    );
    // That request refreshed last_seen_at.
    let fresh: bool = sqlx::query_scalar(
        "SELECT last_seen_at > now() - interval '1 minute' FROM sessions WHERE user_id = $1",
    )
    .bind(idle.id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(fresh);

    sqlx::query("UPDATE sessions SET last_seen_at = now() - interval '12 hours 1 minute' WHERE user_id = $1")
        .bind(idle.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        Req::get("/api/me").user(&idle).send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );

    // Active, but past the seven day limit.
    sqlx::query("UPDATE sessions SET expires_at = now() - interval '1 second' WHERE user_id = $1")
        .bind(old.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        Req::get("/api/me").user(&old).send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );

    // The next sign-in purges both.
    testing::user(&db, "nextperson", &[]).await;
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id = ANY($1)")
        .bind(vec![idle.id, old.id])
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_last_seven_days_and_store_only_a_hash(db: PgPool) {
    let person = testing::user(&db, "hashperson", &[]).await;
    let (hash, lifetime): (Vec<u8>, f64) = sqlx::query_as(
        "SELECT token_hash, extract(epoch FROM expires_at - created_at)::float8 FROM sessions WHERE user_id = $1",
    )
    .bind(person.id)
    .fetch_one(&db)
    .await
    .unwrap();
    let token = person.cookie.trim_start_matches("sl_session=");
    assert_eq!(hash, session::hash_token(token));
    assert_ne!(hash, token.as_bytes());
    assert!((lifetime - 7.0 * 24.0 * 3600.0).abs() < 5.0);
}

#[sqlx::test(migrations = "./migrations")]
async fn logout_ends_the_session_and_clears_the_cookie(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let person = testing::user(&db, "leaving", &[(Role::Admin, None)]).await;
    let res = Req::post("/api/auth/logout").user(&person).send(&app).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(res.header("set-cookie").unwrap().contains("Max-Age=0"));
    assert_eq!(
        Req::get("/api/me").user(&person).send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );
    // Signing out twice is fine.
    assert_eq!(
        Req::post("/api/auth/logout").send(&app).await.status,
        StatusCode::NO_CONTENT
    );
}

// ---------------------------------------------------------------------------
// Keycloak, against a mock that behaves like it

const CLIENT_ID: &str = "scottylabs-org-dev";
const CLIENT_SECRET: &str = "test-client-secret-for-hmac";
const RELAY: &str = "http://localhost:8090/oauth2/callback";
const ADMIN_GROUP: &str = "/projects/scottylabs-org/admins";

#[derive(Default)]
struct Mock {
    issuer: String,
    challenge: Option<String>,
    nonce: Option<String>,
    subject: String,
    username: String,
    groups: Option<Vec<String>>,
}

type Shared = Arc<Mutex<Mock>>;

async fn discovery(MockState(mock): MockState<Shared>) -> axum::Json<Value> {
    let issuer = mock.lock().unwrap().issuer.clone();
    axum::Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/protocol/openid-connect/auth"),
        "token_endpoint": format!("{issuer}/protocol/openid-connect/token"),
        "jwks_uri": format!("{issuer}/protocol/openid-connect/certs"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["HS256"],
    }))
}

async fn jwks() -> axum::Json<Value> {
    axum::Json(json!({ "keys": [] }))
}

async fn token(
    MockState(mock): MockState<Shared>,
    Form(form): Form<HashMap<String, String>>,
) -> axum::response::Response {
    let mock = mock.lock().unwrap();
    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let pkce_ok = mock.challenge.as_deref() == Some(challenge.as_str());
    if form.get("code").map(String::as_str) != Some("good-code")
        || !pkce_ok
        || form.get("redirect_uri").map(String::as_str) != Some(RELAY)
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({ "error": "invalid_grant" })),
        )
            .into_response();
    }
    let standard =
        StandardClaims::<CoreGenderClaim>::new(SubjectIdentifier::new(mock.subject.clone()))
            .set_preferred_username(Some(EndUserUsername::new(mock.username.clone())))
            .set_name(Some(LocalizedClaim::from(EndUserName::new(
                "Kay Cloak".into(),
            ))))
            .set_email(Some(EndUserEmail::new(format!(
                "{}@andrew.cmu.edu",
                mock.username
            ))));
    let claims = IdTokenClaims::new(
        IssuerUrl::new(mock.issuer.clone()).unwrap(),
        vec![Audience::new(CLIENT_ID.into())],
        Utc::now() + Duration::minutes(5),
        Utc::now(),
        standard,
        GroupClaims {
            groups: mock.groups.clone(),
        },
    )
    .set_nonce(mock.nonce.clone().map(openidconnect::Nonce::new));
    let id_token = IdToken::<
        GroupClaims,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >::new(
        claims,
        &CoreHmacKey::new(CLIENT_SECRET),
        CoreJwsSigningAlgorithm::HmacSha256,
        None,
        None,
    )
    .unwrap();
    axum::Json(json!({
        "access_token": "mock-access-token",
        "token_type": "bearer",
        "expires_in": 300,
        "id_token": id_token.to_string(),
    }))
    .into_response()
}

/// Starts the mock and returns an app configured to sign in through it.
async fn keycloak_app(db: &PgPool, username: &str, groups: Option<Vec<&str>>) -> (Router, Shared) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock: Shared = Arc::new(Mutex::new(Mock {
        issuer: format!("{base}/realms/scottylabs"),
        subject: format!("sub-{username}"),
        username: username.into(),
        groups: groups.map(|list| list.into_iter().map(str::to_owned).collect()),
        ..Mock::default()
    }));
    let server = Router::new()
        .route(
            "/realms/scottylabs/.well-known/openid-configuration",
            get(discovery),
        )
        .route(
            "/realms/scottylabs/protocol/openid-connect/certs",
            get(jwks),
        )
        .route(
            "/realms/scottylabs/protocol/openid-connect/token",
            post(token),
        )
        .with_state(mock.clone());
    tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });

    let mut config = testing::config();
    config.oidc = Some(OidcSettings {
        keycloak_url: base,
        keycloak_realm: "scottylabs".into(),
        client_id: CLIENT_ID.into(),
        client_secret: CLIENT_SECRET.into(),
        oauth_relay_url: RELAY.into(),
        project_group: "/projects/scottylabs-org".into(),
        project_admin_group: ADMIN_GROUP.into(),
    });
    (testing::app_with(db.clone(), config).await, mock)
}

struct Started {
    state: String,
    attempt_cookie: String,
}

/// Follows /api/auth/login like a browser and tells the mock what it would have seen.
async fn start_login(app: &Router, mock: &Shared, return_to: &str) -> Started {
    let res = Req::get(&format!(
        "/api/auth/login?returnTo={}",
        urlencode(return_to)
    ))
    .send(app)
    .await;
    assert_eq!(res.status, StatusCode::FOUND, "{}", res.text());
    let location = url::Url::parse(res.header("location").unwrap()).unwrap();
    let params: HashMap<String, String> = location.query_pairs().into_owned().collect();
    assert!(location.path().ends_with("/protocol/openid-connect/auth"));
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["client_id"], CLIENT_ID);
    assert_eq!(params["redirect_uri"], RELAY);
    assert_eq!(params["code_challenge_method"], "S256");
    let scopes: Vec<&str> = params["scope"].split(' ').collect();
    for scope in ["openid", "email", "profile"] {
        assert!(scopes.contains(&scope), "{scope}");
    }
    let relay = decode_state(&params["state"]).unwrap();
    assert_eq!(relay.return_to, "http://localhost:5173/api/auth/callback");

    let set_cookie = res.header("set-cookie").unwrap();
    assert!(set_cookie.starts_with(&format!("sl_signin={}", relay.csrf)));
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("Max-Age=600"));
    {
        let mut mock = mock.lock().unwrap();
        mock.challenge = Some(params["code_challenge"].clone());
        mock.nonce = Some(params["nonce"].clone());
    }
    Started {
        state: params["state"].clone(),
        attempt_cookie: format!("sl_signin={}", relay.csrf),
    }
}

fn urlencode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

async fn finish_login(app: &Router, started: &Started, code: &str) -> Res {
    Req::get(&format!(
        "/api/auth/callback?code={code}&state={}",
        started.state
    ))
    .header("cookie", &started.attempt_cookie)
    .send(app)
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_sign_in_end_to_end(db: PgPool) {
    let (app, mock) = keycloak_app(
        &db,
        "kcadmin",
        Some(vec!["/projects/scottylabs-org", ADMIN_GROUP]),
    )
    .await;
    let config = Req::get("/api/auth/config").send(&app).await;
    assert_eq!(config.json["oidc"], true);

    let started = start_login(&app, &mock, "/portal/ticker/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.status,
        StatusCode::FOUND,
        "{:?}",
        res.header("location")
    );
    assert_eq!(res.header("location"), Some("/portal/ticker/"));
    let cookies: Vec<&str> = res
        .headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(cookies.iter().any(|c| c.starts_with("sl_signin=;")
        || (c.starts_with("sl_signin=") && c.contains("Max-Age=0"))));
    let session = session_cookie_from(&res);

    let me = Req::get("/api/me")
        .header("cookie", &session)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json["user"]["andrewId"], "kcadmin");
    assert_eq!(me.json["user"]["name"], "Kay Cloak");
    assert_eq!(me.json["user"]["email"], "kcadmin@andrew.cmu.edu");
    assert_eq!(me.json["authMode"], "oidc");
    assert_eq!(me.json["roles"][0]["role"], "admin");
    assert_eq!(me.json["roles"][0]["source"], "keycloak_group");
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Kay Cloak became an admin through the Keycloak admin group"]
    );

    // The attempt was used up.
    let replay = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        replay.header("location"),
        Some("/portal/sign-in/?error=expired")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_links_people_added_before_they_signed_in(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kclead", Some(vec!["/projects/scottylabs-org"])).await;
    // Added by Andrew ID before their first sign-in.
    sqlx::query("INSERT INTO users (id, andrew_id) VALUES ($1, 'kclead')")
        .bind(uuid::Uuid::now_v7())
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO role_assignments (id, user_id, role, committee, source)
         SELECT $1, id, 'lead', 'events', 'portal' FROM users WHERE andrew_id = 'kclead'",
    )
    .bind(uuid::Uuid::now_v7())
    .execute(&db)
    .await
    .unwrap();

    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(res.header("location"), Some("/portal/"));
    let me = Req::get("/api/me")
        .header("cookie", &session_cookie_from(&res))
        .send(&app)
        .await;
    assert_eq!(me.json["roles"][0]["role"], "lead");
    assert_eq!(me.json["permissions"]["leadOf"], json!(["events"]));
    let linked: Option<String> =
        sqlx::query_scalar("SELECT keycloak_sub FROM users WHERE andrew_id = 'kclead'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(linked.as_deref(), Some("sub-kclead"));
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE andrew_id = 'kclead'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(users, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_admins_who_leave_the_group_lose_that_role(db: PgPool) {
    testing::user(&db, "otheradmin", &[(Role::Admin, None)]).await;
    let (app, mock) = keycloak_app(&db, "kcformer", Some(vec![ADMIN_GROUP])).await;
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    let me = Req::get("/api/me")
        .header("cookie", &session_cookie_from(&res))
        .send(&app)
        .await;
    assert_eq!(me.json["permissions"]["isAdmin"], true);

    mock.lock().unwrap().groups = Some(vec!["/projects/scottylabs-org".into()]);
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    let me = Req::get("/api/me")
        .header("cookie", &session_cookie_from(&res))
        .send(&app)
        .await;
    assert_eq!(me.json["permissions"]["isAdmin"], false);

    // A token without any groups claim changes nothing.
    sqlx::query("INSERT INTO role_assignments (id, user_id, role, source) SELECT $1, id, 'admin', 'keycloak_group' FROM users WHERE andrew_id = 'kcformer'")
        .bind(uuid::Uuid::now_v7())
        .execute(&db)
        .await
        .unwrap();
    mock.lock().unwrap().groups = None;
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    let me = Req::get("/api/me")
        .header("cookie", &session_cookie_from(&res))
        .send(&app)
        .await;
    assert_eq!(me.json["permissions"]["isAdmin"], true);
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_failures_return_to_sign_in_with_a_reason(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcfail", None).await;

    // Wrong code: Keycloak says invalid_grant.
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "bad-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=expired")
    );

    // No sign-in cookie: started in another browser, or too long ago.
    let started = start_login(&app, &mock, "/portal/").await;
    let res = Req::get(&format!(
        "/api/auth/callback?code=good-code&state={}",
        started.state
    ))
    .send(&app)
    .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=expired")
    );

    // Someone else's sign-in.
    let started = start_login(&app, &mock, "/portal/").await;
    let res = Req::get(&format!(
        "/api/auth/callback?code=good-code&state={}",
        started.state
    ))
    .header("cookie", "sl_signin=somebody-elses")
    .send(&app)
    .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );

    // The person backed out at Keycloak.
    let started = start_login(&app, &mock, "/portal/").await;
    let res = Req::get(&format!(
        "/api/auth/callback?error=access_denied&state={}",
        started.state
    ))
    .header("cookie", &started.attempt_cookie)
    .send(&app)
    .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=cancelled")
    );

    // Garbage state.
    let res = Req::get("/api/auth/callback?code=good-code&state=nope")
        .send(&app)
        .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );

    // An attempt older than ten minutes.
    let started = start_login(&app, &mock, "/portal/").await;
    sqlx::query("UPDATE auth_attempts SET created_at = now() - interval '11 minutes'")
        .execute(&db)
        .await
        .unwrap();
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=expired")
    );

    // Nobody was signed in by any of that.
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_rejects_tokens_for_the_wrong_nonce(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcnonce", None).await;
    let started = start_login(&app, &mock, "/portal/").await;
    mock.lock().unwrap().nonce = Some("a-different-nonce".into());
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn login_without_keycloak_says_it_is_not_configured(db: PgPool) {
    let app = testing::app(db).await;
    let res = Req::get("/api/auth/login?returnTo=/portal/")
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=not_configured")
    );
    let res = Req::get("/api/auth/callback?code=x&state=y")
        .send(&app)
        .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=not_configured")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_keycloak_down_fails_politely(db: PgPool) {
    let mut config = testing::config();
    config.oidc = Some(OidcSettings {
        keycloak_url: "http://127.0.0.1:9".into(),
        keycloak_realm: "scottylabs".into(),
        client_id: CLIENT_ID.into(),
        client_secret: CLIENT_SECRET.into(),
        oauth_relay_url: RELAY.into(),
        project_group: "/projects/scottylabs-org".into(),
        project_admin_group: ADMIN_GROUP.into(),
    });
    let app = testing::app_with(db, config).await;
    let res = Req::get("/api/auth/login").send(&app).await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn renamed_keycloak_accounts_cannot_take_over_linked_people(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kctaken", None).await;
    sqlx::query("INSERT INTO users (id, keycloak_sub, andrew_id, name) VALUES ($1, 'another-subject', 'kctaken', 'Original')")
        .bind(uuid::Uuid::now_v7())
        .execute(&db)
        .await
        .unwrap();
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
}
