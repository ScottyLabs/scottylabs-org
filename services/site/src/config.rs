use std::env;
use std::path::PathBuf;

use anyhow::{Context, bail};
use url::Url;

/// Settings read once at startup from the environment kennel provides.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    /// The public origin, for example `https://scottylabs.org`.
    pub app_url: Url,
    pub bind_addr: String,
    /// The SvelteKit build. Unset in local development, where Vite serves pages.
    pub static_dir: Option<PathBuf>,
    pub oidc: Option<OidcSettings>,
    pub s3: Option<S3Settings>,
    pub dev_login: DevLogin,
    pub seed_demo: bool,
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

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = required("DATABASE_URL")?;
        let app_url =
            Url::parse(&optional("APP_URL").unwrap_or_else(|| "http://localhost:5173".into()))
                .context("APP_URL is not a valid URL")?;
        if !matches!(app_url.scheme(), "http" | "https") {
            bail!("APP_URL must be http or https");
        }

        let bind_addr = match (optional("PORT"), optional("SITE_BIND_ADDR")) {
            (Some(port), _) => format!("0.0.0.0:{port}"),
            (None, Some(addr)) => addr,
            (None, None) => "127.0.0.1:8080".into(),
        };

        let static_dir = optional("STATIC_DIR").map(PathBuf::from);

        let oidc = match (
            optional("OIDC_CLIENT_ID"),
            optional("OIDC_CLIENT_SECRET"),
            optional("KEYCLOAK_URL"),
            optional("KEYCLOAK_REALM"),
            optional("OAUTH_RELAY_URL"),
            optional("PROJECT_GROUP"),
            optional("PROJECT_ADMIN_GROUP"),
        ) {
            (
                Some(client_id),
                Some(client_secret),
                Some(keycloak_url),
                Some(keycloak_realm),
                Some(oauth_relay_url),
                Some(project_group),
                Some(project_admin_group),
            ) => Some(OidcSettings {
                keycloak_url: keycloak_url.trim_end_matches('/').to_owned(),
                keycloak_realm,
                client_id,
                client_secret,
                oauth_relay_url,
                project_group,
                project_admin_group,
            }),
            _ => None,
        };

        let s3 = match (
            optional("S3_ENDPOINT"),
            optional("S3_ACCESS_KEY"),
            optional("S3_SECRET_KEY"),
            optional("S3_BUCKET"),
        ) {
            (Some(endpoint), Some(access_key), Some(secret_key), Some(bucket)) => {
                Some(S3Settings {
                    endpoint: Url::parse(&endpoint).context("S3_ENDPOINT is not a valid URL")?,
                    region: optional("S3_REGION").unwrap_or_else(|| "garage".into()),
                    access_key,
                    secret_key,
                    bucket,
                })
            }
            _ => None,
        };

        let dev_login = dev_login_mode(
            &app_url,
            optional("PORTAL_DEV_LOGIN").as_deref() == Some("1"),
            optional("PORTAL_DEV_LOGIN_KEY"),
        );

        Ok(Self {
            database_url,
            app_url,
            bind_addr,
            static_dir,
            oidc,
            s3,
            dev_login,
            seed_demo: optional("SEED_DEMO_CONTENT").as_deref() == Some("1"),
        })
    }

    pub fn is_https(&self) -> bool {
        self.app_url.scheme() == "https"
    }

    /// `scheme://host[:port]` of `APP_URL`, for comparing with `Origin` headers.
    pub fn origin(&self) -> String {
        self.app_url.origin().ascii_serialization()
    }
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

fn required(name: &str) -> anyhow::Result<String> {
    optional(name).with_context(|| format!("{name} must be set"))
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
