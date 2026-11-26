//! What happens to a project when an approver decides on what a lead sent. routes/approvals.rs calls these inside its transaction and records the decision on the approval itself.

use sqlx::PgConnection;

use super::doc::{self, ProjectDoc, ProjectState, Purpose};
use super::portal::{lock, publish_doc};
use crate::approvals_core::{ApprovalRow, SubjectType};
use crate::audit::{self, subject};
use crate::auth::Viewer;
use crate::error::{AppError, AppResult};
use crate::permissions;

/// Publishes exactly what the lead sent: the approval's snapshot, checked again against the publishing rules. Edits made after it was sent stay in the draft.
pub async fn apply_approval(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    approver: &Viewer,
) -> AppResult<()> {
    check_subject(approval)?;
    // `routes/approvals.rs` locked the approval; the project comes second, as in every project write.
    let project = lock(conn, approval.subject_id).await?;
    let approver = &permissions::recheck(conn, approver).await?;
    if !permissions::can_approve_for(approver, project.committee) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can approve its projects.",
            project.committee.name()
        )));
    }
    if project.state == ProjectState::Archived {
        return Err(AppError::Conflict(format!(
            "{} was archived after it was sent. Unarchive it before approving.",
            project.draft.name
        )));
    }
    let submitted = snapshot_doc(approval)?;
    if submitted.committee != project.committee {
        return Err(AppError::Conflict(format!(
            "{} moved to {} after it was sent. Ask the lead to send it again.",
            project.draft.name,
            project.committee.name()
        )));
    }
    let clean = doc::validate(conn, submitted, Purpose::Publish).await?;
    let draft_after = if project.draft.0 == clean || is_snapshot(&project.draft.0, approval) {
        None
    } else {
        // Keep later edits in the draft; only the published version changes.
        Some(project.draft.0.clone())
    };
    publish_doc(conn, project.id, &clean, approver.user_id).await?;
    if let Some(draft) = draft_after {
        sqlx::query("UPDATE projects SET draft = $2 WHERE id = $1")
            .bind(project.id)
            .bind(sqlx::types::Json(&draft))
            .execute(&mut *conn)
            .await?;
    }
    // What visitors saw before and see now, as for a direct publish.
    audit::record_details(
        conn,
        audit::Entry {
            actor: Some(approver.user_id),
            action: "project.approve",
            subject_type: subject::PROJECT,
            subject_id: Some(project.id.to_string()),
            committee: Some(project.committee),
            summary: format!("Approved and published {}", clean.name),
        },
        serde_json::json!({
            "approval": approval.id,
            "before": project.published.as_ref().map(|doc| &doc.0),
            "after": &clean,
        }),
    )
    .await
}

/// Leaves the project as it is, so the lead can edit and send it again.
pub async fn apply_send_back(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    approver: &Viewer,
) -> AppResult<()> {
    check_subject(approval)?;
    let project = lock(conn, approval.subject_id).await?;
    let approver = &permissions::recheck(conn, approver).await?;
    if !permissions::can_approve_for(approver, project.committee) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can send its projects back.",
            project.committee.name()
        )));
    }
    let name = snapshot_doc(approval).map_or_else(|_| project.draft.name.clone(), |doc| doc.name);
    audit::record_details(
        conn,
        audit::Entry {
            actor: Some(approver.user_id),
            action: "project.send_back",
            subject_type: subject::PROJECT,
            subject_id: Some(project.id.to_string()),
            committee: Some(project.committee),
            summary: format!("Sent {name} back with a note"),
        },
        serde_json::json!({ "approval": approval.id }),
    )
    .await
}

fn check_subject(approval: &ApprovalRow) -> AppResult<()> {
    if approval.subject_type == SubjectType::Project {
        Ok(())
    } else {
        Err(AppError::Internal(anyhow::anyhow!(
            "approval {} is not about a project",
            approval.id
        )))
    }
}

fn snapshot_doc(approval: &ApprovalRow) -> AppResult<ProjectDoc> {
    serde_json::from_value(approval.snapshot.clone()).map_err(|error| {
        AppError::Internal(anyhow::anyhow!(
            "approval {} has an unreadable project snapshot: {error}",
            approval.id
        ))
    })
}

/// Whether the draft is still what the lead sent.
fn is_snapshot(draft: &ProjectDoc, approval: &ApprovalRow) -> bool {
    snapshot_doc(approval).is_ok_and(|submitted| submitted == *draft)
}
