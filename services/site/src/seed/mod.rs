//! Content seeded at startup. Only inserts what is missing, so it never overwrites edits.
//! See "Seed data" in docs/implementation/contract.md.

mod committees;
mod people;
mod projects;
mod ticker;

use anyhow::Context;
use sqlx::PgPool;

use crate::config::Config;
use crate::models::Committee;

/// Color and tint for each committee, from the Team page and the design file.
pub const COMMITTEE_COLORS: [(Committee, &str, &str); 7] = [
    (Committee::Events, "#B1B3E9", "#DEDFF6"),
    (Committee::Outreach, "#B1CAE9", "#DEE9F6"),
    (Committee::Tech, "#EC9798", "#F7D3D4"),
    (Committee::Labrador, "#E8BBAE", "#F5E2DD"),
    (Committee::Design, "#CDB3D2", "#EADFEC"),
    (Committee::Finance, "#B3D5E0", "#DFEDF2"),
    (Committee::Foundry, "#D3C7AE", "#EDE7DD"),
];

pub async fn run(db: &PgPool, config: &Config) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    // Serialize seeding across instances starting at once.
    sqlx::query("SELECT pg_advisory_xact_lock(724_501)")
        .execute(&mut *tx)
        .await?;

    for (order, (committee, color, tint)) in COMMITTEE_COLORS.iter().enumerate() {
        sqlx::query(
            "INSERT INTO committees (slug, name, sort_order, color, tint) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (slug) DO UPDATE
             SET name = EXCLUDED.name, sort_order = EXCLUDED.sort_order,
                 color = EXCLUDED.color, tint = EXCLUDED.tint",
        )
        .bind(committee)
        .bind(committee.name())
        .bind(i32::try_from(order).unwrap_or(0))
        .bind(*color)
        .bind(*tint)
        .execute(&mut *tx)
        .await
        .context("seeding committees")?;
    }

    people::run(&mut tx, config)
        .await
        .context("seeding people")?;
    committees::run(&mut tx, config)
        .await
        .context("seeding committee pages")?;
    projects::run(&mut tx, config)
        .await
        .context("seeding projects")?;
    ticker::run(&mut tx, config)
        .await
        .context("seeding the ticker")?;

    tx.commit().await?;
    Ok(())
}
