//! The database: the pool requests use, the startup work (migrations and seeding), and whether that work is done.
//!
//! The service does not wait for Postgres to start. It serves the static site at once and prepares the database in the background, retrying with backoff, so a restart during a database outage never takes the public pages down (kennel holds a unit that fails to start five times in five minutes stopped until the next deploy). Until the database is ready, `/api` answers 503 `unavailable` and `/api/health` answers 503.

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use anyhow::Context;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::config::Config;

/// The most a request's statement may run. Public pages and the portal run short queries; anything slower is a stalled database, and its request answers 503 instead of holding a connection.
const STATEMENT_TIMEOUT: &str = "5s";
/// The most a request's statement waits for a lock held by someone else.
const LOCK_TIMEOUT: &str = "2s";
/// A transaction left open without a statement this long is ended by Postgres, which frees its locks and connection.
const IDLE_IN_TRANSACTION_TIMEOUT: &str = "30s";
/// How long a request waits for a free connection, or for a new one to open.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);
/// Connections in the request pool when `DATABASE_POOL_SIZE` is not set.
pub const DEFAULT_POOL_SIZE: u32 = 10;
/// The largest `DATABASE_POOL_SIZE` accepted. The host's Postgres is shared with other services.
pub const MAX_POOL_SIZE: u32 = 50;

/// How long opening the startup connection may take before the attempt counts as failed.
const STARTUP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The first retry of the startup work comes after this, then twice as long each time.
const FIRST_RETRY: Duration = Duration::from_secs(1);
/// The longest wait between startup attempts.
const LONGEST_RETRY: Duration = Duration::from_secs(60);

/// Connection options from `DATABASE_URL`.
pub fn connect_options(database_url: &str) -> anyhow::Result<PgConnectOptions> {
    PgConnectOptions::from_str(database_url).context("parsing DATABASE_URL")
}

/// The pool requests use. It opens connections when they are needed, so building it never fails because the database is down. Every connection limits how long a statement runs and waits for locks (see the constants above).
pub fn request_pool(options: PgConnectOptions, size: u32) -> PgPool {
    let options = options.options([
        ("statement_timeout", STATEMENT_TIMEOUT),
        ("lock_timeout", LOCK_TIMEOUT),
        (
            "idle_in_transaction_session_timeout",
            IDLE_IN_TRANSACTION_TIMEOUT,
        ),
    ]);
    PgPoolOptions::new()
        .max_connections(size.clamp(1, MAX_POOL_SIZE))
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect_lazy_with(options)
}

/// Applies new migrations. Migrations a newer release applied are allowed, so a rollback to this release still starts: every migration must stay additive, so that the release before it runs against the new schema.
pub async fn migrate(db: &PgPool) -> anyhow::Result<()> {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(db).await.context("running migrations")
}

/// Why preparing the database failed.
#[derive(Debug)]
pub enum StartupError {
    /// The database could not be reached, or it went away during the work. Retrying may help.
    Unreachable(anyhow::Error),
    /// The database answered, but a migration or the seed failed: this release and the schema disagree.
    Rejected(anyhow::Error),
}

/// Migrates and seeds on a connection of its own, which has none of the request pool's timeouts, so a long migration or a wait for the seed lock is not cut off.
pub async fn prepare(options: &PgConnectOptions, config: &Config) -> Result<(), StartupError> {
    let db = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(STARTUP_CONNECT_TIMEOUT)
        .connect_with(options.clone())
        .await
        .context("connecting to the database")
        .map_err(StartupError::Unreachable)?;
    let result = async {
        migrate(&db).await?;
        crate::seed::run(&db, config)
            .await
            .context("seeding content")?;
        crate::auth::session::end_disallowed_dev_sessions(&db, config)
            .await
            .context("ending dev sessions dev sign-in no longer allows")?;
        anyhow::Ok(())
    }
    .await;
    db.close().await;
    result.map_err(|error| {
        if is_unavailable_error(&error) {
            StartupError::Unreachable(error)
        } else {
            StartupError::Rejected(error)
        }
    })
}

/// Runs [`prepare`] until it succeeds, waiting longer after each failure, then marks the database ready. Never gives up: a database that comes back later, or a schema someone fixes, is picked up without a restart.
pub async fn prepare_until_ready(
    options: &PgConnectOptions,
    config: &Config,
    readiness: &Readiness,
) {
    let mut wait = FIRST_RETRY;
    loop {
        match prepare(options, config).await {
            Ok(()) => {
                readiness.set(Phase::Ready);
                tracing::info!("the database is ready");
                return;
            }
            Err(StartupError::Unreachable(error)) => {
                readiness.set(Phase::Unavailable);
                tracing::warn!(
                    error = %format!("{error:#}"),
                    retry_in_seconds = wait.as_secs(),
                    "the database is not reachable; serving pages, and answering the API with 503 until it is"
                );
            }
            Err(StartupError::Rejected(error)) => {
                readiness.set(Phase::Failed);
                tracing::error!(
                    error = %format!("{error:#}"),
                    retry_in_seconds = wait.as_secs(),
                    "migrating or seeding the database failed; answering the API with 503 until it succeeds"
                );
            }
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(LONGEST_RETRY);
    }
}

/// Where the startup work is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    /// The first attempt has not finished.
    Starting = 0,
    /// The last attempt could not reach the database.
    Unavailable = 1,
    /// The last attempt reached the database, but migrating or seeding failed.
    Failed = 2,
    /// Migrated and seeded. Requests may still fail if the database goes away later.
    Ready = 3,
}

/// Whether the database has been migrated and seeded, shared by the startup task and the request handlers.
#[derive(Debug)]
pub struct Readiness(AtomicU8);

impl Readiness {
    pub fn new(phase: Phase) -> Arc<Self> {
        Arc::new(Self(AtomicU8::new(phase as u8)))
    }

    pub fn phase(&self) -> Phase {
        match self.0.load(Ordering::Acquire) {
            0 => Phase::Starting,
            1 => Phase::Unavailable,
            2 => Phase::Failed,
            _ => Phase::Ready,
        }
    }

    pub fn set(&self, phase: Phase) {
        self.0.store(phase as u8, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.phase() == Phase::Ready
    }
}

/// Whether a database error means the database is down, unreachable, overloaded or too slow, rather than a fault in the request or the code. Such errors answer 503 and are retried.
pub fn is_unavailable(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::WorkerCrashed => true,
        sqlx::Error::Database(error) => error.code().is_some_and(|code| is_unavailable_code(&code)),
        _ => false,
    }
}

/// [`is_unavailable`] for any error with a database error somewhere in its chain of causes.
pub fn is_unavailable_error(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<sqlx::Error>())
        .any(is_unavailable)
}

/// SQLSTATE codes for a database that cannot serve right now: connection exceptions (08), insufficient resources such as too many connections (53), operator intervention, which includes a statement cancelled by `statement_timeout` and a server shutting down or starting up (57), system errors (58), and a lock not granted within `lock_timeout` (55P03).
fn is_unavailable_code(code: &str) -> bool {
    ["08", "53", "57", "58"]
        .iter()
        .any(|class| code.starts_with(class))
        || code == "55P03"
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use sqlx::PgPool;

    use super::*;
    use crate::error::AppError;

    /// A socket directory that does not exist: the incident's "No such file or directory".
    const NOWHERE: &str = "postgresql:///nowhere?host=/nonexistent/sl-site-test";

    #[sqlx::test(migrations = "./migrations")]
    async fn starts_on_a_database_a_newer_release_migrated(db: PgPool) {
        // A later release applied a migration this build does not know.
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (99_999, 'from a newer release', true, '\\x00', 0)",
        )
        .execute(&db)
        .await
        .unwrap();
        super::migrate(&db).await.unwrap();
    }

    #[test]
    fn outages_and_timeouts_are_unavailable_and_faults_are_not() {
        for code in [
            "08006", "53300", "57014", "57P01", "57P03", "58030", "55P03",
        ] {
            assert!(is_unavailable_code(code), "{code}");
        }
        for code in ["23505", "42P01", "40001", "22P02", "55000"] {
            assert!(!is_unavailable_code(code), "{code}");
        }
        assert!(is_unavailable(&sqlx::Error::PoolTimedOut));
        let missing_socket = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert!(is_unavailable(&sqlx::Error::Io(missing_socket)));
        assert!(!is_unavailable(&sqlx::Error::RowNotFound));
        let wrapped = anyhow::Error::from(sqlx::Error::PoolTimedOut).context("loading the ticker");
        assert!(is_unavailable_error(&wrapped));
        assert_eq!(AppError::from(wrapped).code(), "unavailable");
        assert_eq!(AppError::from(sqlx::Error::RowNotFound).code(), "internal");
    }

    #[test]
    fn readiness_moves_between_phases() {
        let readiness = Readiness::new(Phase::Starting);
        assert!(!readiness.is_ready());
        for phase in [Phase::Unavailable, Phase::Failed, Phase::Ready] {
            readiness.set(phase);
            assert_eq!(readiness.phase(), phase);
        }
        assert!(readiness.is_ready());
    }

    #[tokio::test]
    async fn the_request_pool_is_built_without_a_database() {
        // Building the pool must not try to connect; using it fails fast and answers 503.
        let pool = request_pool(connect_options(NOWHERE).unwrap(), DEFAULT_POOL_SIZE);
        let started = Instant::now();
        let error = sqlx::query("SELECT 1").execute(&pool).await.unwrap_err();
        assert!(is_unavailable(&error), "{error}");
        assert!(started.elapsed() < ACQUIRE_TIMEOUT + Duration::from_secs(1));
        let response = axum::response::IntoResponse::into_response(AppError::from(error));
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(response.headers()["retry-after"], "5");
    }

    #[tokio::test]
    async fn preparing_an_unreachable_database_can_be_retried() {
        let config = crate::testing::config();
        let error = prepare(&connect_options(NOWHERE).unwrap(), &config)
            .await
            .unwrap_err();
        assert!(matches!(error, StartupError::Unreachable(_)), "{error:?}");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn request_connections_stop_slow_statements_and_lock_waits(db: PgPool) {
        let pool = request_pool((*db.connect_options()).clone(), 2);
        let timeouts: (String, String, String) = sqlx::query_as(
            "SELECT current_setting('statement_timeout'), current_setting('lock_timeout'),
                    current_setting('idle_in_transaction_session_timeout')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(timeouts, ("5s".into(), "2s".into(), "30s".into()));

        let started = Instant::now();
        let error = sqlx::query("SELECT pg_sleep(30)")
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(AppError::from(error).code(), "unavailable");

        // Someone else holds a lock on the ticker: a request waits two seconds, then gives up.
        let mut holder = db.begin().await.unwrap();
        sqlx::query("LOCK TABLE announcements IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *holder)
            .await
            .unwrap();
        let started = Instant::now();
        let error = sqlx::query("SELECT count(*) FROM announcements")
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(4));
        assert_eq!(AppError::from(error).code(), "unavailable");
        holder.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = false)]
    async fn prepares_an_empty_database_without_the_request_timeouts(db: PgPool) {
        let readiness = Readiness::new(Phase::Starting);
        prepare_until_ready(&db.connect_options(), &crate::testing::config(), &readiness).await;
        assert!(readiness.is_ready());
        let committees: i64 = sqlx::query_scalar("SELECT count(*) FROM committees")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(committees, 7);
    }
}
