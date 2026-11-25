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

/// Applies new migrations. Migrations a newer release applied are allowed, so a rollback to this release still starts: every migration must stay additive, so that the release before it runs against the new schema.
pub async fn migrate(db: &PgPool) -> anyhow::Result<()> {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(db).await.context("running migrations")
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

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
}
