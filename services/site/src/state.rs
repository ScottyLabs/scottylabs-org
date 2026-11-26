use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use sqlx::PgPool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::auth::oidc::OidcProvider;
use crate::config::Config;
use crate::db::{Phase, Readiness};
use crate::last_good::LastGood;
use crate::media::MediaStore;
use crate::ratelimit::RateLimiter;

/// Searches of published projects that run at once. Each reads every published project's text, so a burst of them waits here instead of taking every worker.
const CONCURRENT_SEARCHES: usize = 4;
/// How long a visitor's request waits for one of the public slots before it answers 503 (and the public endpoints their last good copy).
pub const PUBLIC_SLOT_WAIT: Duration = Duration::from_secs(2);

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
    pub search_slots: Arc<Semaphore>,
    /// Database work for anonymous visitors (`/api/public/*` and the project page shells) runs in at most half the pool at once, so a burst of them while the database is slow, such as behind a held lock, cannot take every connection from the health check and the portal.
    pub public_slots: Arc<Semaphore>,
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
        let public_slots = usize::try_from(db.options().get_max_connections() / 2)
            .unwrap_or(1)
            .max(1);
        Ok(Self {
            db,
            database: Readiness::new(Phase::Ready),
            config,
            http,
            oidc,
            media,
            limits: Arc::new(RateLimiter::new()),
            last_good: Arc::new(LastGood::default()),
            search_slots: Arc::new(Semaphore::new(CONCURRENT_SEARCHES)),
            public_slots: Arc::new(Semaphore::new(public_slots)),
        })
    }

    /// One of the [`AppState::public_slots`], waiting at most [`PUBLIC_SLOT_WAIT`]. `None` when they stayed busy: answer 503.
    pub async fn public_slot(&self) -> Option<OwnedSemaphorePermit> {
        tokio::time::timeout(
            PUBLIC_SLOT_WAIT,
            Arc::clone(&self.public_slots).acquire_owned(),
        )
        .await
        .ok()?
        .ok()
    }

    /// State for a service whose database is prepared in the background: the API answers 503 until `database` is marked ready.
    pub async fn starting(db: PgPool, config: Arc<Config>) -> anyhow::Result<Self> {
        let state = Self::new(db, config).await?;
        state.database.set(Phase::Starting);
        Ok(state)
    }
}
