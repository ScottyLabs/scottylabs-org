use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, bail};
use url::Url;

/// Settings read once at startup from the environment kennel provides.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    /// Connections in the request pool, from `DATABASE_POOL_SIZE`.
    pub db_pool_size: u32,
    /// The public origin, for example `https://scottylabs.org`.
    pub app_url: Url,
    /// Loopback unless `SITE_BIND_ADDR` names another address: kennel's Caddy and health probe reach the service on localhost.
    pub bind_addr: String,
    /// The SvelteKit build. Unset in local development, where Vite serves pages.
    pub static_dir: Option<PathBuf>,
    pub oidc: Option<OidcSettings>,
    pub s3: Option<S3Settings>,
    pub dev_login: DevLogin,
    pub seed_demo: bool,
    /// Sync directors and heads from governance at startup and every six hours.
    pub governance_sync: bool,
    /// The PostHog host's origin when public pages send page views, which needs both `PUBLIC_POSTHOG_KEY` and `PUBLIC_POSTHOG_HOST`. Only public pages may connect to it (`static_files::content_security_policy`).
    pub analytics_origin: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OidcSettings {
    pub keycloak_url: String,
    pub keycloak_realm: String,
    pub client_id: String,
    pub client_secret: String,
    pub oauth_relay_url: String,
    pub project_group: String,
    pub project_admin_group: String,
    /// `ANDREW_ID_CLAIM`: an ID token claim that holds the Andrew ID, for example one a Keycloak mapper fills from CMU's directory. When set it decides alone; otherwise `preferred_username` counts only when the token's email is that Andrew ID at cmu.edu or andrew.cmu.edu.
    pub andrew_id_claim: Option<String>,
}

#[derive(Clone, Debug)]
pub struct S3Settings {
    pub endpoint: Url,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
}

/// Whether people can sign in as a seeded persona without Keycloak.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevLogin {
    Off,
    /// Local development: no key needed.
    Open,
    /// Previews: the request must send this key.
    Keyed(String),
}

const PRODUCTION_HOSTS: [&str; 2] = ["scottylabs.org", "www.scottylabs.org"];

/// The seven variables Keycloak sign-in needs, all from the governance `oidc_client` feature.
const OIDC_VARIABLES: [&str; 7] = [
    "OIDC_CLIENT_ID",
    "OIDC_CLIENT_SECRET",
    "KEYCLOAK_URL",
    "KEYCLOAK_REALM",
    "OAUTH_RELAY_URL",
    "PROJECT_GROUP",
    "PROJECT_ADMIN_GROUP",
];

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_vars(optional)
    }

    /// Reads every setting through `var`, which returns a variable's value when it is set and not blank. Logs a warning for settings that are only partly there.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let required = |name: &str| var(name).with_context(|| format!("{name} must be set"));
        let database_url = required("DATABASE_URL")?;
        // Kennel sets it in every deploy and the dev shell sets it locally. Without it the service would quietly run as localhost: no Secure cookie, open dev sign-in, and every browser write refused by the Origin check.
        let app_url = Url::parse(&required("APP_URL")?).context("APP_URL is not a valid URL")?;
        if !matches!(app_url.scheme(), "http" | "https") {
            bail!("APP_URL must be http or https");
        }

        let bind_addr = bind_address(var("SITE_BIND_ADDR"), var("PORT"));
        let db_pool_size = pool_size(var("DATABASE_POOL_SIZE").as_deref())?;

        let static_dir = var("STATIC_DIR").map(PathBuf::from);
        let oidc = oidc_settings(&var).and_then(|settings| relay_guard(&app_url, settings));
        let s3 = s3_settings(&var)?;

        let dev_login = dev_login_mode(
            &app_url,
            var("PORTAL_DEV_LOGIN").as_deref() == Some("1"),
            var("PORTAL_DEV_LOGIN_KEY"),
        );

        let governance_sync =
            governance_sync_enabled(var("GOVERNANCE_SYNC").as_deref(), &dev_login)?;

        Ok(Self {
            database_url,
            db_pool_size,
            app_url,
            bind_addr,
            static_dir,
            oidc,
            s3,
            dev_login,
            seed_demo: var("SEED_DEMO_CONTENT").as_deref() == Some("1"),
            governance_sync,
            analytics_origin: analytics_origin(&var),
        })
    }

    /// How people sign in and where uploads go, for the startup log. Never includes a secret.
    pub fn describe(&self) -> (String, String) {
        let dev_login = match self.dev_login {
            DevLogin::Off => "off",
            DevLogin::Open => "open",
            DevLogin::Keyed(_) => "with a key",
        };
        let keycloak = if self.oidc.is_some() { "on" } else { "off" };
        let media = match &self.s3 {
            Some(s3) => format!("S3 bucket {} at {}", s3.bucket, s3.endpoint),
            None => "Postgres".into(),
        };
        (
            format!("Keycloak sign-in {keycloak}, dev sign-in {dev_login}"),
            media,
        )
    }

    pub fn is_https(&self) -> bool {
        self.app_url.scheme() == "https"
    }

    /// `scheme://host[:port]` of `APP_URL`, for comparing with `Origin` headers.
    pub fn origin(&self) -> String {
        self.app_url.origin().ascii_serialization()
    }

    /// Dev sign-in needs the switch and the seeded personas it signs in as.
    pub fn dev_login_available(&self) -> bool {
        self.dev_login != DevLogin::Off && self.seed_demo
    }

    /// Whether the service listens on a loopback address only, so every connection comes from this host. Rate limits trust a proxy's `CF-Connecting-IP` only then.
    pub fn binds_loopback(&self) -> bool {
        is_loopback_bind(&self.bind_addr)
    }
}

/// `<APP_URL>/api/auth/callback`, where sign-in comes back to.
pub fn sign_in_callback_url(app_url: &Url) -> String {
    let mut url = app_url.clone();
    url.set_path("/api/auth/callback");
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

/// Whether Keycloak may send this site's sign-in codes through the shared Ricochet relay: local development and `*.scottylabs.net` previews and staging. Every other host is production. The relay forwards a code to any host on its allowlist, which holds every `*.scottylabs.net` deployment, so a production code must come straight back to the site's own callback.
pub fn may_use_shared_relay(app_url: &Url) -> bool {
    match app_url.host() {
        Some(url::Host::Domain(host)) => host == "localhost" || host.ends_with(".scottylabs.net"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Keeps Keycloak sign-in off on a production host whose `OAUTH_RELAY_URL` is not its own callback, with one error line saying what governance must change.
fn relay_guard(app_url: &Url, settings: OidcSettings) -> Option<OidcSettings> {
    if may_use_shared_relay(app_url) {
        return Some(settings);
    }
    let callback = sign_in_callback_url(app_url);
    let direct =
        Url::parse(&settings.oauth_relay_url).is_ok_and(|relay| relay.as_str() == callback);
    if direct {
        return Some(settings);
    }
    tracing::error!(
        oauth_relay_url = %settings.oauth_relay_url,
        client_id = %settings.client_id,
        "Keycloak sign-in is off: on a production host Keycloak must redirect straight to this site's callback, not through the shared relay, which also forwards codes to every *.scottylabs.net deployment. In governance, give this prod client the redirect URI {callback} (oidc_client redirect_uris) and set its prod OAUTH_RELAY_URL to the same URL"
    );
    None
}

/// `127.0.0.1:8080`, `[::1]:8080` or `localhost:8080`.
fn is_loopback_bind(addr: &str) -> bool {
    match addr.parse::<SocketAddr>() {
        Ok(socket) => socket.ip().is_loopback(),
        Err(_) => addr
            .rsplit_once(':')
            .is_some_and(|(host, _)| host.eq_ignore_ascii_case("localhost")),
    }
}

/// Where to listen. `SITE_BIND_ADDR` is an explicit choice and wins. Kennel sets only `PORT`, and its Caddy (dialing `localhost`) and health probe (dialing `127.0.0.1`) are on the same host, so the port is bound on loopback only and the service is never reachable from outside whatever the firewall allows.
fn bind_address(explicit: Option<String>, port: Option<String>) -> String {
    match (explicit, port) {
        (Some(addr), _) => addr,
        (None, Some(port)) => format!("127.0.0.1:{port}"),
        (None, None) => "127.0.0.1:8080".into(),
    }
}

/// `DATABASE_POOL_SIZE`, from 1 to [`db::MAX_POOL_SIZE`](crate::db::MAX_POOL_SIZE); [`db::DEFAULT_POOL_SIZE`](crate::db::DEFAULT_POOL_SIZE) when unset.
fn pool_size(setting: Option<&str>) -> anyhow::Result<u32> {
    let Some(setting) = setting else {
        return Ok(crate::db::DEFAULT_POOL_SIZE);
    };
    match setting.trim().parse::<u32>() {
        Ok(size) if (1..=crate::db::MAX_POOL_SIZE).contains(&size) => Ok(size),
        _ => bail!(
            "DATABASE_POOL_SIZE must be a whole number from 1 to {}",
            crate::db::MAX_POOL_SIZE
        ),
    }
}

/// Keycloak sign-in, when all seven variables are set. Some but not all of them is a mistake worth a warning, since sign-in then stays off.
fn oidc_settings(var: &impl Fn(&str) -> Option<String>) -> Option<OidcSettings> {
    let values: Vec<Option<String>> = OIDC_VARIABLES.iter().map(|name| var(name)).collect();
    let missing: Vec<&str> = OIDC_VARIABLES
        .iter()
        .zip(&values)
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() {
        if missing.len() < OIDC_VARIABLES.len() {
            tracing::warn!(
                missing = %missing.join(", "),
                "Keycloak sign-in is off because some of its settings are missing"
            );
        }
        return None;
    }
    let mut values = values.into_iter().flatten();
    let mut next = || values.next().unwrap_or_default();
    Some(OidcSettings {
        client_id: next(),
        client_secret: next(),
        keycloak_url: next().trim_end_matches('/').to_owned(),
        keycloak_realm: next(),
        oauth_relay_url: next(),
        project_group: next(),
        project_admin_group: next(),
        andrew_id_claim: var("ANDREW_ID_CLAIM").map(|claim| claim.trim().to_owned()),
    })
}

/// The bucket for uploads, when configured. Kennel's garage provisioner names the credentials `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`; the dev shell names them `S3_ACCESS_KEY` and `S3_SECRET_KEY`. Kennel sets no region, and Garage answers to `garage`.
fn s3_settings(var: &impl Fn(&str) -> Option<String>) -> anyhow::Result<Option<S3Settings>> {
    let endpoint = var("S3_ENDPOINT");
    let bucket = var("S3_BUCKET");
    let access_key = var("S3_ACCESS_KEY").or_else(|| var("AWS_ACCESS_KEY_ID"));
    let secret_key = var("S3_SECRET_KEY").or_else(|| var("AWS_SECRET_ACCESS_KEY"));
    match (endpoint, access_key, secret_key, bucket) {
        (Some(endpoint), Some(access_key), Some(secret_key), Some(bucket)) => {
            Ok(Some(S3Settings {
                endpoint: Url::parse(&endpoint).context("S3_ENDPOINT is not a valid URL")?,
                region: var("S3_REGION").unwrap_or_else(|| "garage".into()),
                access_key,
                secret_key,
                bucket,
            }))
        }
        (endpoint, access_key, secret_key, bucket) => {
            // Credentials alone may belong to something else; a bucket or endpoint means S3 was meant.
            if endpoint.is_some() || bucket.is_some() {
                let missing: Vec<&str> = [
                    (endpoint.is_none(), "S3_ENDPOINT"),
                    (bucket.is_none(), "S3_BUCKET"),
                    (access_key.is_none(), "S3_ACCESS_KEY or AWS_ACCESS_KEY_ID"),
                    (
                        secret_key.is_none(),
                        "S3_SECRET_KEY or AWS_SECRET_ACCESS_KEY",
                    ),
                ]
                .into_iter()
                .filter_map(|(gone, name)| gone.then_some(name))
                .collect();
                tracing::warn!(
                    missing = %missing.join(", "),
                    "uploads go to Postgres because some S3 settings are missing"
                );
            }
            Ok(None)
        }
    }
}

/// Whether `host` is where production is served: `scottylabs.org` or `www.scottylabs.org`.
pub fn is_production_host(host: &str) -> bool {
    PRODUCTION_HOSTS.contains(&host)
}

/// Applies the dev login rules from docs/implementation/contract.md.
pub fn dev_login_mode(app_url: &Url, enabled: bool, key: Option<String>) -> DevLogin {
    let host = app_url.host_str().unwrap_or_default();
    if !enabled || PRODUCTION_HOSTS.contains(&host) {
        return DevLogin::Off;
    }
    if matches!(host, "localhost" | "127.0.0.1") {
        return DevLogin::Open;
    }
    match key {
        Some(key) if key.len() >= 16 => DevLogin::Keyed(key),
        _ => DevLogin::Off,
    }
}

/// `GOVERNANCE_SYNC=1` or `0` decides. Unset, the sync is on everywhere except local development with open dev sign-in, where it would fetch governance on every restart and add real people next to the seeded personas.
pub fn governance_sync_enabled(
    setting: Option<&str>,
    dev_login: &DevLogin,
) -> anyhow::Result<bool> {
    match setting.map(str::trim) {
        Some("1") => Ok(true),
        Some("0") => Ok(false),
        Some(_) => bail!("GOVERNANCE_SYNC must be 1 or 0"),
        None => Ok(*dev_login != DevLogin::Open),
    }
}

/// Where public pages send page views. The web app loads PostHog only when the key and the host are both set, so the origin counts only then too.
fn analytics_origin(var: &impl Fn(&str) -> Option<String>) -> Option<String> {
    var("PUBLIC_POSTHOG_KEY")?;
    let host = var("PUBLIC_POSTHOG_HOST")?;
    match Url::parse(host.trim()) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.host().is_some() => {
            Some(url.origin().ascii_serialization())
        }
        _ => {
            tracing::warn!(
                "PUBLIC_POSTHOG_HOST is not an http or https address, so the site's security policy blocks analytics"
            );
            None
        }
    }
}

fn optional(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    const BASE: [(&str, &str); 2] = [
        ("DATABASE_URL", "postgresql:///site"),
        ("APP_URL", "https://scottylabs.org"),
    ];

    #[test]
    fn app_url_is_required() {
        let error = Config::from_vars(vars(&[("DATABASE_URL", "postgresql:///site")]))
            .unwrap_err()
            .to_string();
        assert_eq!(error, "APP_URL must be set");
        let config = Config::from_vars(vars(&BASE)).unwrap();
        assert!(config.is_https());
        assert_eq!(config.dev_login, DevLogin::Off);
    }

    #[test]
    fn s3_takes_the_names_kennel_and_the_dev_shell_use() {
        let kennel = [
            BASE[0],
            BASE[1],
            ("S3_ENDPOINT", "http://garage.internal:3900"),
            ("S3_BUCKET", "scottylabs-org-site"),
            ("AWS_ACCESS_KEY_ID", "GKkennel"),
            ("AWS_SECRET_ACCESS_KEY", "kennel-secret"),
        ];
        let s3 = Config::from_vars(vars(&kennel)).unwrap().s3.unwrap();
        assert_eq!(s3.access_key, "GKkennel");
        assert_eq!(s3.secret_key, "kennel-secret");
        assert_eq!(s3.region, "garage");
        assert_eq!(s3.bucket, "scottylabs-org-site");

        let shell = [
            BASE[0],
            BASE[1],
            ("S3_ENDPOINT", "http://127.0.0.1:3900"),
            ("S3_BUCKET", "scottylabs-org"),
            ("S3_REGION", "garage"),
            ("S3_ACCESS_KEY", "GKshell"),
            ("S3_SECRET_KEY", "shell-secret"),
            ("AWS_ACCESS_KEY_ID", "GKother"),
        ];
        let s3 = Config::from_vars(vars(&shell)).unwrap().s3.unwrap();
        assert_eq!(s3.access_key, "GKshell");

        let partial = [BASE[0], BASE[1], ("S3_BUCKET", "scottylabs-org")];
        assert!(Config::from_vars(vars(&partial)).unwrap().s3.is_none());
    }

    #[test]
    fn analytics_needs_the_key_and_a_web_host() {
        let with = |pairs: &[(&'static str, &'static str)]| {
            let mut all: Vec<(&str, &str)> = BASE.to_vec();
            all.extend_from_slice(pairs);
            Config::from_vars(vars(&all)).unwrap().analytics_origin
        };
        assert_eq!(
            with(&[
                ("PUBLIC_POSTHOG_KEY", "phc_test"),
                ("PUBLIC_POSTHOG_HOST", " https://v.scottylabs.org/ingest/ "),
            ])
            .as_deref(),
            Some("https://v.scottylabs.org")
        );
        assert_eq!(with(&[]), None);
        assert_eq!(
            with(&[("PUBLIC_POSTHOG_HOST", "https://v.scottylabs.org")]),
            None
        );
        assert_eq!(with(&[("PUBLIC_POSTHOG_KEY", "phc_test")]), None);
        for bad in [
            "v.scottylabs.org",
            "javascript:alert(1)",
            "data:text/plain,x",
        ] {
            assert_eq!(
                with(&[
                    ("PUBLIC_POSTHOG_KEY", "phc_test"),
                    ("PUBLIC_POSTHOG_HOST", bad)
                ]),
                None,
                "{bad}"
            );
        }
    }

    #[test]
    fn keycloak_needs_all_seven_settings() {
        let mut all: Vec<(&str, &str)> = BASE.to_vec();
        all.extend(OIDC_VARIABLES.iter().map(|name| (*name, "x")));
        all.retain(|(name, _)| !matches!(*name, "KEYCLOAK_URL" | "OAUTH_RELAY_URL"));
        all.push(("KEYCLOAK_URL", "https://idp.cmu.edu/"));
        // Production: Keycloak redirects straight to the site.
        all.push((
            "OAUTH_RELAY_URL",
            "https://scottylabs.org/api/auth/callback",
        ));
        let oidc = Config::from_vars(vars(&all)).unwrap().oidc.unwrap();
        assert_eq!(oidc.keycloak_url, "https://idp.cmu.edu");
        assert_eq!(oidc.project_admin_group, "x");
        all.retain(|(name, _)| *name != "PROJECT_GROUP");
        assert!(Config::from_vars(vars(&all)).unwrap().oidc.is_none());
    }

    #[test]
    fn dev_login_is_never_on_in_production() {
        assert_eq!(
            dev_login_mode(&url("https://scottylabs.org"), true, Some("x".repeat(32))),
            DevLogin::Off
        );
        assert_eq!(
            dev_login_mode(&url("https://www.scottylabs.org"), true, None),
            DevLogin::Off
        );
    }

    #[test]
    fn dev_login_is_open_on_localhost() {
        assert_eq!(
            dev_login_mode(&url("http://localhost:5173"), true, None),
            DevLogin::Open
        );
        assert_eq!(
            dev_login_mode(&url("http://127.0.0.1:8080"), true, None),
            DevLogin::Open
        );
        assert_eq!(
            dev_login_mode(&url("http://localhost:5173"), false, None),
            DevLogin::Off
        );
    }

    #[test]
    fn governance_sync_is_off_by_default_only_in_local_development() {
        let keyed = DevLogin::Keyed("k".repeat(24));
        assert!(!governance_sync_enabled(None, &DevLogin::Open).unwrap());
        assert!(governance_sync_enabled(None, &DevLogin::Off).unwrap());
        assert!(governance_sync_enabled(None, &keyed).unwrap());
        assert!(governance_sync_enabled(Some("1"), &DevLogin::Open).unwrap());
        assert!(!governance_sync_enabled(Some("0"), &DevLogin::Off).unwrap());
        assert!(governance_sync_enabled(Some("yes"), &DevLogin::Off).is_err());
    }

    fn with_keycloak(app_url: &str, relay: &str) -> Vec<(&'static str, String)> {
        let mut all: Vec<(&'static str, String)> = vec![
            ("DATABASE_URL", "postgresql:///site".into()),
            ("APP_URL", app_url.into()),
        ];
        all.extend(OIDC_VARIABLES.iter().map(|name| (*name, "x".to_owned())));
        all.retain(|(name, _)| *name != "OAUTH_RELAY_URL");
        all.push(("OAUTH_RELAY_URL", relay.into()));
        all
    }

    fn keycloak_on(pairs: &[(&'static str, String)]) -> bool {
        let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Config::from_vars(vars(&pairs)).unwrap().oidc.is_some()
    }

    #[test]
    fn production_needs_a_direct_callback_instead_of_the_shared_relay() {
        const RELAY: &str = "https://oauth.scottylabs.org/oauth2/callback";
        // Production hosts: the shared relay turns Keycloak sign-in off.
        for app_url in [
            "https://scottylabs.org",
            "https://www.scottylabs.org",
            "https://events.example.org",
            "https://scottylabs.net",
            "https://scottylabs.net.evil.example",
        ] {
            assert!(!keycloak_on(&with_keycloak(app_url, RELAY)), "{app_url}");
        }
        // Its own callback works, with or without a trailing slash on APP_URL.
        assert!(keycloak_on(&with_keycloak(
            "https://scottylabs.org",
            "https://scottylabs.org/api/auth/callback"
        )));
        assert!(keycloak_on(&with_keycloak(
            "https://scottylabs.org/",
            "https://scottylabs.org/api/auth/callback"
        )));
        // Another site's callback, or a lookalike path, is not its own.
        for relay in [
            "https://www.scottylabs.org/api/auth/callback",
            "https://scottylabs.org/api/auth/callback/",
            "https://scottylabs.org/api/auth/callback?x=1",
            "http://scottylabs.org/api/auth/callback",
            "not a url",
        ] {
            assert!(
                !keycloak_on(&with_keycloak("https://scottylabs.org", relay)),
                "{relay}"
            );
        }
        // Previews, staging and local development may use the relay.
        for app_url in [
            "https://scottylabs-org-site-pr-3.scottylabs.net",
            "https://scottylabs-org-staging.scottylabs.net",
            "http://localhost:5173",
            "http://127.0.0.1:8080",
            "http://[::1]:8080",
        ] {
            assert!(keycloak_on(&with_keycloak(app_url, RELAY)), "{app_url}");
        }
    }

    #[test]
    fn the_callback_is_on_the_app_origin() {
        assert_eq!(
            sign_in_callback_url(&url("https://scottylabs.org/portal/?x=1#y")),
            "https://scottylabs.org/api/auth/callback"
        );
        assert_eq!(
            sign_in_callback_url(&url("http://localhost:5173")),
            "http://localhost:5173/api/auth/callback"
        );
    }

    #[test]
    fn the_andrew_id_claim_is_optional() {
        let pairs = with_keycloak(
            "https://scottylabs-org-site-pr-3.scottylabs.net",
            "https://oauth.scottylabs.org/oauth2/callback",
        );
        let plain: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let oidc = Config::from_vars(vars(&plain)).unwrap().oidc.unwrap();
        assert_eq!(oidc.andrew_id_claim, None);
        let mut with_claim = plain.clone();
        with_claim.push(("ANDREW_ID_CLAIM", " andrew_id "));
        let oidc = Config::from_vars(vars(&with_claim)).unwrap().oidc.unwrap();
        assert_eq!(oidc.andrew_id_claim.as_deref(), Some("andrew_id"));
    }

    #[test]
    fn loopback_binds_are_recognised() {
        let config = |bind: &str| {
            let mut pairs = BASE.to_vec();
            pairs.push(("SITE_BIND_ADDR", bind));
            Config::from_vars(vars(&pairs)).unwrap()
        };
        assert!(config("127.0.0.1:8080").binds_loopback());
        assert!(config("[::1]:8080").binds_loopback());
        assert!(config("localhost:8080").binds_loopback());
        assert!(!config("0.0.0.0:8080").binds_loopback());
        assert!(!config("[::]:8080").binds_loopback());
        assert!(!config("10.0.0.5:8080").binds_loopback());
        let mut pairs = BASE.to_vec();
        pairs.push(("PORT", "3000"));
        let kennel = Config::from_vars(vars(&pairs)).unwrap();
        assert_eq!(
            kennel.binds_loopback(),
            kennel.bind_addr.starts_with("127.")
        );
    }

    #[test]
    fn dev_login_needs_the_demo_personas() {
        let mut pairs = BASE.to_vec();
        pairs[1] = ("APP_URL", "http://localhost:5173");
        pairs.push(("PORTAL_DEV_LOGIN", "1"));
        assert!(
            !Config::from_vars(vars(&pairs))
                .unwrap()
                .dev_login_available()
        );
        pairs.push(("SEED_DEMO_CONTENT", "1"));
        assert!(
            Config::from_vars(vars(&pairs))
                .unwrap()
                .dev_login_available()
        );
    }

    #[test]
    fn binds_loopback_unless_told_otherwise() {
        let config = Config::from_vars(vars(&[BASE[0], BASE[1], ("PORT", "10234")])).unwrap();
        assert_eq!(config.bind_addr, "127.0.0.1:10234");
        let explicit = [
            BASE[0],
            BASE[1],
            ("PORT", "10234"),
            ("SITE_BIND_ADDR", "0.0.0.0:9000"),
        ];
        assert_eq!(
            Config::from_vars(vars(&explicit)).unwrap().bind_addr,
            "0.0.0.0:9000"
        );
        assert_eq!(
            Config::from_vars(vars(&BASE)).unwrap().bind_addr,
            "127.0.0.1:8080"
        );
    }

    #[test]
    fn the_pool_size_is_checked() {
        assert_eq!(
            Config::from_vars(vars(&BASE)).unwrap().db_pool_size,
            crate::db::DEFAULT_POOL_SIZE
        );
        let sized = [BASE[0], BASE[1], ("DATABASE_POOL_SIZE", " 20 ")];
        assert_eq!(Config::from_vars(vars(&sized)).unwrap().db_pool_size, 20);
        for bad in ["0", "51", "ten", "-1"] {
            let error = Config::from_vars(vars(&[BASE[0], BASE[1], ("DATABASE_POOL_SIZE", bad)]))
                .unwrap_err()
                .to_string();
            assert!(
                error.starts_with("DATABASE_POOL_SIZE must be"),
                "{bad}: {error}"
            );
        }
    }

    #[test]
    fn previews_need_a_long_key() {
        let preview = url("https://scottylabs-org-site-pr-3.scottylabs.net");
        assert_eq!(dev_login_mode(&preview, true, None), DevLogin::Off);
        assert_eq!(
            dev_login_mode(&preview, true, Some("short".into())),
            DevLogin::Off
        );
        let key = "k".repeat(24);
        assert_eq!(
            dev_login_mode(&preview, true, Some(key.clone())),
            DevLogin::Keyed(key)
        );
    }
}
