//! Committee pages: what visitors see under /committees/ and the portal's page editor. See "Committee pages" in docs/implementation/contract.md.
//!
//! Each committee has one row in `committee_pages` with a draft and the published version, both a [`CommitteePageDoc`]. Editors work on the draft while holding a five minute edit lock, which the editor refreshes every minute; saves carry the draft version they started from so two tabs cannot overwrite each other.

mod editor;
mod public;
#[cfg(test)]
mod tests;

use axum::Router;
use axum::routing::{get, post, put};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as JsonColumn;
use sqlx::{FromRow, PgConnection, PgExecutor};
use uuid::Uuid;

use crate::blocks::CommitteePageDoc;
use crate::error::{AppError, AppResult};
use crate::models::{Committee, ImageRef};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/public/committees", get(public::list))
        .route("/public/committees/{slug}", get(public::page))
        .route("/committee-pages", get(editor::list))
        .route("/committee-pages/{slug}", get(editor::show))
        .route(
            "/committee-pages/{slug}/lock",
            post(editor::lock).delete(editor::unlock),
        )
        .route("/committee-pages/{slug}/draft", put(editor::save_draft))
        .route("/committee-pages/{slug}/changes", get(editor::changes))
        .route("/committee-pages/{slug}/publish", post(editor::publish))
        .route("/committee-pages/{slug}/discard", post(editor::discard))
}

/// A lock older than this without a heartbeat is free for anyone to take.
pub const LOCK_MINUTES: i32 = 5;

fn committee_from_slug(slug: &str) -> AppResult<Committee> {
    Committee::from_slug(slug).ok_or_else(|| AppError::not_found("committee page"))
}

/// Mirrors `CommitteePageStatus` in types.ts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    Published,
    UnpublishedChanges,
    NeverPublished,
}

/// A committee page with its committee and the people around it, as stored.
#[derive(FromRow)]
struct PageRow {
    committee: Committee,
    name: String,
    color: String,
    tint: String,
    draft: JsonColumn<CommitteePageDoc>,
    published: Option<JsonColumn<CommitteePageDoc>>,
    draft_version: i32,
    draft_updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    /// The draft and the published version are the same document.
    in_sync: bool,
    editor_id: Option<Uuid>,
    editor_andrew_id: Option<String>,
    editor_name: Option<String>,
    lock_user_id: Option<Uuid>,
    lock_andrew_id: Option<String>,
    lock_name: Option<String>,
    lock_since: Option<DateTime<Utc>>,
    /// The holder sent a heartbeat within the last [`LOCK_MINUTES`].
    lock_fresh: bool,
}

/// Selects [`PageRow`]s; add a WHERE or ORDER BY clause.
fn page_select() -> String {
    format!(
        "SELECT c.slug AS committee, c.name, c.color, c.tint,
        p.draft, p.published, p.draft_version, p.draft_updated_at, p.published_at,
        coalesce(p.published = p.draft, false) AS in_sync,
        du.id AS editor_id, du.andrew_id AS editor_andrew_id, du.name AS editor_name,
        p.lock_user_id, lu.andrew_id AS lock_andrew_id, lu.name AS lock_name, p.lock_since,
        coalesce(p.lock_heartbeat_at > now() - make_interval(mins => {LOCK_MINUTES}), false) AS lock_fresh
    FROM committee_pages p
    JOIN committees c ON c.slug = p.committee
    LEFT JOIN users du ON du.id = p.draft_updated_by
    LEFT JOIN users lu ON lu.id = p.lock_user_id"
    )
}

impl PageRow {
    fn status(&self) -> PageStatus {
        if self.published.is_none() {
            PageStatus::NeverPublished
        } else if self.in_sync {
            PageStatus::Published
        } else {
            PageStatus::UnpublishedChanges
        }
    }

    fn draft_editor(&self) -> Option<crate::models::PersonRef> {
        person(
            self.editor_id,
            self.editor_andrew_id.clone(),
            self.editor_name.clone(),
        )
    }

    /// Who holds the edit lock, while it is fresh.
    fn lock_holder(&self) -> Option<crate::models::PersonRef> {
        if !self.lock_fresh {
            return None;
        }
        person(
            self.lock_user_id,
            self.lock_andrew_id.clone(),
            self.lock_name.clone(),
        )
    }

    fn committee_ref(&self) -> CommitteeRef {
        CommitteeRef {
            slug: self.committee,
            name: self.name.clone(),
            color: self.color.clone(),
            tint: self.tint.clone(),
        }
    }
}

fn person(
    id: Option<Uuid>,
    andrew_id: Option<String>,
    name: Option<String>,
) -> Option<crate::models::PersonRef> {
    match (id, andrew_id) {
        (Some(id), Some(andrew_id)) => Some(crate::models::PersonRef::new(
            id,
            andrew_id,
            name.unwrap_or_default(),
        )),
        _ => None,
    }
}

/// Every page, in the site's order.
async fn all_pages<'e>(db: impl PgExecutor<'e>) -> AppResult<Vec<PageRow>> {
    let rows = sqlx::query_as::<_, PageRow>(&format!("{} ORDER BY c.sort_order", page_select()))
        .fetch_all(db)
        .await?;
    Ok(rows)
}

/// One page, or 404 when the committee has none.
async fn one_page(conn: &mut PgConnection, committee: Committee) -> AppResult<PageRow> {
    sqlx::query_as::<_, PageRow>(&format!("{} WHERE p.committee = $1", page_select()))
        .bind(committee)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| AppError::not_found("committee page"))
}

/// `{ slug, name, color, tint }`, as in `CommitteePageEditorResponse.committee`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitteeRef {
    slug: Committee,
    name: String,
    color: String,
    tint: String,
}

/// Mirrors `ProjectCard` in types.ts, read straight from the projects table.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCard {
    id: Uuid,
    slug: String,
    name: String,
    summary: String,
    excerpt: String,
    committee: Committee,
    status: String,
    kind: String,
    cover: Option<ImageRef>,
    site_url: Option<String>,
    repo_url: Option<String>,
    featured: bool,
}

/// The parts of a published `ProjectDoc` that a card shows. Lenient, so a project the projects track stores with more fields still reads.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PublishedProject {
    name: String,
    summary: String,
    description: String,
    status: Option<String>,
    kind: Option<String>,
    site_url: Option<String>,
    repo_url: Option<String>,
    media: Vec<ImageRef>,
}

#[derive(FromRow)]
struct ProjectRow {
    id: Uuid,
    slug: String,
    committee: Committee,
    featured: bool,
    published: JsonColumn<serde_json::Value>,
}

/// Longest excerpt on a card, in characters.
const EXCERPT_CHARS: usize = 240;

/// The committee's published projects, featured first, then by name.
async fn published_projects<'e>(
    db: impl PgExecutor<'e>,
    committee: Committee,
) -> AppResult<Vec<ProjectCard>> {
    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, slug, committee, featured, published FROM projects
         WHERE committee = $1 AND state = 'published' AND published IS NOT NULL
         ORDER BY featured DESC, lower(published->>'name'), slug",
    )
    .bind(committee)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(ProjectRow::into_card).collect())
}

impl ProjectRow {
    fn into_card(self) -> ProjectCard {
        let doc: PublishedProject = serde_json::from_value(self.published.0).unwrap_or_default();
        ProjectCard {
            id: self.id,
            excerpt: crate::richtext::excerpt(&doc.description, EXCERPT_CHARS),
            name: if doc.name.is_empty() {
                self.slug.clone()
            } else {
                doc.name
            },
            slug: self.slug,
            summary: doc.summary,
            committee: self.committee,
            status: doc.status.unwrap_or_else(|| "live".into()),
            kind: doc.kind.unwrap_or_else(|| "other".into()),
            cover: doc.media.into_iter().next(),
            site_url: doc.site_url,
            repo_url: doc.repo_url,
            featured: self.featured,
        }
    }
}
