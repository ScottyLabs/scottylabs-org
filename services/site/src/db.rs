use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let options = PgConnectOptions::from_str(database_url).context("parsing DATABASE_URL")?;
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .context("connecting to the database")
}

pub async fn migrate(db: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations")
        .run(db)
        .await
        .context("running migrations")
}
