//! Keycloak sign-in: authorization code flow with PKCE (S256), a nonce, and a fully verified ID token. Also the checks that follow a sign-in: refresh-token grants while the session lasts, and the URL that ends the Keycloak session at sign-out.
//!
//! On previews and staging Keycloak redirects to Ricochet (`OAUTH_RELAY_URL`), which decodes `state` and forwards the browser to its `return_to` with every provider parameter unchanged (`code`, `state`, `iss`, `session_state`, `error`, `error_description`). In production `OAUTH_RELAY_URL` is the site's own callback (see `config::may_use_shared_relay`), so Keycloak sends the browser straight back with the same parameters. Either way `state` carries our callback URL plus a random `csrf` that keys the stored attempt.
//!
//! Every call to Keycloak has a short time limit, discovery runs once however many requests wait for it, and a failed discovery is remembered for a moment, so sign-in fails fast and says Keycloak is unavailable while it is down.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreErrorResponseType, CoreGenderClaim, CoreJsonWebKey,
    CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm, CoreResponseType,
    CoreRevocableToken, CoreRevocationErrorResponse, CoreTokenIntrospectionResponse, CoreTokenType,
};
use openidconnect::http::StatusCode;
use openidconnect::{
    AdditionalClaims, AsyncHttpClient, AuthenticationFlow, AuthorizationCode,
    ClaimsVerificationError, Client, ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields,
    EndpointMaybeSet, EndpointNotSet, EndpointSet, HttpClientError, HttpRequest, HttpResponse,
    IdToken, IdTokenClaims, IdTokenFields, IdTokenVerifier, IssuerUrl, Nonce, NonceVerifier,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, ProviderMetadataWithLogout,
    RedirectUrl, RefreshToken, RequestTokenError, Scope, SignatureVerificationError,
    StandardErrorResponse, StandardTokenResponse, TokenResponse,
};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

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
/// The domains of CMU email addresses whose local part is the Andrew ID.
const CMU_EMAIL_DOMAINS: [&str; 2] = ["andrew.cmu.edu", "cmu.edu"];

/// How long Keycloak gets to answer, and how long a failed discovery is remembered.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// One discovery (the configuration and the signing keys together), or one token request.
    pub request_timeout: Duration,
    /// After a failed discovery, callers get the previous result or fail at once for this long.
    pub retry_after_failure: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(5),
            retry_after_failure: Duration::from_secs(30),
        }
    }
}

/// Claims governance adds to tokens: `groups` in full-path form such as `/projects/scottylabs-org`, and anything else that is not a standard claim, so `ANDREW_ID_CLAIM` can name any of them.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GroupClaims {
    #[serde(
        default,
        deserialize_with = "one_or_many",
        skip_serializing_if = "Option::is_none"
    )]
    pub groups: Option<Vec<String>>,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
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

type SiteIdToken = IdToken<
    GroupClaims,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
>;

type SiteClaims = IdTokenClaims<GroupClaims, CoreGenderClaim>;

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

type TokenError = RequestTokenError<
    HttpClientError<reqwest::Error>,
    StandardErrorResponse<CoreErrorResponseType>,
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

/// A finished sign-in: who it is, and Keycloak's tokens for checking the session later.
pub struct SignedIn {
    pub identity: Identity,
    pub refresh_token: Option<String>,
    pub id_token: String,
}

/// What a refresh-token grant says about a session that is still good.
#[derive(Debug)]
pub struct Refreshed {
    pub subject: String,
    /// `None` when the new ID token has no `groups` claim at all.
    pub groups: Option<Vec<String>>,
    /// Keycloak may hand out a new refresh token; the old one may then stop working.
    pub refresh_token: Option<String>,
    pub id_token: String,
}

/// Why a refresh-token grant did not confirm a session.
#[derive(Debug)]
pub enum RefreshError {
    /// Keycloak refused the refresh token (`invalid_grant`): the account is disabled, the person signed out of Keycloak, or Keycloak's own session ended. The portal session ends too.
    Rejected { description: Option<String> },
    /// Keycloak did not answer in time, or answered with a server error. The session is kept.
    Unavailable(anyhow::Error),
    /// Anything else, such as a refused client or a token that does not verify. The session is kept but not confirmed.
    Failed(anyhow::Error),
}

/// Why finishing a sign-in failed. [`SignInError::code`] is the `error` the sign-in page understands; [`SignInError::reason`] is the finer reason for the audit log; [`SignInError::log`] says why in the service log, never with codes or secrets.
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
    /// Keycloak did not answer in time, or answered with a server error.
    Unavailable(anyhow::Error),
    /// The account could not be tied to a CMU Andrew ID, so nothing was linked or created. `reason` is a short code; `claimed` is the Andrew ID the account named, when it named one.
    NotAndrewId {
        reason: &'static str,
        claimed: Option<String>,
    },
    /// The Andrew ID belongs to a person already linked to another Keycloak account. An admin can reset the link (docs/implementation/sign-in-recovery.md).
    LinkedElsewhere {
        andrew_id: String,
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
            Self::Unavailable(_) => "unavailable",
            Self::NotAndrewId { .. } => "no_andrew_id",
            Self::LinkedElsewhere { .. } => "linked_elsewhere",
            Self::Provider { .. } | Self::TokenRefused { .. } | Self::Failed(_) => "failed",
        }
    }

    /// A short code for the audit log, finer than [`Self::code`].
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Provider { error, .. } if error == "access_denied" => "cancelled",
            Self::Provider { .. } => "provider_error",
            Self::Expired(_) => "expired",
            Self::TokenRefused { .. } => "code_refused",
            Self::Unavailable(_) => "keycloak_unavailable",
            Self::NotAndrewId { reason, .. } => reason,
            Self::LinkedElsewhere { .. } => "linked_elsewhere",
            Self::Failed(_) => "failed",
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
            Self::Unavailable(cause) => {
                tracing::warn!(error = %format!("{cause:#}"), "Keycloak is unavailable");
            }
            Self::NotAndrewId { reason, claimed } => tracing::warn!(
                reason,
                claimed = ?claimed,
                "refused a Keycloak sign-in that could not be matched to a CMU Andrew ID"
            ),
            Self::LinkedElsewhere { andrew_id } => tracing::warn!(
                andrew_id = %andrew_id,
                "refused a Keycloak sign-in: this Andrew ID is linked to a different Keycloak account"
            ),
            Self::Failed(cause) => {
                tracing::warn!(error = %format!("{cause:#}"), "Keycloak sign-in failed");
            }
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

type DiscoveryResult = Result<Arc<ProviderMetadataWithLogout>, String>;
type RunningDiscovery = Shared<BoxFuture<'static, DiscoveryResult>>;

struct Discovery {
    metadata: Arc<ProviderMetadataWithLogout>,
    fetched_at: Instant,
}

#[derive(Default)]
struct DiscoveryState {
    cached: Option<Discovery>,
    /// When the last discovery failed.
    failed_at: Option<Instant>,
    /// The discovery in progress. Everyone who needs one waits for this instead of starting another, and no lock is held while it runs.
    running: Option<RunningDiscovery>,
}

/// Discovers Keycloak lazily so the site still starts when Keycloak is unreachable.
pub struct OidcProvider {
    pub settings: OidcSettings,
    http: KeycloakHttp,
    timing: Timing,
    discovery: Arc<Mutex<DiscoveryState>>,
    /// Sessions this process is checking with Keycloak right now, so each check runs once.
    checking: Mutex<HashSet<Uuid>>,
}

/// Marks a session as being checked until dropped.
pub struct CheckGuard {
    provider: Arc<OidcProvider>,
    session_id: Uuid,
}

impl Drop for CheckGuard {
    fn drop(&mut self) {
        lock(&self.provider.checking).remove(&self.session_id);
    }
}

impl OidcProvider {
    /// `http` must not follow redirects, so a compromised endpoint cannot bounce requests elsewhere.
    pub fn new(settings: OidcSettings, http: reqwest::Client) -> Self {
        Self::with_timing(settings, http, Timing::default())
    }

    pub fn with_timing(settings: OidcSettings, http: reqwest::Client, timing: Timing) -> Self {
        // The groups are logged so a deployment shows which Keycloak groups it trusts. Only the admin group grants anything; project membership alone does not open the portal.
        tracing::info!(
            keycloak = %settings.keycloak_url,
            realm = %settings.keycloak_realm,
            client_id = %settings.client_id,
            admin_group = %settings.project_admin_group,
            project_group = %settings.project_group,
            andrew_id_from = settings
                .andrew_id_claim
                .as_deref()
                .unwrap_or("preferred_username with a matching CMU email"),
            "sign-in through Keycloak is configured"
        );
        Self {
            settings,
            http: KeycloakHttp(http),
            timing,
            discovery: Arc::new(Mutex::new(DiscoveryState::default())),
            checking: Mutex::new(HashSet::new()),
        }
    }

    pub fn issuer(&self) -> String {
        format!(
            "{}/realms/{}",
            self.settings.keycloak_url, self.settings.keycloak_realm
        )
    }

    /// Provider metadata and signing keys, from the cache when fresh. `refresh` asks for new keys, which is honoured at most every 30 seconds. One discovery runs at a time and nobody holds a lock while it runs. If it fails, the last good metadata is used; without any, callers fail at once until [`Timing::retry_after_failure`] has passed. An error means Keycloak is unavailable.
    async fn metadata(&self, refresh: bool) -> anyhow::Result<ProviderMetadataWithLogout> {
        let running = {
            let mut state = lock(&self.discovery);
            let now = Instant::now();
            if let Some(cached) = &state.cached {
                let age = now.saturating_duration_since(cached.fetched_at);
                let wants_new = age >= DISCOVERY_TTL || (refresh && age >= MIN_REFRESH_INTERVAL);
                if !wants_new {
                    return Ok((*cached.metadata).clone());
                }
            }
            let failed_recently = state.failed_at.is_some_and(|at| {
                now.saturating_duration_since(at) < self.timing.retry_after_failure
            });
            match (&state.running, failed_recently) {
                (Some(running), _) => running.clone(),
                (None, true) => {
                    return match &state.cached {
                        Some(stale) => Ok((*stale.metadata).clone()),
                        None => Err(anyhow!(
                            "Keycloak could not be reached a moment ago, so it is not asked again yet"
                        )),
                    };
                }
                (None, false) => {
                    let running = self.start_discovery();
                    state.running = Some(running.clone());
                    running
                }
            }
        };
        match running.await {
            Ok(metadata) => Ok((*metadata).clone()),
            Err(error) => match &lock(&self.discovery).cached {
                Some(stale) => Ok((*stale.metadata).clone()),
                None => Err(anyhow!("Keycloak discovery failed: {error}")),
            },
        }
    }

    /// Starts a discovery on its own task, so it finishes even if every request waiting for it gives up. The first waiter to see the result records it.
    fn start_discovery(&self) -> RunningDiscovery {
        let task = tokio::spawn(discover(
            self.issuer(),
            self.http.clone(),
            self.timing.request_timeout,
        ));
        let state = Arc::clone(&self.discovery);
        async move {
            let result = task
                .await
                .unwrap_or_else(|error| Err(format!("the discovery task stopped: {error}")));
            let mut state = lock(&state);
            state.running = None;
            match &result {
                Ok(metadata) => {
                    state.cached = Some(Discovery {
                        metadata: Arc::clone(metadata),
                        fetched_at: Instant::now(),
                    });
                    state.failed_at = None;
                }
                Err(error) => {
                    state.failed_at = Some(Instant::now());
                    tracing::warn!(
                        error = %error,
                        using_previous = state.cached.is_some(),
                        "Keycloak discovery failed"
                    );
                }
            }
            result
        }
        .boxed()
        .shared()
    }

    fn client(&self, metadata: ProviderMetadataWithLogout) -> anyhow::Result<SiteClient> {
        let redirect = RedirectUrl::new(self.settings.oauth_relay_url.clone())
            .context("OAUTH_RELAY_URL is not a valid URL")?;
        Ok(SiteClient::from_provider_metadata(
            metadata,
            ClientId::new(self.settings.client_id.clone()),
            Some(ClientSecret::new(self.settings.client_secret.clone())),
        )
        .set_redirect_uri(redirect))
    }

    /// Starts a sign-in. `callback_url` is `<APP_URL>/api/auth/callback`, where the browser comes back to.
    pub async fn authorize(&self, callback_url: &str) -> Result<Authorization, SignInError> {
        let metadata = self
            .metadata(false)
            .await
            .map_err(SignInError::Unavailable)?;
        let client = self.client(metadata)?;
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

    /// Exchanges the code, verifies the ID token (signature, issuer, audience, expiry and nonce) and works out the Andrew ID.
    pub async fn finish(
        &self,
        code: String,
        pkce_verifier: String,
        nonce: String,
    ) -> Result<SignedIn, SignInError> {
        let metadata = self
            .metadata(false)
            .await
            .map_err(SignInError::Unavailable)?;
        let client = self.client(metadata)?;
        let exchange = client
            .exchange_code(AuthorizationCode::new(code))
            .context("Keycloak has no token endpoint")?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&self.http);
        let response = match tokio::time::timeout(self.timing.request_timeout, exchange).await {
            Err(_) => {
                return Err(SignInError::Unavailable(anyhow!(
                    "Keycloak did not answer the code exchange in time"
                )));
            }
            Ok(result) => result.map_err(exchange_error)?,
        };
        let id_token = response
            .id_token()
            .ok_or_else(|| anyhow!("Keycloak returned no ID token"))?;
        let nonce = Nonce::new(nonce);
        let claims = self.verified_claims(&client, id_token, &nonce).await?;
        log_email_verified(&claims);
        let identity = identity(&claims, self.settings.andrew_id_claim.as_deref())?;
        Ok(SignedIn {
            identity,
            refresh_token: response.refresh_token().map(|token| token.secret().clone()),
            id_token: id_token.to_string(),
        })
    }

    /// Asks Keycloak whether a session is still good, with a refresh-token grant, and reads the person's groups from the new ID token.
    pub async fn refresh(&self, refresh_token: &str) -> Result<Refreshed, RefreshError> {
        let metadata = self
            .metadata(false)
            .await
            .map_err(RefreshError::Unavailable)?;
        let client = self.client(metadata).map_err(RefreshError::Failed)?;
        let token = RefreshToken::new(refresh_token.to_owned());
        let grant = client
            .exchange_refresh_token(&token)
            .context("Keycloak has no token endpoint")
            .map_err(RefreshError::Failed)?
            .request_async(&self.http);
        let response = match tokio::time::timeout(self.timing.request_timeout, grant).await {
            Err(_) => {
                return Err(RefreshError::Unavailable(anyhow!(
                    "Keycloak did not answer the refresh in time"
                )));
            }
            Ok(Ok(response)) => response,
            Ok(Err(RequestTokenError::ServerResponse(body)))
                if body.error().as_ref() == "invalid_grant" =>
            {
                return Err(RefreshError::Rejected {
                    description: body.error_description().map(|text| clip(text)),
                });
            }
            Ok(Err(RequestTokenError::ServerResponse(body))) => {
                return Err(RefreshError::Failed(anyhow!(
                    "Keycloak refused the refresh with {}",
                    clip(body.error().as_ref())
                )));
            }
            Ok(Err(RequestTokenError::Request(cause))) => {
                return Err(RefreshError::Unavailable(
                    anyhow::Error::new(cause).context("the refresh request failed"),
                ));
            }
            Ok(Err(other)) => {
                return Err(RefreshError::Failed(anyhow!("the refresh failed: {other}")));
            }
        };
        let id_token = response
            .id_token()
            .ok_or_else(|| RefreshError::Failed(anyhow!("Keycloak returned no ID token")))?;
        // A refreshed ID token carries the first sign-in's nonce or none at all. It came straight from Keycloak's token endpoint, in answer to the client secret and the refresh token.
        let any_nonce = |_: Option<&Nonce>| Ok::<(), String>(());
        let claims = self
            .verified_claims(&client, id_token, any_nonce)
            .await
            .map_err(|error| match error {
                SignInError::Unavailable(cause) => RefreshError::Unavailable(cause),
                SignInError::Failed(cause) => RefreshError::Failed(cause),
                other => RefreshError::Failed(anyhow!("{}", other.reason())),
            })?;
        Ok(Refreshed {
            subject: claims.subject().as_str().to_owned(),
            groups: claims.additional_claims().groups.clone(),
            refresh_token: response.refresh_token().map(|token| token.secret().clone()),
            id_token: id_token.to_string(),
        })
    }

    /// Where to send the browser to end its Keycloak session too, when Keycloak's discovery lists an end-session endpoint on its own origin. Carries `id_token_hint` and `client_id`, and no return address, since the client has none registered for it.
    pub async fn end_session_url(&self, id_token: &str) -> Option<String> {
        let metadata = self.metadata(false).await.ok()?;
        let endpoint = metadata
            .additional_metadata()
            .end_session_endpoint
            .as_ref()?
            .url()
            .clone();
        let issuer = url::Url::parse(&self.issuer()).ok()?;
        if endpoint.origin() != issuer.origin() {
            tracing::warn!(
                endpoint = %endpoint,
                "Keycloak's end-session endpoint is on another origin, so sign-out does not use it"
            );
            return None;
        }
        let mut url = endpoint;
        url.query_pairs_mut()
            .append_pair("id_token_hint", id_token)
            .append_pair("client_id", &self.settings.client_id);
        Some(url.to_string())
    }

    /// Marks a session as being checked by this process. `None` when a check of it is already running here.
    pub fn begin_check(self: &Arc<Self>, session_id: Uuid) -> Option<CheckGuard> {
        lock(&self.checking).insert(session_id).then(|| CheckGuard {
            provider: Arc::clone(self),
            session_id,
        })
    }

    /// Verifies an ID token with the cached keys, and once more with fresh keys when Keycloak rotated them since discovery.
    async fn verified_claims<N: NonceVerifier + Copy>(
        &self,
        client: &SiteClient,
        id_token: &SiteIdToken,
        nonce: N,
    ) -> Result<SiteClaims, SignInError> {
        match id_token.claims(&id_token_verifier(client), nonce) {
            Ok(claims) => Ok(claims.clone()),
            Err(ClaimsVerificationError::SignatureVerification(
                SignatureVerificationError::NoMatchingKey,
            )) => {
                let metadata = self
                    .metadata(true)
                    .await
                    .map_err(SignInError::Unavailable)?;
                let client = self.client(metadata)?;
                let claims = id_token
                    .claims(&id_token_verifier(&client), nonce)
                    .map_err(|error| anyhow!("ID token rejected after refreshing keys: {error}"))?;
                Ok(claims.clone())
            }
            Err(error) => Err(anyhow!("ID token rejected: {error}").into()),
        }
    }
}

/// Fetches Keycloak's configuration and signing keys within `timeout`.
async fn discover(issuer: String, http: KeycloakHttp, timeout: Duration) -> DiscoveryResult {
    let issuer = IssuerUrl::new(issuer)
        .map_err(|error| format!("KEYCLOAK_URL is not a valid URL: {error}"))?;
    match tokio::time::timeout(
        timeout,
        ProviderMetadataWithLogout::discover_async(issuer, &http),
    )
    .await
    {
        Ok(Ok(metadata)) => Ok(Arc::new(metadata)),
        Ok(Err(error)) => Err(format!("{:#}", anyhow::Error::new(error))),
        Err(_) => Err(format!(
            "Keycloak did not answer within {} seconds",
            timeout.as_secs_f32()
        )),
    }
}

/// The HTTP client for Keycloak: reqwest, except that a server error or 429 counts as Keycloak being unavailable rather than as an answer, so an outage is told apart from a refusal.
#[derive(Clone)]
struct KeycloakHttp(reqwest::Client);

impl<'c> AsyncHttpClient<'c> for KeycloakHttp {
    type Error = HttpClientError<reqwest::Error>;
    type Future =
        Pin<Box<dyn Future<Output = Result<HttpResponse, Self::Error>> + Send + Sync + 'c>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let response = self.0.call(request).await?;
            let status = response.status();
            if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                return Err(HttpClientError::Other(format!(
                    "Keycloak answered {status}"
                )));
            }
            Ok(response)
        })
    }
}

fn exchange_error(error: TokenError) -> SignInError {
    match error {
        RequestTokenError::ServerResponse(body) => SignInError::TokenRefused {
            error: clip(body.error().as_ref()),
            description: body.error_description().map(|text| clip(text)),
        },
        RequestTokenError::Request(cause) => {
            SignInError::Unavailable(anyhow::Error::new(cause).context("the code exchange failed"))
        }
        other => SignInError::Failed(anyhow!("token exchange failed: {other}")),
    }
}

/// Checks signature, issuer, audience and expiry, accepting only [`ID_TOKEN_ALGORITHM`].
fn id_token_verifier(client: &SiteClient) -> IdTokenVerifier<'_, CoreJsonWebKey> {
    client
        .id_token_verifier()
        .set_allowed_algs([ID_TOKEN_ALGORITHM])
}

/// One line per sign-in says whether tokens carry `email_verified`, which the Andrew ID check relies on. No personal data.
fn log_email_verified(claims: &SiteClaims) {
    let email_verified = match claims.email_verified() {
        Some(true) => "true",
        Some(false) => "false",
        None => "missing",
    };
    tracing::info!(
        email_verified,
        "Keycloak sign-in: the ID token's email_verified claim"
    );
}

fn identity(claims: &SiteClaims, andrew_id_claim: Option<&str>) -> Result<Identity, SignInError> {
    let andrew_id = andrew_id_of(claims, andrew_id_claim)?;
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

/// The account's Andrew ID, only with proof that it is one. With `ANDREW_ID_CLAIM` set, that claim decides alone. Otherwise `preferred_username` counts only when the token's email is exactly that Andrew ID at andrew.cmu.edu or cmu.edu (ignoring case) and `email_verified` is not false: a username alone can come from an identity provider where people choose their own.
fn andrew_id_of(claims: &SiteClaims, andrew_id_claim: Option<&str>) -> Result<String, SignInError> {
    if let Some(claim) = andrew_id_claim {
        return claims
            .additional_claims()
            .other
            .get(claim)
            .and_then(serde_json::Value::as_str)
            .and_then(normalize_andrew_id)
            .ok_or(SignInError::NotAndrewId {
                reason: "no_andrew_id_claim",
                claimed: None,
            });
    }
    let Some(andrew_id) = claims
        .preferred_username()
        .and_then(|username| normalize_andrew_id(username.as_str()))
    else {
        return Err(SignInError::NotAndrewId {
            reason: "username_not_an_andrew_id",
            claimed: None,
        });
    };
    let refuse = |reason: &'static str| SignInError::NotAndrewId {
        reason,
        claimed: Some(andrew_id.clone()),
    };
    if claims.email_verified() == Some(false) {
        return Err(refuse("email_not_verified"));
    }
    let Some(email) = claims.email() else {
        return Err(refuse("no_email"));
    };
    let email = email.as_str().trim().to_ascii_lowercase();
    let matches = CMU_EMAIL_DOMAINS
        .iter()
        .any(|domain| email == format!("{andrew_id}@{domain}"));
    if !matches {
        return Err(refuse("email_not_andrew_id"));
    }
    Ok(andrew_id)
}

#[cfg(test)]
mod tests;
