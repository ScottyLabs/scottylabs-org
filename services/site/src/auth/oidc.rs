//! Keycloak sign-in through the Ricochet relay: authorization code flow with PKCE (S256), a nonce, and a fully verified ID token.
//!
//! Keycloak redirects to Ricochet (`OAUTH_RELAY_URL`), which decodes `state` and forwards the browser to its `return_to` with every provider parameter unchanged (`code`, `state`, `iss`, `session_state`, `error`, `error_description`). So `state` carries our callback URL plus a random `csrf` that keys the stored attempt.

use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreErrorResponseType, CoreGenderClaim, CoreJsonWebKey,
    CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm, CoreProviderMetadata,
    CoreResponseType, CoreRevocableToken, CoreRevocationErrorResponse,
    CoreTokenIntrospectionResponse, CoreTokenType,
};
use openidconnect::{
    AdditionalClaims, AuthenticationFlow, AuthorizationCode, ClaimsVerificationError, Client,
    ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IdTokenClaims, IdTokenFields, IdTokenVerifier, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RequestTokenError, Scope,
    SignatureVerificationError, StandardErrorResponse, StandardTokenResponse, TokenResponse,
};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::Mutex;

use super::normalize_andrew_id;
use super::session::random_token;
use crate::config::OidcSettings;

/// How long discovery (endpoints and signing keys) is reused before it is fetched again.
const DISCOVERY_TTL: Duration = Duration::from_secs(60 * 60);
/// An unknown signing key triggers a refresh, but no more often than this.
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
/// `state` values longer than this are not ours.
const MAX_STATE_LEN: usize = 4096;
/// Text from a callback URL or an error response is cut to this many characters before it is logged.
const MAX_LOGGED_CHARS: usize = 200;
/// Keycloak signs ID tokens with RS256. Accepting only that refuses tokens signed with the client secret (HS256), which discovery also lists.
const ID_TOKEN_ALGORITHM: CoreJwsSigningAlgorithm = CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256;

/// The `groups` claim governance adds to tokens, in full-path form such as `/projects/scottylabs-org`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GroupClaims {
    #[serde(
        default,
        deserialize_with = "one_or_many",
        skip_serializing_if = "Option::is_none"
    )]
    pub groups: Option<Vec<String>>,
}

impl AdditionalClaims for GroupClaims {}

/// Accepts a list of strings, a single string, or null.
fn one_or_many<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Groups {
        Many(Vec<String>),
        One(String),
    }
    Ok(match Option::<Groups>::deserialize(deserializer)? {
        None => None,
        Some(Groups::Many(list)) => Some(list),
        Some(Groups::One(group)) => Some(vec![group]),
    })
}

type SiteIdTokenFields = IdTokenFields<
    GroupClaims,
    EmptyExtraTokenFields,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
>;

type SiteTokenResponse = StandardTokenResponse<SiteIdTokenFields, CoreTokenType>;

type SiteClient = Client<
    GroupClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    SiteTokenResponse,
    CoreTokenIntrospectionResponse,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// What Ricochet reads from `state`. Field names are Ricochet's.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayState {
    pub return_to: String,
    pub csrf: String,
}

/// base64url (no padding) of the relay state JSON.
pub fn encode_state(return_to: &str, csrf: &str) -> String {
    let state = RelayState {
        return_to: return_to.to_owned(),
        csrf: csrf.to_owned(),
    };
    // Serializing two strings cannot fail.
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&state).unwrap_or_default())
}

pub fn decode_state(raw: &str) -> Option<RelayState> {
    if raw.is_empty() || raw.len() > MAX_STATE_LEN {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(raw.trim_end_matches('=')).ok()?;
    let state: RelayState = serde_json::from_slice(&bytes).ok()?;
    (!state.csrf.is_empty()).then_some(state)
}

/// A sign-in that was just started: where to send the browser, and what to store until it returns.
pub struct Authorization {
    pub url: url::Url,
    pub csrf: String,
    pub pkce_verifier: String,
    pub nonce: String,
}

/// The person Keycloak vouched for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub andrew_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    /// `None` when the token has no `groups` claim at all.
    pub groups: Option<Vec<String>>,
}

/// Why finishing a sign-in failed. [`SignInError::code`] is the `error` the sign-in page understands; [`SignInError::log`] says why in the service log, never with codes or secrets.
#[derive(Debug)]
pub enum SignInError {
    /// Keycloak sent the browser back with `error` instead of a code. `access_denied` means the person backed out.
    Provider {
        error: String,
        description: Option<String>,
    },
    /// The attempt was unknown, used or too old, or the browser came back without the sign-in cookie. People going back or waiting cause this, so it is logged at info.
    Expired(&'static str),
    /// Keycloak refused to exchange the code. `invalid_grant` usually means a used or slow code, but a wrong redirect URI or PKCE verifier says the same, so it is logged as a warning.
    TokenRefused {
        error: String,
        description: Option<String>,
    },
    Failed(anyhow::Error),
}

impl SignInError {
    /// From the callback's `error` and `error_description`, which anyone can put in a URL, so both are cut short.
    pub fn provider(error: &str, description: Option<&str>) -> Self {
        Self::Provider {
            error: clip(error),
            description: description.map(clip),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Provider { error, .. } if error == "access_denied" => "cancelled",
            Self::TokenRefused { error, .. } if error == "invalid_grant" => "expired",
            Self::Expired(_) => "expired",
            Self::Provider { .. } | Self::TokenRefused { .. } | Self::Failed(_) => "failed",
        }
    }

    pub fn log(&self) {
        match self {
            Self::Provider { error, description } => tracing::warn!(
                error = ?error,
                description = ?description,
                "Keycloak sent the sign-in back with an error"
            ),
            Self::Expired(reason) => tracing::info!(reason, "sign-in expired"),
            Self::TokenRefused { error, description } => tracing::warn!(
                error = ?error,
                description = ?description,
                "Keycloak refused the authorization code"
            ),
            Self::Failed(cause) => tracing::warn!(error = ?cause, "Keycloak sign-in failed"),
        }
    }
}

impl From<anyhow::Error> for SignInError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

/// Cuts outside text short before it goes into the log.
pub fn clip(text: &str) -> String {
    text.chars().take(MAX_LOGGED_CHARS).collect()
}

struct Discovery {
    metadata: CoreProviderMetadata,
    fetched_at: Instant,
}

/// Discovers Keycloak lazily so the site still starts when Keycloak is unreachable.
pub struct OidcProvider {
    pub settings: OidcSettings,
    http: reqwest::Client,
    discovery: Mutex<Option<Discovery>>,
}

impl OidcProvider {
    /// `http` must not follow redirects, so a compromised endpoint cannot bounce requests elsewhere.
    pub fn new(settings: OidcSettings, http: reqwest::Client) -> Self {
        // The groups are logged so a deployment shows which Keycloak groups it trusts. Only the admin group grants anything; project membership alone does not open the portal.
        tracing::info!(
            keycloak = %settings.keycloak_url,
            realm = %settings.keycloak_realm,
            client_id = %settings.client_id,
            admin_group = %settings.project_admin_group,
            project_group = %settings.project_group,
            "sign-in through Keycloak is configured"
        );
        Self {
            settings,
            http,
            discovery: Mutex::new(None),
        }
    }

    pub fn issuer(&self) -> String {
        format!(
            "{}/realms/{}",
            self.settings.keycloak_url, self.settings.keycloak_realm
        )
    }

    /// Provider metadata and signing keys, from the cache when fresh. `refresh` asks for new keys,
    /// which is honoured at most every 30 seconds. One discovery runs at a time; if it fails, the last good metadata is used.
    async fn metadata(&self, refresh: bool) -> anyhow::Result<CoreProviderMetadata> {
        let mut discovery = self.discovery.lock().await;
        if let Some(cached) = discovery.as_ref() {
            let age = cached.fetched_at.elapsed();
            let wants_new = age >= DISCOVERY_TTL || (refresh && age >= MIN_REFRESH_INTERVAL);
            if !wants_new {
                return Ok(cached.metadata.clone());
            }
        }
        let issuer = IssuerUrl::new(self.issuer()).context("KEYCLOAK_URL is not a valid URL")?;
        match CoreProviderMetadata::discover_async(issuer, &self.http).await {
            Ok(metadata) => {
                *discovery = Some(Discovery {
                    metadata: metadata.clone(),
                    fetched_at: Instant::now(),
                });
                Ok(metadata)
            }
            Err(error) => match discovery.as_ref() {
                Some(stale) => {
                    tracing::warn!(error = %error, "Keycloak discovery failed; using the previous result");
                    Ok(stale.metadata.clone())
                }
                None => Err(anyhow!("Keycloak discovery failed: {error}")),
            },
        }
    }

    fn client(&self, metadata: CoreProviderMetadata) -> anyhow::Result<SiteClient> {
        let redirect = RedirectUrl::new(self.settings.oauth_relay_url.clone())
            .context("OAUTH_RELAY_URL is not a valid URL")?;
        Ok(SiteClient::from_provider_metadata(
            metadata,
            ClientId::new(self.settings.client_id.clone()),
            Some(ClientSecret::new(self.settings.client_secret.clone())),
        )
        .set_redirect_uri(redirect))
    }

    /// Starts a sign-in. `callback_url` is `<APP_URL>/api/auth/callback`, where Ricochet sends the browser back.
    pub async fn authorize(&self, callback_url: &str) -> anyhow::Result<Authorization> {
        let client = self.client(self.metadata(false).await?)?;
        let csrf = random_token();
        let state = encode_state(callback_url, &csrf);
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        // Only `openid` (added by the library), `email` and `profile`. The realm also offers `microprofile-jwt`, whose mapper writes realm roles into a claim also named `groups`, which would replace the group paths that PROJECT_ADMIN_GROUP is matched against.
        let (url, _, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                move || CsrfToken::new(state),
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".into()))
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        Ok(Authorization {
            url,
            csrf,
            pkce_verifier: verifier.secret().clone(),
            nonce: nonce.secret().clone(),
        })
    }

    /// Exchanges the code and verifies the ID token: signature, issuer, audience, expiry and nonce.
    pub async fn finish(
        &self,
        code: String,
        pkce_verifier: String,
        nonce: String,
    ) -> Result<Identity, SignInError> {
        let client = self.client(self.metadata(false).await?)?;
        let response = client
            .exchange_code(AuthorizationCode::new(code))
            .context("Keycloak has no token endpoint")?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&self.http)
            .await
            .map_err(|error| match error {
                RequestTokenError::ServerResponse(body) => SignInError::TokenRefused {
                    error: clip(body.error().as_ref()),
                    description: body.error_description().map(|text| clip(text)),
                },
                other => SignInError::Failed(anyhow!("token exchange failed: {other}")),
            })?;
        let id_token = response
            .id_token()
            .ok_or_else(|| anyhow!("Keycloak returned no ID token"))?;
        let nonce = Nonce::new(nonce);

        let claims = match id_token.claims(&id_token_verifier(&client), &nonce) {
            Ok(claims) => claims.clone(),
            Err(ClaimsVerificationError::SignatureVerification(
                SignatureVerificationError::NoMatchingKey,
            )) => {
                // Keycloak rotated its keys since discovery. Fetch them again and retry once.
                let client = self.client(self.metadata(true).await?)?;
                id_token
                    .claims(&id_token_verifier(&client), &nonce)
                    .map_err(|error| anyhow!("ID token rejected after refreshing keys: {error}"))?
                    .clone()
            }
            Err(error) => return Err(anyhow!("ID token rejected: {error}").into()),
        };
        identity(&claims).map_err(SignInError::Failed)
    }
}

/// Checks signature, issuer, audience and expiry, accepting only [`ID_TOKEN_ALGORITHM`].
fn id_token_verifier(client: &SiteClient) -> IdTokenVerifier<'_, CoreJsonWebKey> {
    client
        .id_token_verifier()
        .set_allowed_algs([ID_TOKEN_ALGORITHM])
}

fn identity(claims: &IdTokenClaims<GroupClaims, CoreGenderClaim>) -> anyhow::Result<Identity> {
    let username = claims
        .preferred_username()
        .ok_or_else(|| anyhow!("the ID token has no preferred_username"))?;
    let andrew_id = normalize_andrew_id(username.as_str()).ok_or_else(|| {
        anyhow!(
            "preferred_username {:?} is not an Andrew ID",
            username.as_str()
        )
    })?;
    let name = claims
        .name()
        .and_then(|names| names.get(None))
        .map(|name| name.as_str().trim().to_owned())
        .filter(|name| !name.is_empty());
    Ok(Identity {
        subject: claims.subject().as_str().to_owned(),
        andrew_id,
        name,
        email: claims.email().map(|email| email.as_str().to_owned()),
        groups: claims.additional_claims().groups.clone(),
    })
}

#[cfg(test)]
mod tests {
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
            serde_json::json!({
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
    }

    #[test]
    fn text_from_urls_is_cut_short_before_logging() {
        let long = "x".repeat(MAX_LOGGED_CHARS * 5);
        let SignInError::Provider { error, description } =
            SignInError::provider(&long, Some(&long))
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
        let none = serde_json::to_value(GroupClaims { groups: None }).unwrap();
        assert_eq!(none, serde_json::json!({}));
        let empty = serde_json::to_value(GroupClaims {
            groups: Some(Vec::new()),
        })
        .unwrap();
        assert_eq!(empty, serde_json::json!({ "groups": [] }));
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
    }

    #[tokio::test]
    async fn authorize_reports_unreachable_keycloak() {
        let settings = OidcSettings {
            keycloak_url: "http://127.0.0.1:9".into(),
            keycloak_realm: "scottylabs".into(),
            client_id: "scottylabs-org-dev".into(),
            client_secret: "secret".into(),
            oauth_relay_url: "http://localhost:8090/oauth2/callback".into(),
            project_group: "/projects/scottylabs-org".into(),
            project_admin_group: "/projects/scottylabs-org/admins".into(),
        };
        let provider = OidcProvider::new(settings, reqwest::Client::new());
        assert_eq!(provider.issuer(), "http://127.0.0.1:9/realms/scottylabs");
        assert!(
            provider
                .authorize("http://localhost:5173/api/auth/callback")
                .await
                .is_err()
        );
    }
}
