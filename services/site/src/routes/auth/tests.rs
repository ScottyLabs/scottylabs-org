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
    CoreRsaPrivateSigningKey,
};
use openidconnect::{
    Audience, EndUserEmail, EndUserName, EndUserUsername, IdToken, IdTokenClaims, IssuerUrl,
    JsonWebKeyId, LocalizedClaim, PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::*;
use crate::auth::oidc::GroupClaims;
use crate::auth::revalidate::{self, Outcome};
use crate::config::OidcSettings;
use crate::models::{Committee, Role};
use crate::state::AppState;
use crate::testing::{self, Req, Res, TestUser};

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
        ("apallati", true),
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
        assert!(raw.contains("Max-Age=28800"), "{raw}");

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
async fn sessions_store_neither_the_cookie_nor_the_csrf_token(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let person = testing::user(&db, "hashperson", &[]).await;
    let (hash, stored_csrf, lifetime): (Vec<u8>, String, f64) = sqlx::query_as(
        "SELECT token_hash, csrf_token, extract(epoch FROM expires_at - created_at)::float8
         FROM sessions WHERE user_id = $1",
    )
    .bind(person.id)
    .fetch_one(&db)
    .await
    .unwrap();
    let token = person.cookie.trim_start_matches("sl_session=");
    assert_eq!(hash, session::hash_token(token));
    assert_ne!(hash, token.as_bytes());
    // The CSRF token is derived from the cookie; what the row holds is not it.
    let me = Req::get("/api/me").user(&person).send(&app).await;
    let csrf = me.json["csrfToken"].as_str().unwrap();
    assert_eq!(csrf, session::csrf_token_for(token));
    assert_ne!(csrf, stored_csrf);
    let forged = Req::post("/api/auth/logout")
        .cookie(&person)
        .header(crate::auth::csrf::CSRF_HEADER, &stored_csrf)
        .send(&app)
        .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    // Dev sessions last eight hours.
    assert!((lifetime - 8.0 * 3600.0).abs() < 5.0);
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
/// The callback itself, as production uses it instead of the relay.
const DIRECT: &str = "http://localhost:5173/api/auth/callback";
const ADMIN_GROUP: &str = "/projects/scottylabs-org/admins";
const SIGNING_KID: &str = "test-signing-key";

/// How the mock signs ID tokens. Keycloak uses RS256 with a `kid`.
#[derive(Clone, Copy, Default)]
enum Signing {
    #[default]
    Rs256,
    /// HS256 keyed by the client secret, which the site must refuse.
    Hs256,
}

#[derive(Default)]
struct Mock {
    issuer: String,
    challenge: Option<String>,
    nonce: Option<String>,
    subject: String,
    username: String,
    email: Option<String>,
    email_verified: Option<bool>,
    /// Claims besides the standard ones and `groups`, such as one `ANDREW_ID_CLAIM` names.
    extra: std::collections::BTreeMap<String, Value>,
    groups: Option<Vec<String>>,
    signing: Signing,
    /// The `redirect_uri` the client is registered with.
    redirect_uri: String,
    /// The refresh token Keycloak accepts now. Each refresh hands out a new one.
    refresh_token: Option<String>,
    refreshes: u32,
    /// Refreshes fail with `invalid_grant`, as for a disabled account or an ended Keycloak session.
    refresh_rejected: bool,
    /// The token endpoint answers 500.
    down: bool,
    /// Discovery lists an end-session endpoint.
    end_session: bool,
}

type Shared = Arc<Mutex<Mock>>;

/// A throwaway RSA key checked in for these tests only (the file says so above the key). It signs the mock's ID tokens and nothing trusts it.
const TEST_SIGNING_KEY: &str = include_str!("test-signing-key.pem");

fn signing_key() -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(
        TEST_SIGNING_KEY,
        Some(JsonWebKeyId::new(SIGNING_KID.into())),
    )
    .unwrap()
}

async fn discovery(MockState(mock): MockState<Shared>) -> axum::Json<Value> {
    let (issuer, end_session) = {
        let mock = mock.lock().unwrap();
        (mock.issuer.clone(), mock.end_session)
    };
    let mut body = json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/protocol/openid-connect/auth"),
        "token_endpoint": format!("{issuer}/protocol/openid-connect/token"),
        "jwks_uri": format!("{issuer}/protocol/openid-connect/certs"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        // Keycloak lists HMAC algorithms as well; the site accepts only RS256.
        "id_token_signing_alg_values_supported": ["RS256", "HS256"],
        "authorization_response_iss_parameter_supported": true,
    });
    if end_session {
        body["end_session_endpoint"] = json!(format!("{issuer}/protocol/openid-connect/logout"));
    }
    axum::Json(body)
}

/// Like Keycloak's: the RS256 signing key, plus an RSA key for encryption that verifiers must skip.
async fn jwks() -> axum::Json<Value> {
    let mut signing = serde_json::to_value(signing_key().as_verification_key()).unwrap();
    signing["alg"] = json!("RS256");
    let mut encryption = signing.clone();
    encryption["kid"] = json!("test-encryption-key");
    encryption["use"] = json!("enc");
    encryption["alg"] = json!("RSA-OAEP-256");
    axum::Json(json!({ "keys": [encryption, signing] }))
}

fn invalid_grant(description: &str) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(json!({ "error": "invalid_grant", "error_description": description })),
    )
        .into_response()
}

/// An ID token for the mock's person as it is now, with the given nonce.
fn id_token(mock: &Mock, nonce: Option<String>) -> String {
    let standard =
        StandardClaims::<CoreGenderClaim>::new(SubjectIdentifier::new(mock.subject.clone()))
            .set_preferred_username(Some(EndUserUsername::new(mock.username.clone())))
            .set_name(Some(LocalizedClaim::from(EndUserName::new(
                "Kay Cloak".into(),
            ))))
            .set_email(mock.email.clone().map(EndUserEmail::new))
            .set_email_verified(mock.email_verified);
    let claims = IdTokenClaims::new(
        IssuerUrl::new(mock.issuer.clone()).unwrap(),
        vec![Audience::new(CLIENT_ID.into())],
        Utc::now() + Duration::minutes(5),
        Utc::now(),
        standard,
        GroupClaims {
            groups: mock.groups.clone(),
            other: mock.extra.clone(),
        },
    )
    .set_nonce(nonce.map(openidconnect::Nonce::new));
    type SiteIdToken = IdToken<
        GroupClaims,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >;
    match mock.signing {
        Signing::Rs256 => SiteIdToken::new(
            claims,
            &signing_key(),
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            None,
            None,
        ),
        Signing::Hs256 => SiteIdToken::new(
            claims,
            &CoreHmacKey::new(CLIENT_SECRET),
            CoreJwsSigningAlgorithm::HmacSha256,
            None,
            None,
        ),
    }
    .unwrap()
    .to_string()
}

async fn token(
    MockState(mock): MockState<Shared>,
    Form(form): Form<HashMap<String, String>>,
) -> axum::response::Response {
    let mut mock = mock.lock().unwrap();
    if mock.down {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let nonce = if form.get("grant_type").map(String::as_str) == Some("refresh_token") {
        let presented = form.get("refresh_token");
        if mock.refresh_rejected || presented.is_none() || presented != mock.refresh_token.as_ref()
        {
            return invalid_grant("Token is not active");
        }
        // A refreshed ID token carries the first sign-in's nonce, as Keycloak's does.
        mock.nonce.clone()
    } else {
        let verifier = form.get("code_verifier").cloned().unwrap_or_default();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let pkce_ok = mock.challenge.as_deref() == Some(challenge.as_str());
        if form.get("code").map(String::as_str) != Some("good-code")
            || !pkce_ok
            || form.get("redirect_uri") != Some(&mock.redirect_uri)
        {
            return invalid_grant("Code not valid");
        }
        mock.nonce.clone()
    };
    mock.refreshes += 1;
    let refresh_token = format!("mock-refresh-token-{}", mock.refreshes);
    mock.refresh_token = Some(refresh_token.clone());
    axum::Json(json!({
        "access_token": "mock-access-token",
        "token_type": "bearer",
        "expires_in": 300,
        "refresh_token": refresh_token,
        "id_token": id_token(&mock, nonce),
    }))
    .into_response()
}

/// Starts the mock and returns an app, and its state, configured to sign in through it.
async fn keycloak_state(
    db: &PgPool,
    username: &str,
    groups: Option<Vec<&str>>,
    configure: impl FnOnce(&mut OidcSettings),
) -> (Router, Shared, AppState) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut settings = OidcSettings {
        keycloak_url: base.clone(),
        keycloak_realm: "scottylabs".into(),
        client_id: CLIENT_ID.into(),
        client_secret: CLIENT_SECRET.into(),
        oauth_relay_url: RELAY.into(),
        project_group: "/projects/scottylabs-org".into(),
        project_admin_group: ADMIN_GROUP.into(),
        andrew_id_claim: None,
    };
    configure(&mut settings);
    let mock: Shared = Arc::new(Mutex::new(Mock {
        issuer: format!("{base}/realms/scottylabs"),
        subject: format!("sub-{username}"),
        username: username.into(),
        email: Some(format!("{username}@andrew.cmu.edu")),
        email_verified: Some(true),
        groups: groups.map(|list| list.into_iter().map(str::to_owned).collect()),
        redirect_uri: settings.oauth_relay_url.clone(),
        end_session: true,
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
    config.oidc = Some(settings);
    testing::ensure_committees(db).await;
    let state = AppState::new(db.clone(), Arc::new(config)).await.unwrap();
    (crate::routes::router(state.clone()), mock, state)
}

async fn keycloak_app(db: &PgPool, username: &str, groups: Option<Vec<&str>>) -> (Router, Shared) {
    let (app, mock, _) = keycloak_state(db, username, groups, |_| {}).await;
    (app, mock)
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
    let location = url::Url::parse(res.header("location").unwrap())
        .unwrap_or_else(|_| panic!("login went to {:?}", res.header("location")));
    let params: HashMap<String, String> = location.query_pairs().into_owned().collect();
    assert!(location.path().ends_with("/protocol/openid-connect/auth"));
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["client_id"], CLIENT_ID);
    assert_eq!(params["redirect_uri"], mock.lock().unwrap().redirect_uri);
    assert_eq!(params["code_challenge_method"], "S256");
    // Exactly these: microprofile-jwt would overwrite the groups claim.
    let mut scopes: Vec<&str> = params["scope"].split(' ').collect();
    scopes.sort_unstable();
    assert_eq!(scopes, ["email", "openid", "profile"]);
    assert!(!params["nonce"].is_empty());
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
    callback_with(app, started, &format!("code={code}")).await
}

/// The callback as Ricochet forwards it, or as Keycloak sends it straight back: `state` plus whatever Keycloak sent.
async fn callback_with(app: &Router, started: &Started, params: &str) -> Res {
    Req::get(&format!(
        "/api/auth/callback?state={}&{params}",
        started.state
    ))
    .header("cookie", &started.attempt_cookie)
    .send(app)
    .await
}

/// A full Keycloak sign-in, returning the session cookie.
async fn keycloak_sign_in(app: &Router, mock: &Shared) -> String {
    let started = start_login(app, mock, "/portal/").await;
    let res = finish_login(app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/"),
        "{:?}",
        res.headers
    );
    session_cookie_from(&res)
}

/// A signed-in Keycloak session as a test user, so writes carry its CSRF token.
async fn as_user(db: &PgPool, cookie: &str) -> TestUser {
    let token = cookie.trim_start_matches("sl_session=");
    let (id, andrew_id): (uuid::Uuid, String) = sqlx::query_as(
        "SELECT u.id, u.andrew_id FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = $1",
    )
    .bind(session::hash_token(token))
    .fetch_one(db)
    .await
    .unwrap();
    TestUser {
        id,
        andrew_id,
        cookie: cookie.to_owned(),
        csrf: session::csrf_token_for(token),
    }
}

async fn session_id_of(db: &PgPool, cookie: &str) -> uuid::Uuid {
    let token = cookie.trim_start_matches("sl_session=");
    sqlx::query_scalar("SELECT id FROM sessions WHERE token_hash = $1")
        .bind(session::hash_token(token))
        .fetch_one(db)
        .await
        .unwrap()
}

async fn session_count(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(db)
        .await
        .unwrap()
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
        me.json["csrfToken"],
        session::csrf_token_for(session.trim_start_matches("sl_session="))
    );
    assert_eq!(
        testing::audit_summaries(&db).await,
        [
            "Kay Cloak became an admin through the Keycloak admin group",
            "Kay Cloak signed in with ScottyLabs"
        ]
    );

    // The session keeps Keycloak's tokens on the server, counts as confirmed now and lasts seven days.
    let (refresh, id_token, confirmed, lifetime): (Option<String>, Option<String>, bool, f64) =
        sqlx::query_as(
            "SELECT refresh_token, id_token, validated_at > now() - interval '1 minute',
                    extract(epoch FROM expires_at - created_at)::float8
             FROM sessions",
        )
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(refresh.as_deref(), Some("mock-refresh-token-1"));
    assert!(id_token.is_some_and(|token| token.split('.').count() == 3));
    assert!(confirmed);
    assert!((lifetime - 7.0 * 24.0 * 3600.0).abs() < 5.0);
    let raw = res
        .headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with("sl_session="))
        .unwrap();
    assert!(raw.contains("Max-Age=604800"), "{raw}");
    // Nothing the browser sees is stored.
    assert!(!me.text().contains("mock-refresh-token"));

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
    assert_eq!(
        testing::audit_summaries(&db).await,
        [
            "Linked the ScottyLabs account of Kay Cloak to kclead, who was added before signing in",
            "Kay Cloak signed in with ScottyLabs"
        ]
    );
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

    // Keycloak sends an empty list for someone in no group at all: that also means they left.
    sqlx::query("INSERT INTO role_assignments (id, user_id, role, source) SELECT $1, id, 'admin', 'keycloak_group' FROM users WHERE andrew_id = 'kcformer'")
        .bind(uuid::Uuid::now_v7())
        .execute(&db)
        .await
        .unwrap();
    mock.lock().unwrap().groups = Some(Vec::new());
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    let me = Req::get("/api/me")
        .header("cookie", &session_cookie_from(&res))
        .send(&app)
        .await;
    assert_eq!(me.json["permissions"]["isAdmin"], false);
    assert_eq!(me.json["roles"], json!([]));

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
    assert_eq!(session_count(&db).await, 0);
    // None of these proved who anyone is, and anyone can cause them, so they are only logged: the audit log only grows.
    assert!(testing::audit_summaries(&db).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn anonymous_failures_cannot_fill_recent_activity(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcflood", None).await;
    let admin = testing::user(&db, "feedadmin", &[(Role::Admin, None)]).await;
    let director = testing::user(
        &db,
        "feeddirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    let res = Req::patch(&format!(
        "/api/people/assignments/{}",
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM role_assignments WHERE user_id = $1")
            .bind(director.id)
            .fetch_one(&db)
            .await
            .unwrap()
    ))
    .user(&admin)
    .json(json!({ "role": "director", "committee": "labrador" }))
    .send(&app)
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    // A script starting sign-ins and coming back with an error, without cookies.
    for _ in 0..12 {
        let started = start_login(&app, &mock, "/portal/").await;
        let res = Req::get(&format!(
            "/api/auth/callback?error=access_denied&state={}",
            started.state
        ))
        .send(&app)
        .await;
        assert_eq!(
            res.header("location"),
            Some("/portal/sign-in/?error=cancelled")
        );
    }
    // Real sign-ins are audited but stay out of Recent activity, so they cannot bury other entries either.
    keycloak_sign_in(&app, &mock).await;
    let summaries = testing::audit_summaries(&db).await;
    assert_eq!(summaries.len(), 2, "{summaries:?}");
    let activity = Req::get("/api/activity?limit=10")
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(activity.status, StatusCode::OK);
    let shown: Vec<&str> = activity.json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["summary"].as_str())
        .collect();
    assert_eq!(
        shown,
        ["Changed Test feeddirector from director of Events to director of Labrador"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_callbacks_must_name_the_right_issuer(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kciss", None).await;
    let issuer = mock.lock().unwrap().issuer.clone();

    // Ricochet forwards every parameter Keycloak sends, including iss and session_state.
    let started = start_login(&app, &mock, "/portal/ticker/").await;
    let res = callback_with(
        &app,
        &started,
        &format!(
            "session_state=abc-123&iss={}&code=good-code",
            urlencode(&issuer)
        ),
    )
    .await;
    assert_eq!(res.header("location"), Some("/portal/ticker/"));
    assert_eq!(session_count(&db).await, 1);

    // Another issuer: refused, and the attempt is used up so it cannot be retried.
    let started = start_login(&app, &mock, "/portal/").await;
    let res = callback_with(
        &app,
        &started,
        &format!(
            "iss={}&code=good-code",
            urlencode("https://idp.example.com/realms/scottylabs")
        ),
    )
    .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
    let retry = callback_with(
        &app,
        &started,
        &format!("iss={}&code=good-code", urlencode(&issuer)),
    )
    .await;
    assert_eq!(
        retry.header("location"),
        Some("/portal/sign-in/?error=expired")
    );

    // The issuer must match exactly, not just start the same way.
    let started = start_login(&app, &mock, "/portal/").await;
    let res = callback_with(
        &app,
        &started,
        &format!("iss={}&code=good-code", urlencode(&format!("{issuer}/"))),
    )
    .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
    assert_eq!(session_count(&db).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_errors_use_up_the_attempt(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcerror", None).await;
    let issuer = mock.lock().unwrap().issuer.clone();
    let started = start_login(&app, &mock, "/portal/").await;
    let res = callback_with(
        &app,
        &started,
        &format!(
            "error=invalid_request&error_description={}&iss={}",
            urlencode("Missing parameter: code_challenge_method"),
            urlencode(&issuer)
        ),
    )
    .await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_attempts")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(
        finish_login(&app, &started, "good-code")
            .await
            .header("location"),
        Some("/portal/sign-in/?error=expired")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_tokens_signed_with_the_client_secret_are_refused(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kchmac", Some(vec![ADMIN_GROUP])).await;
    mock.lock().unwrap().signing = Signing::Hs256;
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=failed")
    );
    assert_eq!(session_count(&db).await, 0);
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(users, 0);

    // The same sign-in signed with RS256 goes through.
    mock.lock().unwrap().signing = Signing::Rs256;
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(res.header("location"), Some("/portal/"));
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
        andrew_id_claim: None,
    });
    let app = testing::app_with(db, config).await;
    let res = Req::get("/api/auth/login").send(&app).await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=unavailable")
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
        Some("/portal/sign-in/?error=linked_elsewhere")
    );
    assert_eq!(session_count(&db).await, 0);
    assert_eq!(
        testing::audit_summaries(&db).await,
        [
            "Refused a ScottyLabs sign-in as kctaken: that Andrew ID is linked to a different ScottyLabs account"
        ]
    );
}

// ---------------------------------------------------------------------------
// Production: Keycloak straight back to the callback

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_can_redirect_straight_to_the_callback(db: PgPool) {
    let (app, mock, _) = keycloak_state(&db, "kcdirect", None, |settings| {
        settings.oauth_relay_url = DIRECT.into();
    })
    .await;
    let issuer = mock.lock().unwrap().issuer.clone();
    // Keycloak sends the browser straight to /api/auth/callback with code, state, session_state and iss, and the code exchange names the callback as redirect_uri.
    let started = start_login(&app, &mock, "/portal/projects/").await;
    let res = callback_with(
        &app,
        &started,
        &format!(
            "session_state=abc-123&iss={}&code=good-code",
            urlencode(&issuer)
        ),
    )
    .await;
    assert_eq!(res.header("location"), Some("/portal/projects/"));
    let me = Req::get("/api/me")
        .header("cookie", &session_cookie_from(&res))
        .send(&app)
        .await;
    assert_eq!(me.json["user"]["andrewId"], "kcdirect");
}

// ---------------------------------------------------------------------------
// Which accounts count as an Andrew ID

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_sign_in_needs_a_cmu_andrew_id(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcdirector", Some(vec![ADMIN_GROUP])).await;
    // A director added by Andrew ID who has not signed in yet.
    let director = testing::user(
        &db,
        "kcdirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    sqlx::query("DELETE FROM sessions")
        .execute(&db)
        .await
        .unwrap();

    for (email, verified) in [
        (Some("someone.else@gmail.com"), Some(true)),
        (Some("kcdirectorx@andrew.cmu.edu"), Some(true)),
        (Some("kcdirector@andrew.cmu.edu"), Some(false)),
        (None, None),
    ] {
        {
            let mut mock = mock.lock().unwrap();
            mock.email = email.map(str::to_owned);
            mock.email_verified = verified;
        }
        let started = start_login(&app, &mock, "/portal/").await;
        let res = finish_login(&app, &started, "good-code").await;
        assert_eq!(
            res.header("location"),
            Some("/portal/sign-in/?error=no_andrew_id"),
            "{email:?}"
        );
    }
    // Nothing was linked or created, and nobody got a session or a role.
    let linked: Option<String> = sqlx::query_scalar("SELECT keycloak_sub FROM users WHERE id = $1")
        .bind(director.id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(linked, None);
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(users, 1);
    assert_eq!(session_count(&db).await, 0);
    let admins: i64 =
        sqlx::query_scalar("SELECT count(*) FROM role_assignments WHERE role = 'admin'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(admins, 0);
    assert_eq!(
        testing::audit_summaries(&db).await,
        [
            "Refused a ScottyLabs sign-in as kcdirector: the account could not be matched to that CMU Andrew ID (email_not_andrew_id)",
            "Refused a ScottyLabs sign-in as kcdirector: the account could not be matched to that CMU Andrew ID (email_not_andrew_id)",
            "Refused a ScottyLabs sign-in as kcdirector: the account could not be matched to that CMU Andrew ID (email_not_verified)",
            "Refused a ScottyLabs sign-in as kcdirector: the account could not be matched to that CMU Andrew ID (no_email)"
        ]
    );

    // The CMU account itself signs in, with or without email_verified, on either CMU domain.
    {
        let mut mock = mock.lock().unwrap();
        mock.email = Some("KCDirector@cmu.edu".into());
        mock.email_verified = None;
    }
    let cookie = keycloak_sign_in(&app, &mock).await;
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.json["user"]["id"], director.id.to_string());
    assert_eq!(me.json["permissions"]["directorOf"], json!(["events"]));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_configured_andrew_id_claim_decides(db: PgPool) {
    let (app, mock, _) = keycloak_state(&db, "kcclaim", None, |settings| {
        settings.andrew_id_claim = Some("andrew_id".into());
    })
    .await;
    {
        // The username and email say nothing about CMU; the claim does.
        let mut mock = mock.lock().unwrap();
        mock.username = "kay".into();
        mock.email = Some("kay@example.org".into());
        mock.extra.insert("andrew_id".into(), json!("kcclaim"));
    }
    let cookie = keycloak_sign_in(&app, &mock).await;
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.json["user"]["andrewId"], "kcclaim");

    // Without the claim, the username and a matching email are not enough.
    {
        let mut mock = mock.lock().unwrap();
        mock.extra.clear();
        mock.subject = "sub-other".into();
        mock.username = "kcother".into();
        mock.email = Some("kcother@andrew.cmu.edu".into());
    }
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=no_andrew_id")
    );
}

// ---------------------------------------------------------------------------
// Sessions and Keycloak

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_sign_in_ends_the_browsers_previous_session(db: PgPool) {
    testing::seed_personas(&db).await;
    let (app, mock) = keycloak_app(&db, "kcnext", None).await;
    let earlier = session_cookie_from(&dev_login_as(&app, "tkanz", None).await);
    let started = start_login(&app, &mock, "/portal/").await;
    let res = Req::get(&format!(
        "/api/auth/callback?state={}&code=good-code",
        started.state
    ))
    .header("cookie", &format!("{}; {earlier}", started.attempt_cookie))
    .send(&app)
    .await;
    assert_eq!(res.header("location"), Some("/portal/"));
    let old = Req::get("/api/me")
        .header("cookie", &earlier)
        .send(&app)
        .await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    assert_eq!(session_count(&db).await, 1);
}

/// Marks a session as last confirmed by Keycloak `minutes` ago.
async fn confirmed_ago(db: &PgPool, cookie: &str, minutes: i32) {
    let id = session_id_of(db, cookie).await;
    sqlx::query(
        "UPDATE sessions SET validated_at = now() - make_interval(mins => $2), validation_retry_at = NULL
         WHERE id = $1",
    )
    .bind(id)
    .bind(minutes)
    .execute(db)
    .await
    .unwrap();
}

async fn is_admin(app: &Router, cookie: &str) -> Value {
    let me = Req::get("/api/me").header("cookie", cookie).send(app).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.text());
    me.json["permissions"]["isAdmin"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn keycloak_sessions_are_confirmed_and_follow_the_admin_group(db: PgPool) {
    testing::user(&db, "otheradmin", &[(Role::Admin, None)]).await;
    let (app, mock, state) = keycloak_state(&db, "kcgroup", Some(vec![ADMIN_GROUP]), |_| {}).await;
    let oidc = state.oidc.clone().unwrap();
    let cookie = keycloak_sign_in(&app, &mock).await;
    let session = session_id_of(&db, &cookie).await;

    // Confirmed at sign-in, so not due yet.
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::NotDue
    );

    // Fifteen minutes on, Keycloak confirms it and hands out a new refresh token.
    confirmed_ago(&db, &cookie, 16).await;
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Confirmed
    );
    let (refresh, fresh): (String, bool) = sqlx::query_as(
        "SELECT refresh_token, validated_at > now() - interval '1 minute' FROM sessions WHERE id = $1",
    )
    .bind(session)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(refresh, "mock-refresh-token-2");
    assert!(fresh);
    assert_eq!(is_admin(&app, &cookie).await, true);

    // Removed from the admin group: the next check takes the role away, without a new sign-in.
    mock.lock().unwrap().groups = Some(vec!["/projects/scottylabs-org".into()]);
    confirmed_ago(&db, &cookie, 16).await;
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Confirmed
    );
    assert_eq!(is_admin(&app, &cookie).await, false);

    // Added again: the role comes back the same way.
    mock.lock().unwrap().groups = Some(vec![ADMIN_GROUP.into()]);
    confirmed_ago(&db, &cookie, 16).await;
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Confirmed
    );
    assert_eq!(is_admin(&app, &cookie).await, true);
    assert_eq!(
        testing::audit_summaries(&db).await,
        [
            "Kay Cloak became an admin through the Keycloak admin group",
            "Kay Cloak signed in with ScottyLabs",
            "Kay Cloak is no longer an admin after leaving the Keycloak admin group",
            "Kay Cloak became an admin through the Keycloak admin group"
        ]
    );
}

async fn opt_outs(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM admin_group_opt_outs")
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn group_admins_removed_on_the_people_page_stay_removed(db: PgPool) {
    let other = testing::user(&db, "otheradmin", &[(Role::Admin, None)]).await;
    let (app, mock, state) = keycloak_state(&db, "kcstolen", Some(vec![ADMIN_GROUP]), |_| {}).await;
    let oidc = state.oidc.clone().unwrap();
    // Say this session was copied, and the person also leads in Events, so removing the admin role does not sign them out.
    let cookie = keycloak_sign_in(&app, &mock).await;
    let person = as_user(&db, &cookie).await;
    testing::grant(&db, person.id, Role::Lead, Some(Committee::Events)).await;
    let session = session_id_of(&db, &cookie).await;
    let admin_role = || async {
        sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM role_assignments WHERE user_id = $1 AND role = 'admin'",
        )
        .bind(person.id)
        .fetch_one(&db)
        .await
        .unwrap()
    };
    let res = Req::delete(&format!("/api/people/assignments/{}", admin_role().await))
        .user(&other)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text());
    assert_eq!(is_admin(&app, &cookie).await, false);

    // The session's next check with Keycloak, which still lists the group, does not give it back.
    confirmed_ago(&db, &cookie, 16).await;
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Confirmed
    );
    assert_eq!(is_admin(&app, &cookie).await, false);
    // Nor does a new sign-in.
    let again = keycloak_sign_in(&app, &mock).await;
    assert_eq!(is_admin(&app, &again).await, false);
    assert_eq!(opt_outs(&db).await, 1);

    // Once Keycloak shows they left the group, the opt-out is forgotten, so being added again later counts.
    mock.lock().unwrap().groups = Some(vec!["/projects/scottylabs-org".into()]);
    keycloak_sign_in(&app, &mock).await;
    assert_eq!(opt_outs(&db).await, 0);
    mock.lock().unwrap().groups = Some(vec![ADMIN_GROUP.into()]);
    let back = keycloak_sign_in(&app, &mock).await;
    assert_eq!(is_admin(&app, &back).await, true);

    // Lowering the role to a director also keeps the group from giving it back; giving the admin role on the People page again clears that.
    let res = Req::patch(&format!("/api/people/assignments/{}", admin_role().await))
        .user(&other)
        .json(json!({ "role": "director", "committee": "tech" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(opt_outs(&db).await, 1);
    let res = Req::post("/api/people")
        .user(&other)
        .json(json!({ "andrewId": "kcstolen", "role": "admin" }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text());
    assert_eq!(opt_outs(&db).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_keycloak_refuses_end_at_once(db: PgPool) {
    let (app, mock, state) = keycloak_state(&db, "kcgone", Some(vec![ADMIN_GROUP]), |_| {}).await;
    let oidc = state.oidc.clone().unwrap();
    let cookie = keycloak_sign_in(&app, &mock).await;
    let session = session_id_of(&db, &cookie).await;

    // The account was disabled, or the person signed out of Keycloak.
    mock.lock().unwrap().refresh_rejected = true;
    confirmed_ago(&db, &cookie, 16).await;
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Ended
    );
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(session_count(&db).await, 0);
    assert_eq!(
        testing::audit_summaries(&db)
            .await
            .last()
            .map(String::as_str),
        Some("Ended a session of Kay Cloak because ScottyLabs sign-in no longer accepts it")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sessions_outlive_a_keycloak_outage_but_group_admin_lapses(db: PgPool) {
    let (app, mock, state) = keycloak_state(&db, "kcdown", Some(vec![ADMIN_GROUP]), |_| {}).await;
    let oidc = state.oidc.clone().unwrap();
    let cookie = keycloak_sign_in(&app, &mock).await;
    let session = session_id_of(&db, &cookie).await;
    let portal_admin = testing::user(&db, "portaladmin", &[(Role::Admin, None)]).await;

    // Keycloak answers 500: the session is kept, and asked about again in five minutes.
    mock.lock().unwrap().down = true;
    confirmed_ago(&db, &cookie, 16).await;
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Unconfirmed
    );
    let retry_in: f64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM validation_retry_at - now())::float8 FROM sessions WHERE id = $1",
    )
    .bind(session)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!((280.0..=300.0).contains(&retry_in), "{retry_in}");
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::NotDue
    );
    assert_eq!(is_admin(&app, &cookie).await, true);

    // A day without a confirmation: the group's admin role stops counting, the session does not.
    confirmed_ago(&db, &cookie, 24 * 60 + 1).await;
    sqlx::query(
        "UPDATE sessions SET validation_retry_at = now() + interval '5 minutes' WHERE id = $1",
    )
    .bind(session)
    .execute(&db)
    .await
    .unwrap();
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json["permissions"]["isAdmin"], false);
    assert_eq!(me.json["permissions"]["canUsePortal"], false);
    let people = Req::get("/api/people")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(people.status, StatusCode::FORBIDDEN);
    // Admins added in the portal, and dev sessions, are not affected.
    let other = Req::get("/api/people").user(&portal_admin).send(&app).await;
    assert_eq!(other.status, StatusCode::OK);

    // Keycloak is back: the next check confirms the session and the role counts again.
    mock.lock().unwrap().down = false;
    sqlx::query("UPDATE sessions SET validation_retry_at = NULL WHERE id = $1")
        .bind(session)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        revalidate::check(&state, &oidc, session).await.unwrap(),
        Outcome::Confirmed
    );
    assert_eq!(is_admin(&app, &cookie).await, true);
}

#[sqlx::test(migrations = "./migrations")]
async fn requests_check_due_sessions_in_the_background(db: PgPool) {
    let (app, mock, _) = keycloak_state(&db, "kcbusy", None, |_| {}).await;
    let cookie = keycloak_sign_in(&app, &mock).await;
    let session = session_id_of(&db, &cookie).await;
    confirmed_ago(&db, &cookie, 16).await;
    mock.lock().unwrap().refresh_rejected = true;

    // The request itself goes on; the check ends the session right after.
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::OK);
    let mut gone = false;
    for _ in 0..50 {
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE id = $1")
            .bind(session)
            .fetch_one(&db)
            .await
            .unwrap();
        if left == 0 {
            gone = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(gone, "the background check did not end the session");
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_keycloak_outage_during_the_code_exchange_says_unavailable(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcflaky", None).await;
    let started = start_login(&app, &mock, "/portal/").await;
    mock.lock().unwrap().down = true;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=unavailable")
    );
    // Logged, not audited: an outage would otherwise write one entry per visitor.
    assert!(testing::audit_summaries(&db).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn logout_offers_to_end_the_keycloak_session(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcbye", None).await;
    let cookie = keycloak_sign_in(&app, &mock).await;
    let id_token: String = sqlx::query_scalar("SELECT id_token FROM sessions")
        .fetch_one(&db)
        .await
        .unwrap();
    let user = as_user(&db, &cookie).await;
    let res = Req::post("/api/auth/logout").user(&user).send(&app).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert!(res.header("set-cookie").unwrap().contains("Max-Age=0"));
    let url = url::Url::parse(res.json["endSessionUrl"].as_str().unwrap()).unwrap();
    let issuer = mock.lock().unwrap().issuer.clone();
    assert_eq!(
        url.as_str().split('?').next(),
        Some(format!("{issuer}/protocol/openid-connect/logout").as_str())
    );
    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(params["id_token_hint"], id_token);
    assert_eq!(params["client_id"], CLIENT_ID);
    assert_eq!(session_count(&db).await, 0);
    assert_eq!(
        testing::audit_summaries(&db)
            .await
            .last()
            .map(String::as_str),
        Some("Kay Cloak signed out")
    );

    // Without an end-session endpoint, or for a dev session, it is a plain 204.
    let (app, mock) = keycloak_app(&db, "kcbye2", None).await;
    mock.lock().unwrap().end_session = false;
    let cookie = keycloak_sign_in(&app, &mock).await;
    let user = as_user(&db, &cookie).await;
    let res = Req::post("/api/auth/logout").user(&user).send(&app).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let dev = testing::user(&db, "devbye", &[]).await;
    let res = Req::post("/api/auth/logout").user(&dev).send(&app).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
}

// ---------------------------------------------------------------------------
// Admins: signing people out and resetting Keycloak links

#[sqlx::test(migrations = "./migrations")]
async fn admins_can_sign_people_out_everywhere(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = testing::user(&db, "endadmin", &[(Role::Admin, None)]).await;
    let director = testing::user(
        &db,
        "enddirector",
        &[(Role::Director, Some(Committee::Events))],
    )
    .await;
    let second = testing::session_for(&db, director.id).await;
    let path = format!("/api/people/{}/sessions", director.id);

    // Directors cannot, even for leads in their committee.
    let res = Req::delete(&format!("/api/people/{}/sessions", admin.id))
        .user(&director)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    let res = Req::delete(&path).user(&admin).send(&app).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json, json!({ "ended": 2 }));
    for user in [&director, &second] {
        let me = Req::get("/api/me").user(user).send(&app).await;
        assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    }
    // Their role stays; only the sessions ended.
    let roles: i64 = sqlx::query_scalar("SELECT count(*) FROM role_assignments WHERE user_id = $1")
        .bind(director.id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(roles, 1);
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Signed Test enddirector out everywhere"]
    );

    for missing in [uuid::Uuid::now_v7().to_string(), "not-an-id".into()] {
        let res = Req::delete(&format!("/api/people/{missing}/sessions"))
            .user(&admin)
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::NOT_FOUND);
    }

    // Signing yourself out everywhere also clears this browser's cookie.
    let res = Req::delete(&format!("/api/people/{}/sessions", admin.id))
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.header("set-cookie").unwrap().contains("Max-Age=0"));
}

#[sqlx::test(migrations = "./migrations")]
async fn admins_can_reset_a_keycloak_link(db: PgPool) {
    let (app, mock) = keycloak_app(&db, "kcmoved", None).await;
    let admin = testing::user(&db, "linkadmin", &[(Role::Admin, None)]).await;
    let director = testing::user(
        &db,
        "linkdirector",
        &[(Role::Director, Some(Committee::Tech))],
    )
    .await;
    let cookie = keycloak_sign_in(&app, &mock).await;
    let moved = as_user(&db, &cookie).await;
    testing::grant(&db, moved.id, Role::Lead, Some(Committee::Tech)).await;

    // Keycloak was rebuilt and gave the same person a new subject: refused until an admin resets the link.
    mock.lock().unwrap().subject = "sub-kcmoved-rebuilt".into();
    let started = start_login(&app, &mock, "/portal/").await;
    let res = finish_login(&app, &started, "good-code").await;
    assert_eq!(
        res.header("location"),
        Some("/portal/sign-in/?error=linked_elsewhere")
    );

    let path = format!("/api/people/{}/keycloak-link", moved.id);
    let res = Req::delete(&path).user(&director).send(&app).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = Req::delete(&path).user(&admin).send(&app).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text());
    // The old session ended with the link.
    let me = Req::get("/api/me").user(&moved).send(&app).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);

    // The next sign-in links the new account and keeps the person's roles.
    let cookie = keycloak_sign_in(&app, &mock).await;
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.json["user"]["id"], moved.id.to_string());
    assert_eq!(me.json["permissions"]["leadOf"], json!(["tech"]));
    let summaries = testing::audit_summaries(&db).await;
    assert!(summaries.contains(
        &"Reset the ScottyLabs sign-in link of Kay Cloak; their next sign-in links it again".into()
    ));
    // Resetting someone who is not linked changes nothing.
    let res = Req::delete(&format!("/api/people/{}/keycloak-link", director.id))
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let me = Req::get("/api/me").user(&director).send(&app).await;
    assert_eq!(me.status, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn removing_the_last_role_signs_the_person_out(db: PgPool) {
    let app = testing::app(db.clone()).await;
    let admin = testing::user(&db, "roleadmin", &[(Role::Admin, None)]).await;
    let lead = testing::user(
        &db,
        "twolead",
        &[
            (Role::Lead, Some(Committee::Events)),
            (Role::Lead, Some(Committee::Tech)),
        ],
    )
    .await;
    let ids: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM role_assignments WHERE user_id = $1 ORDER BY committee")
            .bind(lead.id)
            .fetch_all(&db)
            .await
            .unwrap();
    // One role left: still signed in.
    let res = Req::delete(&format!("/api/people/assignments/{}", ids[0]))
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        Req::get("/api/me").user(&lead).send(&app).await.status,
        StatusCode::OK
    );
    // No role left: signed out everywhere.
    let res = Req::delete(&format!("/api/people/assignments/{}", ids[1]))
        .user(&admin)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        Req::get("/api/me").user(&lead).send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );
}

// ---------------------------------------------------------------------------
// Dev sign-in on previews

fn preview_config(key: &str) -> crate::config::Config {
    let mut preview = testing::config();
    preview.app_url = url::Url::parse("https://scottylabs-org-site-pr-3.scottylabs.net").unwrap();
    preview.dev_login = DevLogin::Keyed(key.into());
    preview
}

const PREVIEW_KEY: &str = "preview-key-0123456789";

fn secure_session_cookie(res: &Res) -> String {
    let raw = res
        .headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("__Host-sl_session="))
        .expect("a session cookie");
    raw.split(';').next().unwrap().to_owned()
}

#[sqlx::test(migrations = "./migrations")]
async fn previews_list_the_personas_only_with_the_key(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app_with(db, preview_config(PREVIEW_KEY)).await;
    let config = Req::get("/api/auth/config").send(&app).await;
    assert_eq!(config.json["devLogin"], true);
    assert_eq!(config.json["devLoginNeedsKey"], true);
    assert_eq!(config.json["devPersonas"], json!([]));
    assert!(!config.text().contains("Thomas Kanz"));

    for key in [json!(null), json!(""), json!("preview-key-012345678")] {
        let res = Req::post("/api/auth/dev-personas")
            .json(json!({ "key": key }))
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{key}");
        assert!(!res.text().contains("Thomas Kanz"));
    }
    let res = Req::post("/api/auth/dev-personas")
        .json(json!({ "key": PREVIEW_KEY }))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.json["personas"].as_array().unwrap().len(), 5);
    assert_eq!(res.json["personas"][0]["andrewId"], "tkanz");
    assert_eq!(res.header("cache-control"), Some("no-store"));
}

#[sqlx::test(migrations = "./migrations")]
async fn preview_dev_sessions_end_when_the_key_changes_or_dev_sign_in_turns_off(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app_with(db.clone(), preview_config(PREVIEW_KEY)).await;
    let res = dev_login_as(&app, "tkanz", Some(PREVIEW_KEY)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(res.header("set-cookie").unwrap().contains("Max-Age=28800"));
    let cookie = secure_session_cookie(&res);
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::OK);

    // A new key, as after rotating a leaked one.
    let rotated = testing::app_with(db.clone(), preview_config("another-key-9876543210")).await;
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&rotated)
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    // Dev sign-in turned off.
    let mut off = preview_config(PREVIEW_KEY);
    off.dev_login = DevLogin::Off;
    let off = testing::app_with(db.clone(), off).await;
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&off)
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    // Back to the key it was made with: the session ended for good, it was not only refused.
    let me = Req::get("/api/me")
        .header("cookie", &cookie)
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(session_count(&db).await, 0);
    assert_eq!(
        testing::audit_summaries(&db).await,
        ["Thomas Kanz signed in with dev sign-in"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn startup_ends_dev_sessions_the_configuration_no_longer_allows(db: PgPool) {
    testing::seed_personas(&db).await;
    let app = testing::app_with(db.clone(), preview_config(PREVIEW_KEY)).await;
    for andrew_id in ["tkanz", "evanfost"] {
        let res = dev_login_as(&app, andrew_id, Some(PREVIEW_KEY)).await;
        assert_eq!(res.status, StatusCode::NO_CONTENT);
    }
    // A Keycloak session is never touched.
    let person = testing::user(&db, "kcperson", &[(Role::Admin, None)]).await;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(person.id)
        .execute(&db)
        .await
        .unwrap();
    let mut conn = db.acquire().await.unwrap();
    session::create(&mut conn, person.id, crate::models::AuthMode::Oidc)
        .await
        .unwrap();
    drop(conn);
    let ended = session::end_disallowed_dev_sessions(&db, &preview_config(PREVIEW_KEY))
        .await
        .unwrap();
    assert_eq!(ended, 0);
    // The real evanfost signed in with Keycloak since.
    sqlx::query("UPDATE users SET keycloak_sub = 'sub-real-evanfost' WHERE andrew_id = 'evanfost'")
        .execute(&db)
        .await
        .unwrap();
    let ended = session::end_disallowed_dev_sessions(&db, &preview_config(PREVIEW_KEY))
        .await
        .unwrap();
    assert_eq!(ended, 1);
    // A new key ends the rest; other sessions stay.
    let ended =
        session::end_disallowed_dev_sessions(&db, &preview_config("rotated-key-0000000000"))
            .await
            .unwrap();
    assert_eq!(ended, 1);
    let res = dev_login_as(&app, "tkanz", Some(PREVIEW_KEY)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let mut off = preview_config(PREVIEW_KEY);
    off.dev_login = DevLogin::Off;
    assert_eq!(
        session::end_disallowed_dev_sessions(&db, &off)
            .await
            .unwrap(),
        1
    );
    assert_eq!(session_count(&db).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn preview_dev_sign_in_never_acts_as_a_linked_account(db: PgPool) {
    testing::seed_personas(&db).await;
    // The real apallati signed in to this preview with Keycloak.
    sqlx::query("UPDATE users SET keycloak_sub = 'sub-real-apallati' WHERE andrew_id = 'apallati'")
        .execute(&db)
        .await
        .unwrap();
    let app = testing::app_with(db.clone(), preview_config(PREVIEW_KEY)).await;
    let res = dev_login_as(&app, "apallati", Some(PREVIEW_KEY)).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        res.json["error"]["fields"]["andrewId"]
            .as_str()
            .unwrap()
            .starts_with("That person has signed in here with their ScottyLabs account")
    );
    let evan = dev_login_as(&app, "evanfost", Some(PREVIEW_KEY)).await;
    assert_eq!(evan.status, StatusCode::NO_CONTENT);
    let evan = secure_session_cookie(&evan);
    // A wrong key is refused and only logged: anyone can send one.
    assert_eq!(
        dev_login_as(&app, "evanfost", Some("wrong-key-000000000000"))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        testing::audit_summaries(&db).await,
        [
            "A dev sign-in as apallati was refused: that person signs in with ScottyLabs here (linked_account)",
            "Evan Foster signed in with dev sign-in"
        ]
    );

    // The real evanfost signs in with Keycloak: the dev session made before that ends at its next request.
    sqlx::query(
        "UPDATE users SET keycloak_sub = 'sub-real-evanfost', keycloak_linked_at = now() WHERE andrew_id = 'evanfost'",
    )
    .execute(&db)
    .await
    .unwrap();
    let me = Req::get("/api/me").header("cookie", &evan).send(&app).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(session_count(&db).await, 0);

    // The persona admin resets the real apallati's link: dev sign-in still cannot act as them.
    let tk = dev_login_as(&app, "tkanz", Some(PREVIEW_KEY)).await;
    let tk = secure_session_cookie(&tk);
    let me = Req::get("/api/me").header("cookie", &tk).send(&app).await;
    let csrf = me.json["csrfToken"].as_str().unwrap().to_owned();
    let apallati: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE andrew_id = 'apallati'")
            .fetch_one(&db)
            .await
            .unwrap();
    let res = Req::delete(&format!("/api/people/{apallati}/keycloak-link"))
        .header("cookie", &tk)
        .header("x-csrf-token", &csrf)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text());
    assert_eq!(
        dev_login_as(&app, "apallati", Some(PREVIEW_KEY))
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // On localhost a developer may still act as anyone seeded.
    let local = testing::app(db).await;
    assert_eq!(
        dev_login_as(&local, "apallati", None).await.status,
        StatusCode::NO_CONTENT
    );
}

// ---------------------------------------------------------------------------
// Limits

#[sqlx::test(migrations = "./migrations")]
async fn too_many_sign_in_starts_return_to_the_sign_in_page(db: PgPool) {
    let app = testing::app(db).await;
    for _ in 0..60 {
        let res = Req::get("/api/auth/login").send(&app).await;
        assert_eq!(
            res.header("location"),
            Some("/portal/sign-in/?error=not_configured")
        );
    }
    let limited = Req::get("/api/auth/login").send(&app).await;
    assert_eq!(limited.status, StatusCode::FOUND);
    assert_eq!(
        limited.header("location"),
        Some("/portal/sign-in/?error=rate_limited")
    );
    assert!(limited.header("retry-after").is_some());
    assert_eq!(limited.header("cache-control"), Some("no-store"));
}
