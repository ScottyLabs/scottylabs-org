//! GET /api/overview: what is live in each section and what needs the viewer, scoped to their roles. See "Overview and activity" in docs/implementation/contract.md.

mod attention;
mod when;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::audit::{self, ActivityEntry};
use crate::auth::{PortalViewer, Viewer};
use crate::error::AppResult;
use crate::models::{Committee, ImageRef, PersonRef, TickerSource};
use crate::permissions;
use crate::state::AppState;

use attention::AttentionItem;

pub fn router() -> Router<AppState> {
    Router::new().route("/overview", get(overview))
}

/// Announcements the ticker shows at once.
const TICKER_SLOTS: i64 = 6;
/// Project covers shown on the Projects card.
const PROJECT_COVERS: usize = 6;

/// What the viewer looks after, worked out once from their roles.
struct Scope {
    user_id: Uuid,
    admin: bool,
    /// Ticker sources whose announcements the viewer extends and ends.
    ticker_sources: Vec<&'static str>,
    /// Committees whose pages the viewer edits.
    pages: Vec<&'static str>,
    /// Committees whose projects the viewer edits and publishes.
    project_committees: Vec<&'static str>,
    /// Projects the viewer edits as a lead.
    lead_projects: Vec<Uuid>,
    /// Committees whose submissions the viewer decides on.
    approves_for: Vec<&'static str>,
}

impl Scope {
    fn of(viewer: &Viewer) -> Self {
        let slugs = |list: Vec<Committee>| list.into_iter().map(Committee::slug).collect();
        Self {
            user_id: viewer.user_id,
            admin: permissions::is_admin(viewer),
            ticker_sources: permissions::ticker_manages(viewer)
                .into_iter()
                .map(TickerSource::slug)
                .collect(),
            pages: slugs(permissions::editable_pages(viewer)),
            project_committees: slugs(permissions::project_committees(viewer)),
            lead_projects: viewer.lead_projects.clone(),
            approves_for: slugs(permissions::approves_for(viewer)),
        }
    }
}

/// Mirrors `OverviewResponse` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewResponse {
    ticker: TickerOverview,
    committees: CommitteesOverview,
    projects: ProjectsOverview,
    attention: Vec<AttentionItem>,
    activity: Vec<ActivityEntry>,
}

async fn overview(
    State(state): State<AppState>,
    viewer: PortalViewer,
) -> AppResult<Json<OverviewResponse>> {
    let now = Utc::now();
    let scope = Scope::of(&viewer);
    let mut conn = state.db.acquire().await?;
    let ticker = ticker_overview(&mut conn, &scope, now).await?;
    let committees = committees_overview(&mut conn, &scope).await?;
    let projects = projects_overview(&mut conn, &scope).await?;
    let attention = attention::collect(&mut conn, &scope, now).await?;
    let activity = audit::recent(&mut *conn, &viewer, audit::DEFAULT_ACTIVITY).await?;
    Ok(Json(OverviewResponse {
        ticker,
        committees,
        projects,
        attention,
        activity,
    }))
}

// ---------------------------------------------------------------------------
// Ticker

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TickerOverview {
    live: i64,
    slots: i64,
    /// Live announcements from sources the viewer manages, or that they posted.
    mine_live: i64,
    /// The first of those to leave the ticker.
    next_to_expire: Option<NextToExpire>,
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct NextToExpire {
    id: Uuid,
    message: String,
    expires_at: DateTime<Utc>,
}

async fn ticker_overview(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
) -> AppResult<TickerOverview> {
    let (live, mine_live): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE committee = ANY($2) OR created_by = $3)
         FROM announcements WHERE state = 'live' AND expires_at > $1",
    )
    .bind(now)
    .bind(&scope.ticker_sources)
    .bind(scope.user_id)
    .fetch_one(&mut *conn)
    .await?;
    let next_to_expire = sqlx::query_as::<_, NextToExpire>(
        "SELECT id, message, expires_at FROM announcements
         WHERE state = 'live' AND expires_at > $1 AND (committee = ANY($2) OR created_by = $3)
         ORDER BY expires_at, id LIMIT 1",
    )
    .bind(now)
    .bind(&scope.ticker_sources)
    .bind(scope.user_id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(TickerOverview {
        live,
        slots: TICKER_SLOTS,
        mine_live,
        next_to_expire,
    })
}

// ---------------------------------------------------------------------------
// Committee pages

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitteesOverview {
    total: usize,
    published: usize,
    /// Pages the viewer edits whose draft differs from what visitors see.
    with_unpublished_changes: Vec<Committee>,
    editable: Vec<Committee>,
    covers: Vec<CommitteeCover>,
    /// When each page the viewer edits was last saved, and by whom.
    last_edited: Vec<PageEdit>,
}

#[derive(Serialize)]
struct CommitteeCover {
    slug: Committee,
    cover: Option<ImageRef>,
}

#[derive(Serialize)]
struct PageEdit {
    slug: Committee,
    at: DateTime<Utc>,
    by: Option<PersonRef>,
}

#[derive(FromRow)]
struct PageRow {
    slug: Committee,
    published: bool,
    changed: bool,
    cover: Option<serde_json::Value>,
    edited_at: Option<DateTime<Utc>>,
    editor_id: Option<Uuid>,
    editor_andrew_id: Option<String>,
    editor_name: Option<String>,
}

impl PageRow {
    /// The last save of the draft. Publishing copies the draft and is not an edit.
    fn last_edit(&self) -> Option<PageEdit> {
        let editor = match (self.editor_id, &self.editor_andrew_id) {
            (Some(id), Some(andrew_id)) => Some(PersonRef::new(
                id,
                andrew_id.clone(),
                self.editor_name.clone().unwrap_or_default(),
            )),
            _ => None,
        };
        self.edited_at.map(|at| PageEdit {
            slug: self.slug,
            at,
            by: editor,
        })
    }
}

/// An image stored in a document, if it is one the site may show.
fn image(value: Option<serde_json::Value>) -> Option<ImageRef> {
    value
        .and_then(|value| serde_json::from_value::<ImageRef>(value).ok())
        .filter(|image| image.check(false).is_ok())
}

async fn committees_overview(
    conn: &mut PgConnection,
    scope: &Scope,
) -> AppResult<CommitteesOverview> {
    // The cover is the image of the published page's cover block.
    let rows = sqlx::query_as::<_, PageRow>(
        "SELECT c.slug,
                cp.published IS NOT NULL AS published,
                cp.committee IS NOT NULL AND cp.draft IS DISTINCT FROM cp.published AS changed,
                (SELECT b->'props'->'image'
                 FROM jsonb_array_elements(
                     CASE WHEN jsonb_typeof(cp.published->'blocks') = 'array'
                          THEN cp.published->'blocks' ELSE '[]'::jsonb END) AS b
                 WHERE b->>'type' = 'cover' LIMIT 1) AS cover,
                cp.draft_updated_at AS edited_at,
                u.id AS editor_id, u.andrew_id AS editor_andrew_id, u.name AS editor_name
         FROM committees c
         LEFT JOIN committee_pages cp ON cp.committee = c.slug
         LEFT JOIN users u ON u.id = cp.draft_updated_by
         ORDER BY c.sort_order",
    )
    .fetch_all(conn)
    .await?;
    let editable: Vec<Committee> = rows
        .iter()
        .map(|row| row.slug)
        .filter(|slug| scope.pages.contains(&slug.slug()))
        .collect();
    Ok(CommitteesOverview {
        total: rows.len(),
        published: rows.iter().filter(|row| row.published).count(),
        with_unpublished_changes: rows
            .iter()
            .filter(|row| row.changed && editable.contains(&row.slug))
            .map(|row| row.slug)
            .collect(),
        last_edited: rows
            .iter()
            .filter(|row| editable.contains(&row.slug))
            .filter_map(PageRow::last_edit)
            .collect(),
        editable,
        covers: rows
            .into_iter()
            .map(|row| CommitteeCover {
                slug: row.slug,
                cover: image(row.cover),
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Projects

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectsOverview {
    /// Every project that is not archived.
    total: i64,
    /// Of the projects in scope.
    published: i64,
    drafts: i64,
    /// Projects the viewer edits.
    in_scope: i64,
    covers: Vec<ImageRef>,
}

#[derive(FromRow)]
struct ProjectRow {
    state: String,
    in_scope: bool,
    cover: Option<serde_json::Value>,
}

async fn projects_overview(conn: &mut PgConnection, scope: &Scope) -> AppResult<ProjectsOverview> {
    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT p.state,
                ($1 OR p.committee = ANY($2) OR p.id = ANY($3)) AS in_scope,
                coalesce(p.published->'media'->0, p.draft->'media'->0) AS cover
         FROM projects p
         WHERE p.state <> 'archived'
         ORDER BY p.state = 'published' DESC, p.featured DESC,
                  lower(coalesce(p.published->>'name', p.draft->>'name', p.slug)), p.id",
    )
    .bind(scope.admin)
    .bind(&scope.project_committees)
    .bind(&scope.lead_projects)
    .fetch_all(conn)
    .await?;
    let count = |keep: &dyn Fn(&ProjectRow) -> bool| {
        i64::try_from(rows.iter().filter(|row| keep(row)).count()).unwrap_or(i64::MAX)
    };
    let total = count(&|_| true);
    let in_scope = count(&|row| row.in_scope);
    let published = count(&|row| row.in_scope && row.state == "published");
    let drafts = count(&|row| row.in_scope && row.state == "draft");
    let covers = rows
        .into_iter()
        .filter(|row| row.in_scope)
        .filter_map(|row| image(row.cover))
        .take(PROJECT_COVERS)
        .collect();
    Ok(ProjectsOverview {
        total,
        published,
        drafts,
        in_scope,
        covers,
    })
}

#[cfg(test)]
mod tests;
