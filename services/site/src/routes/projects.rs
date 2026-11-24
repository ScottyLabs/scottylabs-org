//! projects endpoints, public and portal. Projects track. See docs/implementation/contract.md.

use axum::Router;
use sqlx::PgConnection;

use crate::approvals_core::ApprovalRow;
use crate::auth::Viewer;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}

/// Called by routes/approvals.rs, inside its transaction, when an approver approves a submitted project. Publishes exactly the approval's snapshot.
pub async fn apply_approval(
    _conn: &mut PgConnection,
    _approval: &ApprovalRow,
    _approver: &Viewer,
) -> AppResult<()> {
    Err(AppError::Internal(anyhow::anyhow!(
        "approving projects is not implemented yet"
    )))
}
