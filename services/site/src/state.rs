use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use sqlx::PgPool;

use crate::auth::oidc::OidcProvider;
use crate::config::Config;
use crate::media::MediaStore;
use crate::ratelimit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    /// Present only when every OIDC variable is set.
    pub oidc: Option<Arc<OidcProvider>>,
    pub media: Arc<MediaStore>,
    pub limits: Arc<RateLimiter>,
}

impl AppState {
    pub async fn new(db: PgPool, config: Arc<Config>) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("scottylabs-site")
            .build()
            .context("building the HTTP client")?;
        let oidc = config
            .oidc
            .clone()
            .map(|settings| Arc::new(OidcProvider::new(settings, http.clone())));
        let media = Arc::new(MediaStore::new(config.s3.clone(), db.clone(), http.clone()));
        Ok(Self {
            db,
            config,
            http,
            oidc,
            media,
            limits: Arc::new(RateLimiter::new()),
        })
    }
}
