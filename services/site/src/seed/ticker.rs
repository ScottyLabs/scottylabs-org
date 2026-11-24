//! Seeds ticker. Ticker track.

use sqlx::PgConnection;

use crate::config::Config;

pub async fn run(_conn: &mut PgConnection, _config: &Config) -> anyhow::Result<()> {
    Ok(())
}
