//! The portal's committee pages: the list, the editor's payload, the edit lock, saving the draft, the change list, publishing and discarding.
//!
//! Who may edit is decided by `permissions::can_edit_page`. Every write takes or refreshes the caller's edit lock first and fails with 409 while someone else holds a fresh one.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection, PgExecutor};
use uuid::Uuid;

use super::{
    CommitteeRef, PageRow, PageStatus, ProjectCard, all_pages, committee_from_slug, one_page,
    published_projects,
};
use crate::audit::{self, subject};
use crate::auth::{PortalViewer, Viewer};
use crate::blocks::{self, CommitteePageDoc};
use crate::error::{AppError, AppResult, Problems};
use crate::json::Json as JsonBody;
use crate::models::{Committee, ImageRef, PersonRef};
use crate::permissions;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Shapes

/// Mirrors `CommitteePageSummary` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageSummary {
    #[serde(flatten)]
    committee: CommitteeRef,
    status: PageStatus,
    cover: Option<ImageRef>,
    published_at: Option<DateTime<Utc>>,
    draft_updated_at: DateTime<Utc>,
    draft_updated_by: Option<PersonRef>,
    directors: Vec<PersonRef>,
    can_edit: bool,
}

#[derive(Serialize)]
pub(super) struct PagesResponse {
    pages: Vec<PageSummary>,
}

/// Mirrors `EditLock` in types.ts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EditLock {
    holder: PersonRef,
    since: DateTime<Utc>,
    mine: bool,
}

/// Mirrors `CommitteePageEditorResponse` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EditorResponse {
    committee: CommitteeRef,
    draft: CommitteePageDoc,
    published: Option<CommitteePageDoc>,
    draft_version: i32,
    status: PageStatus,
    lock: Option<EditLock>,
    can_edit: bool,
    projects: Vec<ProjectCard>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveDraftInput {
    doc: CommitteePageDoc,
    base_version: i32,
}

/// Mirrors `SaveDraftResponse` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveDraftResponse {
    draft_version: i32,
    saved_at: DateTime<Utc>,
    status: PageStatus,
}

#[derive(Serialize)]
pub(super) struct ChangesResponse {
    changes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublishInput {
    draft_version: i32,
}

// ---------------------------------------------------------------------------
// Reading

/// GET /api/committee-pages: the seven pages, with who directs each and whether the viewer may edit it.
pub(super) async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
) -> AppResult<Json<PagesResponse>> {
    let rows = all_pages(&state.db).await?;
    let mut directors = directors_by_committee(&state.db).await?;
    let pages = rows
        .into_iter()
        .map(|row| PageSummary {
            committee: row.committee_ref(),
            status: row.status(),
            cover: row.draft.0.cover().and_then(|cover| cover.image.clone()),
            published_at: row.published_at,
            draft_updated_at: row.draft_updated_at,
            draft_updated_by: row.draft_editor(),
            directors: directors.remove(&row.committee).unwrap_or_default(),
            can_edit: permissions::can_edit_page(&viewer, row.committee),
        })
        .collect();
    Ok(Json(PagesResponse { pages }))
}

#[derive(FromRow)]
struct DirectorRow {
    committee: Committee,
    id: Uuid,
    andrew_id: String,
    name: String,
}

async fn directors_by_committee<'e>(
    db: impl PgExecutor<'e>,
) -> AppResult<HashMap<Committee, Vec<PersonRef>>> {
    let rows = sqlx::query_as::<_, DirectorRow>(
        "SELECT r.committee, u.id, u.andrew_id, u.name
         FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE r.role = 'director' AND r.committee IS NOT NULL
         ORDER BY u.name, u.andrew_id",
    )
    .fetch_all(db)
    .await?;
    let mut by_committee: HashMap<Committee, Vec<PersonRef>> = HashMap::new();
    for row in rows {
        by_committee
            .entry(row.committee)
            .or_default()
            .push(PersonRef::new(row.id, row.andrew_id, row.name));
    }
    Ok(by_committee)
}

/// GET /api/committee-pages/:slug: everything the editor needs. Read only unless `canEdit`.
pub(super) async fn show(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
) -> AppResult<Json<EditorResponse>> {
    let committee = committee_from_slug(&slug)?;
    let mut conn = state.db.acquire().await?;
    Ok(Json(editor_response(&mut conn, &viewer, committee).await?))
}

async fn editor_response(
    conn: &mut PgConnection,
    viewer: &Viewer,
    committee: Committee,
) -> AppResult<EditorResponse> {
    let row = one_page(&mut *conn, committee).await?;
    let projects = published_projects(&mut *conn, committee).await?;
    Ok(EditorResponse {
        committee: row.committee_ref(),
        status: row.status(),
        lock: lock_of(&row, viewer),
        draft_version: row.draft_version,
        can_edit: permissions::can_edit_page(viewer, committee),
        draft: row.draft.0,
        published: row.published.map(|json| json.0),
        projects,
    })
}

fn lock_of(row: &PageRow, viewer: &Viewer) -> Option<EditLock> {
    let holder = row.lock_holder()?;
    Some(EditLock {
        mine: holder.id == viewer.user_id,
        since: row.lock_since.unwrap_or(row.draft_updated_at),
        holder,
    })
}

/// GET /api/committee-pages/:slug/changes: what publishing would change, in plain sentences.
pub(super) async fn changes(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
) -> AppResult<Json<ChangesResponse>> {
    let committee = committee_from_slug(&slug)?;
    require_editor(&viewer, committee)?;
    let mut conn = state.db.acquire().await?;
    let row = one_page(&mut conn, committee).await?;
    let changes =
        blocks::describe_changes(row.published.as_ref().map(|json| &json.0), &row.draft.0);
    Ok(Json(ChangesResponse { changes }))
}

fn require_editor(viewer: &Viewer, committee: Committee) -> AppResult<()> {
    if permissions::can_edit_page(viewer, committee) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Only the director of {} or an admin can edit this page.",
            committee.name()
        )))
    }
}

// ---------------------------------------------------------------------------
// The edit lock

/// Takes the edit lock, or refreshes it when the viewer already holds it. Fails with 409 naming the holder while someone else holds a fresh lock. Call inside a transaction, after locking the row with [`locked_page`].
async fn claim_lock(conn: &mut PgConnection, row: &PageRow, viewer: &Viewer) -> AppResult<()> {
    match row.lock_holder() {
        Some(holder) if holder.id != viewer.user_id => Err(AppError::Conflict(format!(
            "{} is editing this page right now. You can look at it, and edit it once they are done.",
            holder.name
        ))),
        Some(_) => {
            sqlx::query(
                "UPDATE committee_pages SET lock_heartbeat_at = now() WHERE committee = $1",
            )
            .bind(row.committee)
            .execute(conn)
            .await?;
            Ok(())
        }
        None => {
            sqlx::query(
                "UPDATE committee_pages
                 SET lock_user_id = $2, lock_since = now(), lock_heartbeat_at = now()
                 WHERE committee = $1",
            )
            .bind(row.committee)
            .bind(viewer.user_id)
            .execute(conn)
            .await?;
            Ok(())
        }
    }
}

/// The page, with its row locked until the transaction ends so writes happen one at a time.
async fn locked_page(conn: &mut PgConnection, committee: Committee) -> AppResult<PageRow> {
    sqlx::query("SELECT 1 FROM committee_pages WHERE committee = $1 FOR UPDATE")
        .bind(committee)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| AppError::not_found("committee page"))?;
    one_page(conn, committee).await
}

/// POST /api/committee-pages/:slug/lock: take or refresh the five minute edit lock.
pub(super) async fn lock(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
) -> AppResult<Json<EditLock>> {
    let committee = committee_from_slug(&slug)?;
    require_editor(&viewer, committee)?;
    let mut tx = state.db.begin().await?;
    let row = locked_page(&mut tx, committee).await?;
    claim_lock(&mut tx, &row, &viewer).await?;
    let row = one_page(&mut tx, committee).await?;
    tx.commit().await?;
    lock_of(&row, &viewer)
        .map(Json)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("the edit lock was not taken")))
}

/// DELETE /api/committee-pages/:slug/lock: let go of the lock, if the viewer holds it.
pub(super) async fn unlock(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
) -> AppResult<StatusCode> {
    let committee = committee_from_slug(&slug)?;
    sqlx::query(
        "UPDATE committee_pages
         SET lock_user_id = NULL, lock_since = NULL, lock_heartbeat_at = NULL
         WHERE committee = $1 AND lock_user_id = $2",
    )
    .bind(committee)
    .bind(viewer.user_id)
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Writing

/// Draft saves are recorded in the audit log at most this often per person and page.
const DRAFT_AUDIT_MINUTES: i32 = 30;

/// PUT /api/committee-pages/:slug/draft: save the draft. 409 when `baseVersion` is not the latest version or someone else holds the lock.
pub(super) async fn save_draft(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
    JsonBody(input): JsonBody<SaveDraftInput>,
) -> AppResult<Json<SaveDraftResponse>> {
    let committee = committee_from_slug(&slug)?;
    require_editor(&viewer, committee)?;
    let mut doc = blocks::clean_draft(input.doc)?;
    let mut tx = state.db.begin().await?;
    check_media(&mut tx, &mut doc).await?;
    let row = locked_page(&mut tx, committee).await?;
    claim_lock(&mut tx, &row, &viewer).await?;
    if input.base_version != row.draft_version {
        return Err(AppError::Conflict(
            "This page was saved somewhere else after you opened it. Reload to get the latest version, then make your change again."
                .into(),
        ));
    }
    let saved = sqlx::query_as::<_, SavedRow>(
        "UPDATE committee_pages
         SET draft = $2, draft_version = draft_version + 1,
             draft_updated_at = now(), draft_updated_by = $3
         WHERE committee = $1
         RETURNING draft_version, draft_updated_at, published IS NULL AS never_published,
                   coalesce(published = draft, false) AS in_sync",
    )
    .bind(committee)
    .bind(sqlx::types::Json(&doc))
    .bind(viewer.user_id)
    .fetch_one(&mut *tx)
    .await?;
    record_draft_edit(&mut tx, &viewer, committee).await?;
    tx.commit().await?;
    Ok(Json(SaveDraftResponse {
        draft_version: saved.draft_version,
        saved_at: saved.draft_updated_at,
        status: saved.status(),
    }))
}

#[derive(FromRow)]
struct SavedRow {
    draft_version: i32,
    draft_updated_at: DateTime<Utc>,
    never_published: bool,
    in_sync: bool,
}

impl SavedRow {
    fn status(&self) -> PageStatus {
        if self.never_published {
            PageStatus::NeverPublished
        } else if self.in_sync {
            PageStatus::Published
        } else {
            PageStatus::UnpublishedChanges
        }
    }
}

/// Autosave runs every few seconds, so the log gets one "edited" entry per person and page every half hour, not one per save.
async fn record_draft_edit(
    conn: &mut PgConnection,
    viewer: &Viewer,
    committee: Committee,
) -> AppResult<()> {
    let recent: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM audit_log
             WHERE actor_id = $1 AND subject_type = $2 AND subject_id = $3
               AND action = 'committee_page.edit_draft'
               AND created_at > now() - make_interval(mins => $4))",
    )
    .bind(viewer.user_id)
    .bind(subject::COMMITTEE_PAGE)
    .bind(committee.slug())
    .bind(DRAFT_AUDIT_MINUTES)
    .fetch_one(&mut *conn)
    .await?;
    if recent {
        return Ok(());
    }
    audit::record(
        conn,
        audit_entry(
            viewer,
            committee,
            "committee_page.edit_draft",
            format!("Edited the draft of the {} page", committee.name()),
        ),
    )
    .await
}

fn audit_entry<'a>(
    viewer: &Viewer,
    committee: Committee,
    action: &'a str,
    summary: String,
) -> audit::Entry<'a> {
    audit::Entry {
        actor: Some(viewer.user_id),
        action,
        subject_type: subject::COMMITTEE_PAGE,
        subject_id: Some(committee.slug().to_owned()),
        committee: Some(committee),
        summary,
    }
}

/// Uploaded images must exist. Their stored sizes replace whatever the browser sent, so pages reserve the right space.
async fn check_media(conn: &mut PgConnection, doc: &mut CommitteePageDoc) -> AppResult<()> {
    let ids = doc.media_ids();
    if ids.is_empty() {
        return Ok(());
    }
    let sizes: HashMap<Uuid, (i32, i32)> = sqlx::query_as::<_, (Uuid, i32, i32)>(
        "SELECT id, width, height FROM media WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(id, width, height)| (id, (width, height)))
    .collect();
    let mut problems = Problems::new();
    for (field, image) in doc.images() {
        if image.media_id().is_some_and(|id| !sizes.contains_key(&id)) {
            problems.add(
                field,
                "That photo is no longer available. Upload it again or choose another one.",
            );
        }
    }
    problems.finish()?;
    for image in doc.images_mut() {
        if let Some((width, height)) = image.media_id().and_then(|id| sizes.get(&id)) {
            image.width = u32::try_from(*width).ok();
            image.height = u32::try_from(*height).ok();
        }
    }
    Ok(())
}

/// POST /api/committee-pages/:slug/publish: make the draft live. `draftVersion` must be the version the editor showed, so nothing unseen goes out.
pub(super) async fn publish(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
    JsonBody(input): JsonBody<PublishInput>,
) -> AppResult<Json<EditorResponse>> {
    let committee = committee_from_slug(&slug)?;
    require_editor(&viewer, committee)?;
    let mut tx = state.db.begin().await?;
    let row = locked_page(&mut tx, committee).await?;
    claim_lock(&mut tx, &row, &viewer).await?;
    if input.draft_version != row.draft_version {
        return Err(AppError::Conflict(
            "The draft changed after you opened Publish. Check the changes and publish again."
                .into(),
        ));
    }
    let mut doc = blocks::clean_draft(row.draft.0.clone())?;
    check_media(&mut tx, &mut doc).await?;
    blocks::check_for_publishing(&doc)?;
    sqlx::query(
        "UPDATE committee_pages
         SET published = draft, published_at = now(), published_by = $2
         WHERE committee = $1",
    )
    .bind(committee)
    .bind(viewer.user_id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        audit_entry(
            &viewer,
            committee,
            "committee_page.publish",
            format!("Published the {} page", committee.name()),
        ),
    )
    .await?;
    let response = editor_response(&mut tx, &viewer, committee).await?;
    tx.commit().await?;
    Ok(Json(response))
}

/// POST /api/committee-pages/:slug/discard: throw the draft away; it becomes the published version again.
pub(super) async fn discard(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(slug): Path<String>,
) -> AppResult<Json<EditorResponse>> {
    let committee = committee_from_slug(&slug)?;
    require_editor(&viewer, committee)?;
    let mut tx = state.db.begin().await?;
    let row = locked_page(&mut tx, committee).await?;
    claim_lock(&mut tx, &row, &viewer).await?;
    if row.published.is_none() {
        return Err(AppError::Conflict(
            "This page has not been published yet, so there is no published version to go back to."
                .into(),
        ));
    }
    sqlx::query(
        "UPDATE committee_pages
         SET draft = published, draft_version = draft_version + 1,
             draft_updated_at = now(), draft_updated_by = $2
         WHERE committee = $1",
    )
    .bind(committee)
    .bind(viewer.user_id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        audit_entry(
            &viewer,
            committee,
            "committee_page.discard",
            format!(
                "Discarded the unpublished changes to the {} page",
                committee.name()
            ),
        ),
    )
    .await?;
    let response = editor_response(&mut tx, &viewer, committee).await?;
    tx.commit().await?;
    Ok(Json(response))
}
