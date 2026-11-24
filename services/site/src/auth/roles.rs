//! Role assignments as the browser sees them (`RoleAssignment` in types.ts), for `/api/me` and the People page.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{Committee, PersonRef, Role, RoleSource};

/// Mirrors `RoleAssignment` in types.ts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleAssignmentDto {
    pub id: Uuid,
    pub role: Role,
    pub committee: Option<Committee>,
    pub can_post_ticker: bool,
    pub project_ids: Vec<Uuid>,
    pub appointed_by: Option<PersonRef>,
    pub appointed_at: DateTime<Utc>,
    pub source: RoleSource,
}

#[derive(FromRow)]
struct AssignmentListRow {
    id: Uuid,
    user_id: Uuid,
    role: Role,
    committee: Option<Committee>,
    can_post_ticker: bool,
    created_at: DateTime<Utc>,
    source: RoleSource,
    appointer_id: Option<Uuid>,
    appointer_andrew_id: Option<String>,
    appointer_name: Option<String>,
    project_ids: Vec<Uuid>,
}

/// Every role of the given people, keyed by user: admin first, then by committee order.
pub async fn assignments_for<'e>(
    db: impl PgExecutor<'e>,
    user_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, Vec<RoleAssignmentDto>>> {
    let rows = sqlx::query_as::<_, AssignmentListRow>(
        "SELECT r.id, r.user_id, r.role, r.committee, r.can_post_ticker, r.created_at, r.source,
                a.id AS appointer_id, a.andrew_id AS appointer_andrew_id, a.name AS appointer_name,
                CASE WHEN r.role = 'lead' THEN coalesce((
                    SELECT array_agg(pe.project_id ORDER BY pe.project_id)
                    FROM project_editors pe JOIN projects p ON p.id = pe.project_id
                    WHERE pe.user_id = r.user_id AND p.committee = r.committee
                ), '{}') ELSE '{}' END AS project_ids
         FROM role_assignments r
         LEFT JOIN users a ON a.id = r.appointed_by
         LEFT JOIN committees c ON c.slug = r.committee
         WHERE r.user_id = ANY($1)
         ORDER BY r.user_id,
                  CASE r.role WHEN 'admin' THEN 0 WHEN 'director' THEN 1 ELSE 2 END,
                  c.sort_order, r.created_at",
    )
    .bind(user_ids)
    .fetch_all(db)
    .await?;
    let mut map: HashMap<Uuid, Vec<RoleAssignmentDto>> = HashMap::new();
    for row in rows {
        let appointed_by = match (row.appointer_id, row.appointer_andrew_id) {
            (Some(id), Some(andrew_id)) => Some(PersonRef::new(
                id,
                andrew_id,
                row.appointer_name.unwrap_or_default(),
            )),
            _ => None,
        };
        map.entry(row.user_id).or_default().push(RoleAssignmentDto {
            id: row.id,
            role: row.role,
            committee: row.committee,
            can_post_ticker: row.can_post_ticker,
            project_ids: row.project_ids,
            appointed_by,
            appointed_at: row.created_at,
            source: row.source,
        });
    }
    Ok(map)
}
