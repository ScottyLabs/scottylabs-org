//! The ScottyLabs site service: public pages, the content portal and its API on one origin.

mod approvals_core;
mod audit;
mod auth;
mod blocks;
mod config;
mod db;
mod error;
mod host_redirect;
mod json;
mod last_good;
mod media;
mod models;
mod permissions;
mod ratelimit;
mod retention;
mod richtext;
mod routes;
mod seed;
mod server;
mod state;
mod static_files;
#[cfg(test)]
mod testing;

use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

/// After `server::serve` returns, work on the blocking pool (image decodes, searches) gets this long before the process exits anyway. The graceful shutdown already took up to 8 seconds, and kennel waits 10 for a unit to stop.
const BLOCKING_WORK_GRACE: Duration = Duration::from_secs(1);

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")?;
    let result = runtime.block_on(run());
    stop(runtime, BLOCKING_WORK_GRACE);
    result
}

/// Stops the runtime, waiting at most `grace` for blocking tasks. Dropping it instead would wait for every running `spawn_blocking` task, however long it takes.
fn stop(runtime: tokio::runtime::Runtime, grace: Duration) {
    runtime.shutdown_timeout(grace);
}

async fn run() -> anyhow::Result<()> {
    // html5ever warns once per node it cannot place while ammonia sanitizes, which anyone can trigger thousands of times per request, so it logs only errors, whatever RUST_LOG says.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,site=debug"))
        .add_directive("html5ever=error".parse()?);
    // Colors only on a terminal; journald and log collectors would store the escape codes.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let config = Arc::new(Config::from_env().context("loading configuration")?);
    let (sign_in, media) = config.describe();
    tracing::info!(sign_in, media, "configuration loaded");

    // The site is served before the database is ready, and whether or not it ever is: the static pages need no database. Migrations and seeding run in the background until they succeed; until then the API answers 503.
    let options = db::connect_options(&config.database_url)?;
    let state = AppState::starting(
        db::request_pool(options.clone(), config.db_pool_size),
        config.clone(),
    )
    .await?;
    let listener = server::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, app_url = %config.app_url, "listening");

    tokio::spawn({
        let state = state.clone();
        async move {
            db::prepare_until_ready(&options, &state.config, &state.database).await;
            // Deletes expired sessions, old announcements, unused uploads and audit entries past retention, hourly.
            retention::spawn(state.clone());
            if state.config.governance_sync {
                // After seeding, so a slow or failing governance fetch never delays startup.
                routes::spawn_governance_sync(state);
            } else {
                tracing::info!("governance sync is off; set GOVERNANCE_SYNC=1 to turn it on");
            }
        }
    });

    server::serve(
        listener,
        routes::router(state),
        server::Limits::default(),
        shutdown_signal(),
    )
    .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    #[test]
    fn stopping_does_not_wait_for_long_blocking_work() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Like an image decode that is still running when the shutdown grace ends.
            tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_secs(5)));
            tokio::time::sleep(Duration::from_millis(50)).await;
        });
        let started = Instant::now();
        super::stop(runtime, Duration::from_millis(200));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
