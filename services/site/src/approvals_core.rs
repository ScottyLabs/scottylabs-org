//! Creating and finding approvals. Used by the ticker and projects when a lead submits, and by routes/approvals.rs when an approver decides.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{Committee, text_enum};

text_enum!(SubjectType {
    Announcement => "announcement",
    Project => "project",
});

text_enum!(ApprovalStatus {
    Waiting => "waiting",
    Approved => "approved",
    SentBack => "sent_back",
    Withdrawn => "withdrawn",
});

#[derive(Clone, Debug, FromRow)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub subject_type: SubjectType,
    pub subject_id: Uuid,
    pub committee: Committee,
    pub snapshot: serde_json::Value,
    pub status: ApprovalStatus,
    pub note: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
}

const COLUMNS: &str =
    "id, subject_type, subject_id, committee, snapshot, status, note, decided_by, decided_at";

/// Withdraws any waiting approval for the subject, then opens a new one.
pub async fn submit(
    conn: &mut PgConnection,
    subject_type: SubjectType,
    subject_id: Uuid,
    committee: Committee,
    submitted_by: Uuid,
    snapshot: &serde_json::Value,
) -> AppResult<ApprovalRow> {
    withdraw_waiting(conn, subject_type, subject_id).await?;
    let row = sqlx::query_as::<_, ApprovalRow>(&format!(
        "INSERT INTO approvals (id, subject_type, subject_id, committee, submitted_by, snapshot, status)
         VALUES ($1, $2, $3, $4, $5, $6, 'waiting')
         RETURNING {COLUMNS}"
    ))
    .bind(Uuid::now_v7())
    .bind(subject_type)
    .bind(subject_id)
    .bind(committee)
    .bind(submitted_by)
    .bind(snapshot)
    .fetch_one(conn)
    .await?;
    Ok(row)
}

/// Marks a waiting approval for the subject as withdrawn, if there is one.
pub async fn withdraw_waiting(
    conn: &mut PgConnection,
    subject_type: SubjectType,
    subject_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE approvals SET status = 'withdrawn', decided_at = now()
         WHERE subject_type = $1 AND subject_id = $2 AND status = 'waiting'",
    )
    .bind(subject_type)
    .bind(subject_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// The most recent approval for a subject, in any status.
pub async fn latest(
    conn: &mut PgConnection,
    subject_type: SubjectType,
    subject_id: Uuid,
) -> AppResult<Option<ApprovalRow>> {
    let row = sqlx::query_as::<_, ApprovalRow>(&format!(
        "SELECT {COLUMNS} FROM approvals WHERE subject_type = $1 AND subject_id = $2
         ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(subject_type)
    .bind(subject_id)
    .fetch_optional(conn)
    .await?;
    Ok(row)
}

/// Locks and returns an approval for deciding on it.
pub async fn lock_for_decision(
    conn: &mut PgConnection,
    id: Uuid,
) -> AppResult<Option<ApprovalRow>> {
    let row = sqlx::query_as::<_, ApprovalRow>(&format!(
        "SELECT {COLUMNS} FROM approvals WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row)
}
