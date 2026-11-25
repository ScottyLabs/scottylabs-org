//! The ScottyLabs site service: public pages, the content portal and its API on one origin.

mod approvals_core;
mod audit;
mod auth;
mod blocks;
mod config;
mod db;
mod error;
mod json;
mod media;
mod models;
mod permissions;
mod ratelimit;
mod richtext;
mod routes;
mod seed;
mod state;
mod static_files;
#[cfg(test)]
mod testing;

use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // html5ever warns once per node it cannot place while ammonia sanitizes, which anyone can trigger thousands of times per request, so it logs only errors, whatever RUST_LOG says.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,site=debug"))
        .add_directive("html5ever=error".parse()?);
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = Arc::new(Config::from_env().context("loading configuration")?);
    let db = db::connect(&config.database_url).await?;
    db::migrate(&db).await?;
    seed::run(&db, &config).await.context("seeding content")?;

    let state = AppState::new(db, config.clone()).await?;
    if config.governance_sync {
        // In the background, after seeding, so a slow or failing governance fetch never delays startup.
        routes::spawn_governance_sync(state.clone());
    } else {
        tracing::info!("governance sync is off; set GOVERNANCE_SYNC=1 to turn it on");
    }
    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("binding {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, app_url = %config.app_url, "listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("serving")?;
    Ok(())
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
