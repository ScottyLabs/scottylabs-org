//! announcements endpoints, public and portal. Ticker track. See docs/implementation/contract.md.

use axum::Router;
use sqlx::PgConnection;

use crate::approvals_core::ApprovalRow;
use crate::auth::Viewer;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}

/// Called by routes/approvals.rs, inside its transaction, when an approver approves a submitted announcement. Publishes exactly the approval's snapshot.
pub async fn apply_approval(
    _conn: &mut PgConnection,
    _approval: &ApprovalRow,
    _approver: &Viewer,
) -> AppResult<()> {
    Err(AppError::Internal(anyhow::anyhow!(
        "approving announcements is not implemented yet"
    )))
}

/// Called by routes/approvals.rs, inside its transaction, when an approver sends a submitted announcement back with a note. Sets the announcement back to `sent_back` so the lead can edit and resubmit it.
pub async fn apply_send_back(
    _conn: &mut PgConnection,
    _approval: &ApprovalRow,
    _approver: &Viewer,
) -> AppResult<()> {
    Err(AppError::Internal(anyhow::anyhow!(
        "sending announcements back is not implemented yet"
    )))
}
