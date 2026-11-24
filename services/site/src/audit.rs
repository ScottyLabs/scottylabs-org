//! The audit log behind "Recent activity". Write an entry in the same transaction as the change.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::Committee;

/// One change, described for people, for example "Published the Tech page".
pub struct Entry<'a> {
    pub actor: Option<Uuid>,
    /// Machine name such as `announcement.publish`.
    pub action: &'a str,
    pub subject_type: &'a str,
    pub subject_id: Option<String>,
    pub committee: Option<Committee>,
    pub summary: String,
}

pub async fn record(conn: &mut PgConnection, entry: Entry<'_>) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, subject_type, subject_id, committee, summary)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(entry.actor)
    .bind(entry.action)
    .bind(entry.subject_type)
    .bind(entry.subject_id)
    .bind(entry.committee)
    .bind(entry.summary)
    .execute(conn)
    .await?;
    Ok(())
}
