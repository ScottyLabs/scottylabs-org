//! Keycloak sign-in through the Ricochet relay. Platform track.

use crate::config::OidcSettings;

/// Discovers Keycloak lazily so the site still starts when Keycloak is unreachable.
pub struct OidcProvider {
    pub settings: OidcSettings,
}

impl OidcProvider {
    pub fn new(settings: OidcSettings, _http: reqwest::Client) -> Self {
        Self { settings }
    }
}
