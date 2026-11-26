use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use sqlx::PgPool;

use crate::auth::oidc::OidcProvider;
use crate::config::Config;
use crate::db::{Phase, Readiness};
use crate::last_good::LastGood;
use crate::media::MediaStore;
use crate::ratelimit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Whether `db` has been migrated and seeded. Until then the API answers 503.
    pub database: Arc<Readiness>,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    /// Present only when every OIDC variable is set.
    pub oidc: Option<Arc<OidcProvider>>,
    pub media: Arc<MediaStore>,
    pub limits: Arc<RateLimiter>,
    /// The last good answer of each public endpoint, served when the database fails.
    pub last_good: Arc<LastGood>,
}

impl AppState {
    /// State over a database that is already migrated and seeded, as in tests.
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
        let media = Arc::new(MediaStore::new(
            config.s3.clone(),
            db.clone(),
            http.clone(),
        )?);
        Ok(Self {
            db,
            database: Readiness::new(Phase::Ready),
            config,
            http,
            oidc,
            media,
            limits: Arc::new(RateLimiter::new()),
            last_good: Arc::new(LastGood::default()),
        })
    }

    /// State for a service whose database is prepared in the background: the API answers 503 until `database` is marked ready.
    pub async fn starting(db: PgPool, config: Arc<Config>) -> anyhow::Result<Self> {
        let state = Self::new(db, config).await?;
        state.database.set(Phase::Starting);
        Ok(state)
    }
}
