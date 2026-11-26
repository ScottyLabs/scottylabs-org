use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::http::StatusCode as AxumStatus;
use chrono::Utc;
use openidconnect::core::CoreGenderClaim;
use openidconnect::{
    Audience, EndUserEmail, EndUserUsername, IssuerUrl, StandardClaims, SubjectIdentifier,
};
use serde_json::json;

use super::*;

#[test]
fn state_round_trips_in_ricochet_format() {
    let encoded = encode_state("http://localhost:5173/api/auth/callback", "abc123");
    assert!(!encoded.contains('='));
    assert!(!encoded.contains('+') && !encoded.contains('/'));
    let json: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(&encoded).unwrap()).unwrap();
    assert_eq!(
        json,
        json!({
            "return_to": "http://localhost:5173/api/auth/callback",
            "csrf": "abc123"
        })
    );
    assert_eq!(
        decode_state(&encoded),
        Some(RelayState {
            return_to: "http://localhost:5173/api/auth/callback".into(),
            csrf: "abc123".into(),
        })
    );
}

#[test]
fn bad_states_are_rejected() {
    assert_eq!(decode_state(""), None);
    assert_eq!(decode_state("not base64 at all!"), None);
    assert_eq!(decode_state(&URL_SAFE_NO_PAD.encode(b"[1,2]")), None);
    assert_eq!(decode_state(&encode_state("x", "")), None);
    assert_eq!(decode_state(&"a".repeat(MAX_STATE_LEN + 1)), None);
}

#[test]
fn sign_in_errors_map_to_the_sign_in_page_codes() {
    let refused = |error: &str| SignInError::TokenRefused {
        error: error.into(),
        description: None,
    };
    assert_eq!(
        SignInError::provider("access_denied", None).code(),
        "cancelled"
    );
    assert_eq!(
        SignInError::provider("invalid_request", Some("Missing parameter")).code(),
        "failed"
    );
    assert_eq!(SignInError::Expired("no cookie").code(), "expired");
    assert_eq!(refused("invalid_grant").code(), "expired");
    assert_eq!(refused("unauthorized_client").code(), "failed");
    assert_eq!(SignInError::Failed(anyhow!("broken")).code(), "failed");
    assert_eq!(
        SignInError::Unavailable(anyhow!("down")).code(),
        "unavailable"
    );
    let not_andrew = SignInError::NotAndrewId {
        reason: "email_not_andrew_id",
        claimed: Some("tkanz".into()),
    };
    assert_eq!(not_andrew.code(), "no_andrew_id");
    assert_eq!(not_andrew.reason(), "email_not_andrew_id");
    let linked = SignInError::LinkedElsewhere {
        andrew_id: "tkanz".into(),
    };
    assert_eq!(linked.code(), "linked_elsewhere");
    assert_eq!(refused("invalid_grant").reason(), "code_refused");
    assert_eq!(
        SignInError::provider("server_error", None).reason(),
        "provider_error"
    );
}

#[test]
fn text_from_urls_is_cut_short_before_logging() {
    let long = "x".repeat(MAX_LOGGED_CHARS * 5);
    let SignInError::Provider { error, description } = SignInError::provider(&long, Some(&long))
    else {
        panic!("expected a provider error");
    };
    assert_eq!(error.chars().count(), MAX_LOGGED_CHARS);
    assert_eq!(
        description.map(|d| d.chars().count()),
        Some(MAX_LOGGED_CHARS)
    );
    assert_eq!(clip("caf\u{e9}"), "caf\u{e9}");
}

#[test]
fn missing_groups_stay_missing_when_serialized() {
    let none = serde_json::to_value(GroupClaims::default()).unwrap();
    assert_eq!(none, json!({}));
    let empty = serde_json::to_value(GroupClaims {
        groups: Some(Vec::new()),
        ..GroupClaims::default()
    })
    .unwrap();
    assert_eq!(empty, json!({ "groups": [] }));
}

#[test]
fn groups_claim_accepts_lists_and_single_values() {
    let many: GroupClaims =
        serde_json::from_str(r#"{"groups":["/projects/scottylabs-org","/x"]}"#).unwrap();
    assert_eq!(many.groups.unwrap().len(), 2);
    let one: GroupClaims = serde_json::from_str(r#"{"groups":"/x"}"#).unwrap();
    assert_eq!(one.groups, Some(vec!["/x".to_owned()]));
    let none: GroupClaims = serde_json::from_str("{}").unwrap();
    assert_eq!(none.groups, None);
    // Other claims are kept for ANDREW_ID_CLAIM, and groups is not among them.
    let other: GroupClaims =
        serde_json::from_str(r#"{"groups":["/x"],"andrew_id":"tkanz","n":1}"#).unwrap();
    assert_eq!(other.other.get("andrew_id"), Some(&json!("tkanz")));
    assert!(!other.other.contains_key("groups"));
}

// ---------------------------------------------------------------------------
// Which Andrew ID an account belongs to

fn claims(
    username: Option<&str>,
    email: Option<&str>,
    verified: Option<bool>,
    other: serde_json::Value,
) -> SiteClaims {
    let standard = StandardClaims::<CoreGenderClaim>::new(SubjectIdentifier::new("sub-1".into()))
        .set_preferred_username(username.map(|u| EndUserUsername::new(u.into())))
        .set_email(email.map(|e| EndUserEmail::new(e.into())))
        .set_email_verified(verified);
    let other: GroupClaims = serde_json::from_value(other).unwrap();
    IdTokenClaims::new(
        IssuerUrl::new("https://idp.example/realms/r".into()).unwrap(),
        vec![Audience::new("client".into())],
        Utc::now() + chrono::Duration::minutes(5),
        Utc::now(),
        standard,
        other,
    )
}

fn refusal(result: Result<String, SignInError>) -> &'static str {
    match result {
        Err(SignInError::NotAndrewId { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn usernames_count_as_andrew_ids_only_with_a_matching_cmu_email() {
    let ok = |username, email, verified| {
        andrew_id_of(
            &claims(Some(username), Some(email), verified, json!({})),
            None,
        )
    };
    assert_eq!(
        ok("tkanz", "tkanz@andrew.cmu.edu", Some(true)).unwrap(),
        "tkanz"
    );
    assert_eq!(ok("tkanz", "tkanz@cmu.edu", None).unwrap(), "tkanz");
    // Case does not matter, on either side.
    assert_eq!(
        ok("TKanz", "TKANZ@Andrew.CMU.edu", Some(true)).unwrap(),
        "tkanz"
    );

    for (username, email) in [
        ("tkanz", "someone@andrew.cmu.edu"),
        ("tkanz", "tkanz@gmail.com"),
        ("tkanz", "tkanz@andrew.cmu.edu.evil.example"),
        ("tkanz", "tkanz@scottylabs.org"),
        ("tkanz", "tkanz@alumni.cmu.edu"),
        ("tkanz", "xtkanz@andrew.cmu.edu"),
        ("tkanz", "tkanz+x@andrew.cmu.edu"),
    ] {
        assert_eq!(
            refusal(ok(username, email, Some(true))),
            "email_not_andrew_id",
            "{username} {email}"
        );
    }
    assert_eq!(
        refusal(ok("tkanz", "tkanz@andrew.cmu.edu", Some(false))),
        "email_not_verified"
    );
    assert_eq!(
        refusal(andrew_id_of(
            &claims(Some("tkanz"), None, Some(true), json!({})),
            None
        )),
        "no_email"
    );
    assert_eq!(
        refusal(andrew_id_of(
            &claims(None, Some("tkanz@andrew.cmu.edu"), Some(true), json!({})),
            None
        )),
        "username_not_an_andrew_id"
    );
    assert_eq!(
        refusal(andrew_id_of(
            &claims(
                Some("tkanz@andrew.cmu.edu"),
                Some("tkanz@andrew.cmu.edu"),
                Some(true),
                json!({})
            ),
            None
        )),
        "username_not_an_andrew_id"
    );
    // The refusal names the Andrew ID the account claimed, for the audit log.
    let Err(SignInError::NotAndrewId { claimed, .. }) = ok("tkanz", "x@cmu.edu", None) else {
        panic!("expected a refusal");
    };
    assert_eq!(claimed.as_deref(), Some("tkanz"));
}

#[test]
fn a_configured_andrew_id_claim_decides_alone() {
    let with = |username: Option<&str>, other: serde_json::Value| {
        andrew_id_of(
            &claims(username, Some("someone@gmail.com"), Some(false), other),
            Some("andrew_id"),
        )
    };
    // The claim wins over the username, and the email no longer matters.
    assert_eq!(
        with(Some("whatever"), json!({ "andrew_id": " TKanz " })).unwrap(),
        "tkanz"
    );
    assert_eq!(
        refusal(with(Some("tkanz"), json!({}))),
        "no_andrew_id_claim"
    );
    assert_eq!(
        refusal(with(Some("tkanz"), json!({ "andrew_id": 7 }))),
        "no_andrew_id_claim"
    );
    assert_eq!(
        refusal(with(Some("tkanz"), json!({ "andrew_id": "not an id" }))),
        "no_andrew_id_claim"
    );
}

#[test]
fn identity_reads_name_email_and_groups() {
    let identity = identity(
        &claims(
            Some("tkanz"),
            Some("tkanz@andrew.cmu.edu"),
            Some(true),
            json!({ "groups": ["/a"] }),
        ),
        None,
    )
    .unwrap();
    assert_eq!(identity.subject, "sub-1");
    assert_eq!(identity.andrew_id, "tkanz");
    assert_eq!(identity.email.as_deref(), Some("tkanz@andrew.cmu.edu"));
    assert_eq!(identity.groups, Some(vec!["/a".to_owned()]));
}

// ---------------------------------------------------------------------------
// Discovery while Keycloak is down

fn settings(keycloak_url: &str) -> OidcSettings {
    OidcSettings {
        keycloak_url: keycloak_url.into(),
        keycloak_realm: "scottylabs".into(),
        client_id: "scottylabs-org-dev".into(),
        client_secret: "secret".into(),
        oauth_relay_url: "http://localhost:8090/oauth2/callback".into(),
        project_group: "/projects/scottylabs-org".into(),
        project_admin_group: "/projects/scottylabs-org/admins".into(),
        andrew_id_claim: None,
    }
}

fn provider(keycloak_url: &str, timing: Timing) -> OidcProvider {
    OidcProvider::with_timing(settings(keycloak_url), reqwest::Client::new(), timing)
}

const CALLBACK: &str = "http://localhost:5173/api/auth/callback";

#[tokio::test]
async fn authorize_reports_unreachable_keycloak() {
    let provider = OidcProvider::new(settings("http://127.0.0.1:9"), reqwest::Client::new());
    assert_eq!(provider.issuer(), "http://127.0.0.1:9/realms/scottylabs");
    let error = provider.authorize(CALLBACK).await.err().unwrap();
    assert_eq!(error.code(), "unavailable");
}

/// Accepts connections and never answers, like a Keycloak that hangs. Counts the connections.
async fn hanging_keycloak() -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&accepted);
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            held.push(socket);
        }
    });
    (base, accepted)
}

/// Answers every request with 500, like idp.scottylabs.org during the outage. Counts the requests.
async fn broken_keycloak() -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let app = Router::new().fallback(move || {
        let counter = Arc::clone(&counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            AxumStatus::INTERNAL_SERVER_ERROR
        }
    });
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, hits)
}

#[tokio::test]
async fn sign_ins_share_one_discovery_while_keycloak_hangs() {
    let (base, accepted) = hanging_keycloak().await;
    let provider = provider(
        &base,
        Timing {
            request_timeout: Duration::from_millis(400),
            retry_after_failure: Duration::from_secs(60),
        },
    );
    let started = Instant::now();
    let attempts = (0..5).map(|_| provider.authorize(CALLBACK));
    let results = futures::future::join_all(attempts).await;
    let elapsed = started.elapsed();
    assert!(
        results
            .iter()
            .all(|result| result.as_ref().err().map(SignInError::code) == Some("unavailable"))
    );
    // One discovery for all five, not one after another (that would take five times as long).
    assert!(elapsed < Duration::from_millis(1500), "{elapsed:?}");
    assert_eq!(accepted.load(Ordering::SeqCst), 1);

    // The failure is remembered: the next sign-in fails at once without asking again.
    let started = Instant::now();
    let error = provider.authorize(CALLBACK).await.err().unwrap();
    assert_eq!(error.code(), "unavailable");
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_failed_discovery_is_tried_again_after_a_pause() {
    let (base, hits) = broken_keycloak().await;
    let provider = provider(
        &base,
        Timing {
            request_timeout: Duration::from_secs(2),
            retry_after_failure: Duration::from_millis(200),
        },
    );
    assert_eq!(
        provider.authorize(CALLBACK).await.err().unwrap().code(),
        "unavailable"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(provider.authorize(CALLBACK).await.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(provider.authorize(CALLBACK).await.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    // Refreshes and sign-out see the same outage.
    assert!(matches!(
        provider.refresh("token").await,
        Err(RefreshError::Unavailable(_))
    ));
    assert_eq!(provider.end_session_url("id-token").await, None);
}

#[test]
fn checks_of_one_session_do_not_overlap() {
    let provider = Arc::new(provider("http://127.0.0.1:9", Timing::default()));
    let session = Uuid::now_v7();
    let first = provider.begin_check(session);
    assert!(first.is_some());
    assert!(provider.begin_check(session).is_none());
    assert!(provider.begin_check(Uuid::now_v7()).is_some());
    drop(first);
    assert!(provider.begin_check(session).is_some());
}
