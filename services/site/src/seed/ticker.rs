//! Seeds the ticker with the announcements from the design file, only while the table is empty, so edits and deletions stick across restarts. CollegeCart is always seeded, since it replaces the announcement that used to be hard-coded in `Hero.svelte`; the rest only with demo content.

use anyhow::Context;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::approvals_core::{self, SubjectType};
use crate::config::Config;
use crate::models::{Committee, TickerSource};

struct Sample {
    /// An `AnnouncementKind` slug.
    kind: &'static str,
    committee: TickerSource,
    message: &'static str,
    link_url: Option<&'static str>,
    days_left: i64,
    /// A persona credited as the poster, when demo people exist.
    posted_by: Option<&'static str>,
}

const COLLEGECART: Sample = Sample {
    kind: "project_update",
    committee: TickerSource::Labrador,
    message: "Check out CollegeCart, new from the Labrador committee.",
    link_url: Some("https://collegecart.org/"),
    days_left: 30,
    posted_by: None,
};

/// The other live samples from the Figma ticker, with expiries spread over the next 2 to 18 days.
const DEMO: [Sample; 4] = [
    Sample {
        kind: "club_news",
        committee: TickerSource::Club,
        message: "Work sessions run every Saturday, 4 to 6 PM in Tepper.",
        link_url: None,
        days_left: 18,
        posted_by: Some("tkanz"),
    },
    Sample {
        kind: "milestone",
        committee: TickerSource::Events,
        message: "TartanHacks passed 1,000 participants this year.",
        link_url: Some("https://tartanhacks.com/"),
        days_left: 3,
        posted_by: Some("evanfost"),
    },
    Sample {
        kind: "project_update",
        committee: TickerSource::Tech,
        message: "CMU Maps now has room-level directions.",
        link_url: Some("https://cmumaps.com/"),
        days_left: 9,
        posted_by: Some("anishp"),
    },
    Sample {
        kind: "event",
        committee: TickerSource::Foundry,
        message: "Foundry fireside chat this Thursday, 6 PM in Tepper 2612.",
        link_url: None,
        days_left: 2,
        posted_by: None,
    },
];

/// A lead's submission waiting for the Director of Events, so approvals have something to show.
const PENDING: Sample = Sample {
    kind: "event",
    committee: TickerSource::Events,
    message: "Volunteer sign-ups for TartanHacks are open.",
    link_url: Some("https://tartanhacks.com/"),
    days_left: 10,
    posted_by: Some("spchoksh"),
};

pub async fn run(conn: &mut PgConnection, config: &Config) -> anyhow::Result<()> {
    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM announcements")
        .fetch_one(&mut *conn)
        .await?;
    if existing > 0 {
        return Ok(());
    }
    let now = Utc::now();
    insert_live(conn, &COLLEGECART, 0, now).await?;
    if !config.seed_demo {
        return Ok(());
    }
    for (position, sample) in (1..).zip(&DEMO) {
        insert_live(conn, sample, position, now).await?;
    }
    insert_pending(conn, &PENDING, now).await
}

async fn persona_id(
    conn: &mut PgConnection,
    andrew_id: Option<&str>,
) -> anyhow::Result<Option<Uuid>> {
    let Some(andrew_id) = andrew_id else {
        return Ok(None);
    };
    let id = sqlx::query_scalar("SELECT id FROM users WHERE andrew_id = $1")
        .bind(andrew_id)
        .fetch_optional(conn)
        .await?;
    Ok(id)
}

async fn insert_live(
    conn: &mut PgConnection,
    sample: &Sample,
    position: i32,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let posted_by = persona_id(conn, sample.posted_by).await?;
    sqlx::query(
        "INSERT INTO announcements
             (id, kind, committee, message, link_url, state, starts_at, expires_at, position, created_by)
         VALUES ($1, $2, $3, $4, $5, 'live', $6, $7, $8, $9)",
    )
    .bind(Uuid::now_v7())
    .bind(sample.kind)
    .bind(sample.committee)
    .bind(sample.message)
    .bind(sample.link_url)
    .bind(now)
    .bind(now + TimeDelta::days(sample.days_left))
    .bind(position)
    .bind(posted_by)
    .execute(conn)
    .await
    .with_context(|| format!("seeding the announcement {:?}", sample.message))?;
    Ok(())
}

/// Only when the lead persona exists, since the submission is theirs.
async fn insert_pending(
    conn: &mut PgConnection,
    sample: &Sample,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let Some(lead) = persona_id(conn, sample.posted_by).await? else {
        return Ok(());
    };
    let committee = sample.committee.committee().unwrap_or(Committee::Events);
    let id = Uuid::now_v7();
    let expires_at = now + TimeDelta::days(sample.days_left);
    sqlx::query(
        "INSERT INTO announcements
             (id, kind, committee, message, link_url, state, expires_at, created_by)
         VALUES ($1, $2, $3, $4, $5, 'pending_approval', $6, $7)",
    )
    .bind(id)
    .bind(sample.kind)
    .bind(sample.committee)
    .bind(sample.message)
    .bind(sample.link_url)
    .bind(expires_at)
    .bind(lead)
    .execute(&mut *conn)
    .await?;
    // The same shape as `AnnouncementInput`, which approving reads back.
    let snapshot = serde_json::json!({
        "kind": sample.kind,
        "committee": sample.committee,
        "message": sample.message,
        "linkUrl": sample.link_url,
        "expiresAt": expires_at,
    });
    approvals_core::submit(
        conn,
        SubjectType::Announcement,
        id,
        committee,
        lead,
        &snapshot,
    )
    .await
    .context("submitting the sample announcement for approval")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::testing;

    async fn states(db: &PgPool) -> Vec<(String, String)> {
        sqlx::query_as("SELECT message, state FROM announcements ORDER BY state, position, message")
            .fetch_all(db)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn seeds_collegecart_alone_without_demo_content(db: PgPool) {
        testing::ensure_committees(&db).await;
        let mut config = testing::config();
        config.seed_demo = false;
        let mut conn = db.acquire().await.unwrap();
        run(&mut conn, &config).await.unwrap();
        let seeded = states(&db).await;
        assert_eq!(seeded.len(), 1);
        assert_eq!(seeded[0].0, COLLEGECART.message);
        let days: f64 = sqlx::query_scalar(
            "SELECT extract(epoch FROM expires_at - starts_at)::float8 / 86400 FROM announcements",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!((days - 30.0).abs() < 0.01);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn seeds_the_demo_ticker_once(db: PgPool) {
        testing::seed_personas(&db).await;
        let config = testing::config();
        let mut conn = db.acquire().await.unwrap();
        run(&mut conn, &config).await.unwrap();
        let seeded = states(&db).await;
        assert_eq!(seeded.iter().filter(|(_, s)| s == "live").count(), 5);
        assert_eq!(
            seeded
                .iter()
                .filter(|(_, s)| s == "pending_approval")
                .map(|(m, _)| m.as_str())
                .collect::<Vec<_>>(),
            [PENDING.message]
        );
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM approvals WHERE subject_type = 'announcement' AND status = 'waiting' AND committee = 'events'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(waiting, 1);

        // Someone deletes one; seeding again leaves the ticker as it is.
        sqlx::query("DELETE FROM announcements WHERE committee = 'tech'")
            .execute(&db)
            .await
            .unwrap();
        run(&mut conn, &config).await.unwrap();
        assert_eq!(states(&db).await.len(), 5);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn skips_the_pending_sample_without_the_lead(db: PgPool) {
        testing::ensure_committees(&db).await;
        let mut conn = db.acquire().await.unwrap();
        run(&mut conn, &testing::config()).await.unwrap();
        let seeded = states(&db).await;
        assert_eq!(seeded.len(), 5);
        assert!(seeded.iter().all(|(_, s)| s == "live"));
    }
}
