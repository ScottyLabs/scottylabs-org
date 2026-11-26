//! Retention: an hourly cleanup of what the site no longer needs to keep. See "Retention" in docs/implementation/contract.md.
//!
//! - Sessions past their idle or absolute limit, and sign-in attempts more than an hour old.
//! - Announcements that left the ticker more than 90 days ago (or would have, for submissions never approved), with their approvals.
//! - Uploads that no draft, published version or open submission has used for seven days, with their files.
//! - Audit entries more than two years old, through the database's `expire_audit_log()`, the only way entries leave the log.
//!
//! Every step deletes only rows that match its rule when it runs, so a run that stops halfway or runs twice does no harm, and runs beside requests and other replicas safely: the quick steps run under a transaction-level advisory lock that a second replica skips, and each image is deleted under its own row lock. Logs carry counts, never names or ids.

use std::time::Duration;

use serde_json::json;
use sqlx::{FromRow, PgPool};
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use crate::audit::{self, subject};
use crate::auth::session;
use crate::error::AppResult;
use crate::media::MediaStore;
use crate::state::AppState;

/// How often the cleanup runs.
const INTERVAL: Duration = Duration::from_secs(60 * 60);
/// The first run waits this long after startup, so it never competes with seeding and the first requests.
const FIRST_RUN_AFTER: Duration = Duration::from_secs(5 * 60);
/// Held for a run's quick steps, so two replicas do not do the same work at once.
const CLEANUP_LOCK: i64 = 724_640;
/// Announcements stay this long after leaving the ticker, so they can be posted again (spec TK-9).
pub const ANNOUNCEMENT_DAYS: i32 = 90;
/// Uploads nothing uses are kept this long, in case an editor puts them back.
pub const UNUSED_MEDIA_DAYS: i32 = 7;
/// Sign-in attempts are good for ten minutes; they are kept an hour.
const ATTEMPT_MINUTES: i32 = 60;
/// Images deleted in one run at most, so a run stays short.
const MEDIA_BATCH: i64 = 200;

/// The uploads in use: those named in a project's draft or published version, a committee page's draft or published version, or a submission that is waiting or was sent back. A query that returns one `id` per image.
pub const MEDIA_IN_USE: &str = "SELECT DISTINCT (found.m)[1]::uuid AS id
    FROM (
        SELECT regexp_matches(
                   docs.doc::text,
                   '/media/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})',
                   'gi') AS m
        FROM (
            SELECT draft AS doc FROM projects
            UNION ALL SELECT published FROM projects WHERE published IS NOT NULL
            UNION ALL SELECT draft FROM committee_pages
            UNION ALL SELECT published FROM committee_pages WHERE published IS NOT NULL
            UNION ALL SELECT snapshot FROM approvals WHERE status IN ('waiting', 'sent_back')
        ) docs
    ) found";

/// What one run removed, for the log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub sessions: u64,
    pub sign_in_attempts: u64,
    pub announcements: u64,
    pub approvals: u64,
    /// Uploads newly found unused, and found in use again.
    pub media_unused: u64,
    pub media_used_again: u64,
    pub media_deleted: u64,
    pub audit_entries: i64,
}

impl Report {
    fn removed_anything(&self) -> bool {
        *self != Self::default()
    }
}

/// Runs the cleanup every hour while the service runs. A failed run logs a warning and the next one tries again.
pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut ticks =
            tokio::time::interval_at(tokio::time::Instant::now() + FIRST_RUN_AFTER, INTERVAL);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            match run(&state.db, &state.media).await {
                Ok(Some(report)) if report.removed_anything() => tracing::info!(
                    sessions = report.sessions,
                    sign_in_attempts = report.sign_in_attempts,
                    announcements = report.announcements,
                    approvals = report.approvals,
                    media_unused = report.media_unused,
                    media_used_again = report.media_used_again,
                    media_deleted = report.media_deleted,
                    audit_entries = report.audit_entries,
                    "cleaned up data past its retention"
                ),
                Ok(Some(_)) => tracing::debug!("nothing to clean up"),
                Ok(None) => tracing::debug!("another replica is cleaning up"),
                Err(error) => {
                    tracing::warn!(error = %error, "the cleanup failed; it runs again in an hour")
                }
            }
        }
    });
}

/// One cleanup. `None` when another replica holds the cleanup lock.
pub async fn run(db: &PgPool, media: &MediaStore) -> AppResult<Option<Report>> {
    let mut report = Report::default();
    let mut tx = db.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(CLEANUP_LOCK)
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Ok(None);
    }

    report.sessions = sqlx::query(
        "DELETE FROM sessions
         WHERE expires_at <= now() OR last_seen_at <= now() - make_interval(hours => $1)",
    )
    .bind(i32::try_from(session::IDLE_TIMEOUT_HOURS).unwrap_or(12))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    report.sign_in_attempts = sqlx::query(
        "DELETE FROM auth_attempts WHERE created_at <= now() - make_interval(mins => $1)",
    )
    .bind(ATTEMPT_MINUTES)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let gone: Vec<Uuid> = sqlx::query_scalar(
        "DELETE FROM announcements
         WHERE coalesce(ended_at, expires_at) <= now() - make_interval(days => $1)
         RETURNING id",
    )
    .bind(ANNOUNCEMENT_DAYS)
    .fetch_all(&mut *tx)
    .await?;
    report.announcements = gone.len() as u64;
    report.approvals = sqlx::query(
        "DELETE FROM approvals WHERE subject_type = 'announcement' AND subject_id = ANY($1)",
    )
    .bind(&gone)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    report.audit_entries = sqlx::query_scalar("SELECT expire_audit_log()")
        .fetch_one(&mut *tx)
        .await?;

    report.media_unused = sqlx::query(&format!(
        "UPDATE media m SET unreferenced_since = now()
         WHERE m.unreferenced_since IS NULL
           AND NOT EXISTS (SELECT 1 FROM ({MEDIA_IN_USE}) used WHERE used.id = m.id)"
    ))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    report.media_used_again = sqlx::query(&format!(
        "UPDATE media m SET unreferenced_since = NULL
         WHERE m.unreferenced_since IS NOT NULL
           AND EXISTS (SELECT 1 FROM ({MEDIA_IN_USE}) used WHERE used.id = m.id)"
    ))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;

    let due: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM media
         WHERE unreferenced_since <= now() - make_interval(days => $1)
         ORDER BY unreferenced_since, id
         LIMIT $2",
    )
    .bind(UNUSED_MEDIA_DAYS)
    .bind(MEDIA_BATCH)
    .fetch_all(db)
    .await?;
    for id in due {
        match delete_unused(db, media, id).await {
            Ok(true) => report.media_deleted += 1,
            Ok(false) => {}
            // Storage may be down for a moment; the image stays and the next run tries again.
            Err(error) => tracing::warn!(error = %error, "could not delete an unused upload"),
        }
    }
    Ok(Some(report))
}

#[derive(FromRow)]
struct Unused {
    storage: String,
    variants: Vec<i32>,
    bytes: i64,
    uploaded_by: Option<Uuid>,
}

/// Deletes one upload that has been unused for [`UNUSED_MEDIA_DAYS`], files first. The row stays locked meanwhile and the check runs again under the lock, so an image a draft started using again in the meantime stays. Returns whether it was deleted.
async fn delete_unused(db: &PgPool, media: &MediaStore, id: Uuid) -> AppResult<bool> {
    let mut tx = db.begin().await?;
    let Some(unused) = sqlx::query_as::<_, Unused>(
        "SELECT storage, variants, bytes, uploaded_by FROM media
         WHERE id = $1 AND unreferenced_since <= now() - make_interval(days => $2)
         FOR UPDATE SKIP LOCKED",
    )
    .bind(id)
    .bind(UNUSED_MEDIA_DAYS)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };
    let used_again: bool = sqlx::query_scalar(&format!(
        "SELECT EXISTS (SELECT 1 FROM ({MEDIA_IN_USE}) used WHERE used.id = $1)"
    ))
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if used_again {
        sqlx::query("UPDATE media SET unreferenced_since = NULL WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(false);
    }
    media
        .remove_objects(id, &unused.storage, &unused.variants)
        .await?;
    sqlx::query("DELETE FROM media WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::record_details(
        &mut tx,
        audit::Entry {
            actor: None,
            action: "media.expire",
            subject_type: subject::MEDIA,
            subject_id: Some(id.to_string()),
            committee: None,
            summary: format!(
                "Deleted an uploaded image no page had used for {UNUSED_MEDIA_DAYS} days"
            ),
        },
        audit::change(
            json!({
                "id": id,
                "bytes": unused.bytes,
                "uploadedBy": unused.uploaded_by,
                "storage": unused.storage,
            }),
            serde_json::Value::Null,
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests;
