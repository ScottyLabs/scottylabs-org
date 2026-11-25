//! The portal's projects: the list, adding a project, the editor and every write it makes.
//!
//! Who may do what comes from `crate::permissions` (`project_access`, `can_view_project`, `can_publish_projects_for`, `can_curate_projects`). Every write is audited in its transaction and answers with the editor's view of the project.

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as JsonColumn;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::doc::{self, Checklist, ProjectDoc, ProjectState, Purpose};
use crate::approvals_core::{self, ApprovalStatus, SubjectType};
use crate::audit::{self, subject};
use crate::auth::{PortalViewer, Viewer};
use crate::error::{AppError, AppResult};
use crate::json::Json as JsonBody;
use crate::models::{Committee, ImageRef, PersonRef, check_line};
use crate::permissions::{self, ProjectAccess};
use crate::routes::common::{self, blocking, query_param};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list).post(create))
        .route("/projects/{id}", get(show))
        .route("/projects/{id}/draft", put(save_draft))
        .route("/projects/{id}/publish", post(publish))
        .route("/projects/{id}/archive", post(archive))
        .route("/projects/{id}/unarchive", post(unarchive))
        .route("/projects/{id}/discard", post(discard))
        .route("/projects/{id}/settings", patch(settings))
        .route("/projects/{id}/editors", put(set_editors))
}

// ---------------------------------------------------------------------------
// Reading projects

/// A project as stored, locked or not.
#[derive(FromRow)]
pub(super) struct ProjectRow {
    pub id: Uuid,
    pub slug: String,
    pub committee: Committee,
    pub state: ProjectState,
    pub draft: JsonColumn<ProjectDoc>,
    pub published: Option<JsonColumn<ProjectDoc>>,
    pub featured: bool,
    pub published_at: Option<DateTime<Utc>>,
}

impl ProjectRow {
    fn has_unpublished_changes(&self) -> bool {
        self.published
            .as_ref()
            .is_some_and(|published| published.0 != self.draft.0)
    }
}

const ROW_COLUMNS: &str = "id, slug, committee, state, draft, published, featured, published_at";

/// Locks a project for a write.
pub(super) async fn lock(conn: &mut PgConnection, id: Uuid) -> AppResult<ProjectRow> {
    sqlx::query_as::<_, ProjectRow>(&format!(
        "SELECT {ROW_COLUMNS} FROM projects WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| AppError::not_found("project"))
}

/// Project ids come from the path; anything that is not one is a project we don't have.
fn parse_id(raw: &str) -> AppResult<Uuid> {
    common::parse_id(raw, "project")
}

#[derive(FromRow)]
struct PersonRow {
    id: Uuid,
    andrew_id: String,
    name: String,
}

impl From<PersonRow> for PersonRef {
    fn from(row: PersonRow) -> Self {
        PersonRef::new(row.id, row.andrew_id, row.name)
    }
}

fn person(id: Option<Uuid>, andrew_id: Option<String>, name: Option<String>) -> Option<PersonRef> {
    Some(PersonRef::new(id?, andrew_id?, name.unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// GET /api/projects

/// One row of the portal list. Mirrors `ProjectAdminSummary` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAdminSummary {
    id: Uuid,
    slug: String,
    name: String,
    committee: Committee,
    state: ProjectState,
    has_unpublished_changes: bool,
    pending_approval: bool,
    cover: Option<ImageRef>,
    has_repo: bool,
    has_site: bool,
    featured: bool,
    updated_at: DateTime<Utc>,
    updated_by: Option<PersonRef>,
    can_edit: bool,
    can_publish: bool,
}

#[derive(Serialize)]
struct ProjectListResponse {
    projects: Vec<ProjectAdminSummary>,
}

#[derive(FromRow)]
struct ListRow {
    #[sqlx(flatten)]
    project: ProjectRow,
    updated_at: DateTime<Utc>,
    updater_id: Option<Uuid>,
    updater_andrew_id: Option<String>,
    updater_name: Option<String>,
    pending: bool,
}

const MAX_QUERY_CHARS: usize = 64;

struct ListFilters {
    committee: Option<Committee>,
    state: Option<ProjectState>,
    search: Option<String>,
}

fn list_filters(query: Option<&str>) -> AppResult<ListFilters> {
    let committee =
        match query_param(query, "committee") {
            None => None,
            Some(slug) => Some(Committee::from_slug(&slug).ok_or_else(|| {
                AppError::field("committee", "Choose one of the seven committees.")
            })?),
        };
    let state = match query_param(query, "state") {
        None => None,
        Some(slug) => Some(ProjectState::from_slug(&slug).ok_or_else(|| {
            AppError::field("state", "Choose drafts, published or archived projects.")
        })?),
    };
    let search = query_param(query, "q").map(|q| {
        let q: String = q.chars().take(MAX_QUERY_CHARS).collect();
        format!("%{}%", super::super::people::escape_like(&q.to_lowercase()))
    });
    Ok(ListFilters {
        committee,
        state,
        search,
    })
}

async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(query): RawQuery,
) -> AppResult<Json<ProjectListResponse>> {
    let filters = list_filters(query.as_deref())?;
    let rows = sqlx::query_as::<_, ListRow>(&format!(
        "SELECT p.{}, p.updated_at,
                u.id AS updater_id, u.andrew_id AS updater_andrew_id, u.name AS updater_name,
                EXISTS (
                    SELECT 1 FROM approvals a
                    WHERE a.subject_type = 'project' AND a.subject_id = p.id AND a.status = 'waiting'
                ) AS pending
         FROM projects p LEFT JOIN users u ON u.id = p.updated_by
         WHERE ($1::text IS NULL OR p.committee = $1)
           AND ($2::text IS NULL OR p.state = $2)
           AND ($3::text IS NULL OR lower(p.draft->>'name') LIKE $3 ESCAPE '\\' OR p.slug LIKE $3 ESCAPE '\\')
         ORDER BY p.updated_at DESC, p.id",
        ROW_COLUMNS.replace(", ", ", p.")
    ))
    .bind(filters.committee)
    .bind(filters.state)
    .bind(filters.search)
    .fetch_all(&state.db)
    .await?;

    let projects = rows
        .into_iter()
        .filter(|row| permissions::can_view_project(&viewer, row.project.id))
        .map(|row| summary_of(&viewer, row))
        .collect();
    Ok(Json(ProjectListResponse { projects }))
}

fn summary_of(viewer: &Viewer, row: ListRow) -> ProjectAdminSummary {
    let access = permissions::project_access(viewer, row.project.committee, row.project.id);
    let has_unpublished_changes = row.project.has_unpublished_changes();
    let ProjectRow {
        id,
        slug,
        committee,
        state,
        draft,
        featured,
        ..
    } = row.project;
    let draft = draft.0;
    ProjectAdminSummary {
        id,
        slug,
        name: draft.name.clone(),
        committee,
        state,
        has_unpublished_changes,
        pending_approval: row.pending,
        cover: draft.cover().cloned(),
        has_repo: draft.repo_url.is_some(),
        has_site: draft.site_url.is_some(),
        featured,
        updated_at: row.updated_at,
        updated_by: person(row.updater_id, row.updater_andrew_id, row.updater_name),
        can_edit: access != ProjectAccess::None,
        can_publish: access == ProjectAccess::Full,
    }
}

// ---------------------------------------------------------------------------
// The editor's view: GET /api/projects/:id, and the answer to every write

/// Mirrors `ApprovalRef` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalRef {
    id: Uuid,
    status: ApprovalStatus,
    note: Option<String>,
    decided_by: Option<PersonRef>,
    decided_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorProject {
    id: Uuid,
    slug: String,
    state: ProjectState,
    featured: bool,
    draft: ProjectDoc,
    published: Option<ProjectDoc>,
    has_unpublished_changes: bool,
    /// The lead's submission while it waits, or the note it came back with.
    pending_approval: Option<ApprovalRef>,
    editors: Vec<PersonRef>,
    /// Directors of the owning committee, who approve what leads send.
    approvers: Vec<PersonRef>,
    checklist: Checklist,
    updated_at: DateTime<Utc>,
    can_edit: bool,
    can_publish: bool,
    can_archive: bool,
    can_manage_editors: bool,
}

/// Mirrors `ProjectEditorResponse` in types.ts.
#[derive(Serialize)]
pub(super) struct ProjectEditorResponse {
    project: EditorProject,
}

pub(super) async fn editor_view(
    conn: &mut PgConnection,
    viewer: &Viewer,
    id: Uuid,
) -> AppResult<ProjectEditorResponse> {
    let row = sqlx::query_as::<_, ListRow>(&format!(
        "SELECT p.{}, p.updated_at,
                NULL::uuid AS updater_id, NULL::text AS updater_andrew_id, NULL::text AS updater_name,
                false AS pending
         FROM projects p WHERE p.id = $1",
        ROW_COLUMNS.replace(", ", ", p.")
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::not_found("project"))?;
    let editors = editors_of(&mut *conn, id, row.project.committee).await?;
    let approvers = directors_of(&mut *conn, row.project.committee).await?;
    let pending_approval = approval_to_show(&mut *conn, &row.project).await?;

    let access = permissions::project_access(viewer, row.project.committee, id);
    let has_unpublished_changes = row.project.has_unpublished_changes();
    let project = row.project;
    Ok(ProjectEditorResponse {
        project: EditorProject {
            checklist: Checklist::of(&project.draft),
            id,
            slug: project.slug,
            state: project.state,
            featured: project.featured,
            draft: project.draft.0,
            published: project.published.map(|doc| doc.0),
            has_unpublished_changes,
            pending_approval,
            editors,
            approvers,
            updated_at: row.updated_at,
            can_edit: access != ProjectAccess::None,
            can_publish: access == ProjectAccess::Full,
            can_archive: access == ProjectAccess::Full,
            can_manage_editors: permissions::can_publish_projects_for(viewer, project.committee),
        },
    })
}

/// Leads listed as editors who are still leads of the owning committee, the ones who can edit.
async fn editors_of(
    conn: &mut PgConnection,
    id: Uuid,
    committee: Committee,
) -> AppResult<Vec<PersonRef>> {
    let rows = sqlx::query_as::<_, PersonRow>(
        "SELECT u.id, u.andrew_id, u.name FROM project_editors pe JOIN users u ON u.id = pe.user_id
         WHERE pe.project_id = $1
           AND EXISTS (
               SELECT 1 FROM role_assignments r
               WHERE r.user_id = u.id AND r.role = 'lead' AND r.committee = $2
           )
         ORDER BY lower(coalesce(nullif(u.name, ''), u.andrew_id)), u.andrew_id",
    )
    .bind(id)
    .bind(committee)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(PersonRef::from).collect())
}

async fn directors_of(conn: &mut PgConnection, committee: Committee) -> AppResult<Vec<PersonRef>> {
    let rows = sqlx::query_as::<_, PersonRow>(
        "SELECT DISTINCT u.id, u.andrew_id, u.name FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE r.role = 'director' AND r.committee = $1
         ORDER BY u.name, u.andrew_id",
    )
    .bind(committee)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(PersonRef::from).collect())
}

/// A waiting submission, or one sent back since the project was last published.
async fn approval_to_show(
    conn: &mut PgConnection,
    project: &ProjectRow,
) -> AppResult<Option<ApprovalRef>> {
    let Some(approval) =
        approvals_core::latest(&mut *conn, SubjectType::Project, project.id).await?
    else {
        return Ok(None);
    };
    let show = match approval.status {
        ApprovalStatus::Waiting => true,
        ApprovalStatus::SentBack => match (approval.decided_at, project.published_at) {
            (Some(decided), Some(published)) => decided > published,
            _ => true,
        },
        ApprovalStatus::Approved | ApprovalStatus::Withdrawn => false,
    };
    if !show {
        return Ok(None);
    }
    let decided_by = match approval.decided_by {
        None => None,
        Some(user_id) => {
            sqlx::query_as::<_, PersonRow>("SELECT id, andrew_id, name FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&mut *conn)
                .await?
                .map(PersonRef::from)
        }
    };
    Ok(Some(ApprovalRef {
        id: approval.id,
        status: approval.status,
        note: approval.note,
        decided_by,
        decided_at: approval.decided_at,
    }))
}

async fn show(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    let mut conn = state.db.acquire().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1)")
        .bind(id)
        .fetch_one(&mut *conn)
        .await?;
    if !exists {
        return Err(AppError::not_found("project"));
    }
    if !permissions::can_view_project(&viewer, id) {
        return Err(AppError::Forbidden(
            "You can't open this project. Ask the director of its committee to add you as an editor."
                .into(),
        ));
    }
    Ok(Json(editor_view(&mut conn, &viewer, id).await?))
}

// ---------------------------------------------------------------------------
// Shared by the writes

async fn record(
    conn: &mut PgConnection,
    viewer: &Viewer,
    action: &str,
    project: Uuid,
    committee: Committee,
    summary: String,
) -> AppResult<()> {
    audit::record(
        conn,
        audit::Entry {
            actor: Some(viewer.user_id),
            action,
            subject_type: subject::PROJECT,
            subject_id: Some(project.to_string()),
            committee: Some(committee),
            summary,
        },
    )
    .await
}

const EDIT_ACTION: &str = "project.edit";

/// Autosave writes often, so a run of edits by one person shows as one entry: the latest edit moves that entry forward instead of adding another.
async fn record_edit(
    conn: &mut PgConnection,
    viewer: &Viewer,
    project: Uuid,
    committee: Committee,
    name: &str,
) -> AppResult<()> {
    let refreshed = sqlx::query(
        "UPDATE audit_log SET created_at = now(), summary = $4, committee = $5
         WHERE id = (
             SELECT id FROM audit_log
             WHERE subject_type = $1 AND subject_id = $2
             ORDER BY created_at DESC, id DESC LIMIT 1
         )
           AND actor_id = $3 AND action = $6 AND created_at > now() - interval '15 minutes'",
    )
    .bind(subject::PROJECT)
    .bind(project.to_string())
    .bind(viewer.user_id)
    .bind(format!("Edited {name}"))
    .bind(committee)
    .bind(EDIT_ACTION)
    .execute(&mut *conn)
    .await?;
    if refreshed.rows_affected() > 0 {
        return Ok(());
    }
    record(
        conn,
        viewer,
        EDIT_ACTION,
        project,
        committee,
        format!("Edited {name}"),
    )
    .await
}

/// The viewer's access to a project they are about to change.
fn require_access(viewer: &Viewer, project: &ProjectRow) -> AppResult<ProjectAccess> {
    match permissions::project_access(viewer, project.committee, project.id) {
        ProjectAccess::None => Err(AppError::Forbidden(format!(
            "You can't edit {}. Ask the director of {} to add you as an editor.",
            project.draft.name,
            project.committee.name()
        ))),
        access => Ok(access),
    }
}

/// Archiving, publishing directly and changing editors are for admins and the owning committee's directors.
fn require_full(viewer: &Viewer, project: &ProjectRow, what: &str) -> AppResult<()> {
    if permissions::project_access(viewer, project.committee, project.id) == ProjectAccess::Full {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can {what}.",
            project.committee.name()
        )))
    }
}

async fn finish(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    viewer: &Viewer,
    id: Uuid,
) -> AppResult<Json<ProjectEditorResponse>> {
    let view = editor_view(&mut tx, viewer, id).await?;
    tx.commit().await?;
    Ok(Json(view))
}

// ---------------------------------------------------------------------------
// POST /api/projects

#[derive(Deserialize)]
struct CreateInput {
    name: String,
    committee: Committee,
}

const MAX_SLUG_TRIES: usize = 100;

async fn create(
    State(state): State<AppState>,
    viewer: PortalViewer,
    JsonBody(input): JsonBody<CreateInput>,
) -> AppResult<Response> {
    if !permissions::can_publish_projects_for(&viewer, input.committee) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can add projects for {}.",
            input.committee.name(),
            input.committee.name()
        )));
    }
    let name = check_line(&input.name, 1, doc::MAX_NAME_CHARS, "the project name")
        .map_err(|message| AppError::field("name", message))?;
    let draft = ProjectDoc::new(name.clone(), input.committee);
    let base = doc::slugify(&name);
    let id = Uuid::now_v7();

    let mut tx = state.db.begin().await?;
    let mut created = false;
    for attempt in 1..=MAX_SLUG_TRIES {
        let slug = if attempt == 1 {
            base.clone()
        } else {
            format!("{base}-{attempt}")
        };
        // Old addresses keep redirecting to their project, so a new project never takes one.
        let inserted = sqlx::query(
            "INSERT INTO projects (id, slug, committee, state, draft, created_by, updated_by)
             SELECT $1, $2, $3, 'draft', $4, $5, $5
             WHERE NOT EXISTS (SELECT 1 FROM project_slug_redirects WHERE old_slug = $2)
             ON CONFLICT (slug) DO NOTHING",
        )
        .bind(id)
        .bind(&slug)
        .bind(input.committee)
        .bind(JsonColumn(&draft))
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 1 {
            created = true;
            break;
        }
    }
    if !created {
        return Err(AppError::Conflict(
            "Too many projects already have that name. Choose a different name.".into(),
        ));
    }
    record(
        &mut tx,
        &viewer,
        "project.create",
        id,
        input.committee,
        format!("Added the project {name}"),
    )
    .await?;
    let view = finish(tx, &viewer, id).await?;
    Ok((StatusCode::CREATED, view).into_response())
}

// ---------------------------------------------------------------------------
// PUT /api/projects/:id/draft

#[derive(Deserialize)]
struct SaveDraftInput {
    doc: ProjectDoc,
}

async fn save_draft(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<SaveDraftInput>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    // Sanitizing takes CPU time, so it happens before the project is locked and off the async runtime. Its problems are reported after the access checks.
    let cleaned = blocking(move || doc::Cleaned::of(input.doc)).await?;
    let target = cleaned.committee();
    let mut tx = state.db.begin().await?;
    let project = lock(&mut tx, id).await?;
    let access = require_access(&viewer, &project)?;
    let moving = target != project.committee;
    if moving && !permissions::can_publish_projects_for(&viewer, target) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can move a project to {}.",
            target.name(),
            target.name()
        )));
    }
    let clean = doc::finish_validation(&mut tx, cleaned, Purpose::Save).await?;
    if clean != project.draft.0 {
        sqlx::query(
            "UPDATE projects SET draft = $2, committee = $3, updated_at = now(), updated_by = $4
             WHERE id = $1",
        )
        .bind(id)
        .bind(JsonColumn(&clean))
        .bind(clean.committee)
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
        if access == ProjectAccess::Lead {
            // What the director would approve is no longer what the lead is working on.
            approvals_core::withdraw_waiting(&mut tx, SubjectType::Project, id).await?;
        }
        if moving {
            record(
                &mut tx,
                &viewer,
                "project.move",
                id,
                clean.committee,
                format!("Moved {} to {}", clean.name, clean.committee.name()),
            )
            .await?;
        } else {
            record_edit(&mut tx, &viewer, id, clean.committee, &clean.name).await?;
        }
    }
    finish(tx, &viewer, id).await
}

// ---------------------------------------------------------------------------
// POST /api/projects/:id/publish

async fn publish(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let project = lock(&mut tx, id).await?;
    let access = require_access(&viewer, &project)?;
    if project.state == ProjectState::Archived {
        return Err(AppError::Conflict(format!(
            "{} is archived. Unarchive it before publishing.",
            project.draft.name
        )));
    }
    let clean = doc::validate(&mut tx, project.draft.0.clone(), Purpose::Publish).await?;
    let unchanged = project.state == ProjectState::Published
        && project
            .published
            .as_ref()
            .is_some_and(|published| published.0 == clean);

    if access == ProjectAccess::Full {
        if !unchanged {
            publish_doc(&mut tx, id, &clean, viewer.user_id).await?;
            approvals_core::withdraw_waiting(&mut tx, SubjectType::Project, id).await?;
            record(
                &mut tx,
                &viewer,
                "project.publish",
                id,
                clean.committee,
                format!("Published {}", clean.name),
            )
            .await?;
        }
        return finish(tx, &viewer, id).await;
    }

    if unchanged {
        return Err(AppError::Conflict(
            "There are no changes to send. Edit the project, then send it for approval.".into(),
        ));
    }
    sqlx::query("UPDATE projects SET draft = $2 WHERE id = $1")
        .bind(id)
        .bind(JsonColumn(&clean))
        .execute(&mut *tx)
        .await?;
    approvals_core::submit(
        &mut tx,
        SubjectType::Project,
        id,
        clean.committee,
        viewer.user_id,
        &serde_json::to_value(&clean)?,
    )
    .await?;
    record(
        &mut tx,
        &viewer,
        "project.submit",
        id,
        clean.committee,
        format!("Sent {} for approval", clean.name),
    )
    .await?;
    finish(tx, &viewer, id).await
}

/// Makes `doc` the published version. The draft becomes the same doc.
pub(super) async fn publish_doc(
    conn: &mut PgConnection,
    id: Uuid,
    doc: &ProjectDoc,
    by: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE projects
         SET published = $2, draft = $2, state = 'published', published_at = now(), published_by = $3,
             updated_at = now(), updated_by = $3
         WHERE id = $1",
    )
    .bind(id)
    .bind(JsonColumn(doc))
    .bind(by)
    .execute(conn)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Archive, unarchive and discard

async fn archive(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let project = lock(&mut tx, id).await?;
    require_full(&viewer, &project, "archive projects")?;
    if project.state != ProjectState::Archived {
        set_state(&mut tx, id, ProjectState::Archived, viewer.user_id).await?;
        approvals_core::withdraw_waiting(&mut tx, SubjectType::Project, id).await?;
        record(
            &mut tx,
            &viewer,
            "project.archive",
            id,
            project.committee,
            format!("Archived {}", project.draft.name),
        )
        .await?;
    }
    finish(tx, &viewer, id).await
}

async fn unarchive(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let project = lock(&mut tx, id).await?;
    require_full(&viewer, &project, "unarchive projects")?;
    if project.state == ProjectState::Archived {
        let state = if project.published.is_some() {
            ProjectState::Published
        } else {
            ProjectState::Draft
        };
        set_state(&mut tx, id, state, viewer.user_id).await?;
        record(
            &mut tx,
            &viewer,
            "project.unarchive",
            id,
            project.committee,
            format!("Unarchived {}", project.draft.name),
        )
        .await?;
    }
    finish(tx, &viewer, id).await
}

async fn set_state(
    conn: &mut PgConnection,
    id: Uuid,
    state: ProjectState,
    by: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE projects SET state = $2, updated_at = now(), updated_by = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(state)
    .bind(by)
    .execute(conn)
    .await?;
    Ok(())
}

async fn discard(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let project = lock(&mut tx, id).await?;
    let access = require_access(&viewer, &project)?;
    let Some(published) = project.published.as_ref().map(|doc| doc.0.clone()) else {
        return Err(AppError::Conflict(format!(
            "{} has never been published, so there is no published version to go back to.",
            project.draft.name
        )));
    };
    if published != project.draft.0 {
        require_move_back(&viewer, &project, &published)?;
        sqlx::query(
            "UPDATE projects SET draft = $2, committee = $3, updated_at = now(), updated_by = $4
             WHERE id = $1",
        )
        .bind(id)
        .bind(JsonColumn(&published))
        .bind(published.committee)
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
        if access == ProjectAccess::Lead {
            approvals_core::withdraw_waiting(&mut tx, SubjectType::Project, id).await?;
        }
        record(
            &mut tx,
            &viewer,
            "project.discard",
            id,
            published.committee,
            format!("Discarded the unpublished changes to {}", published.name),
        )
        .await?;
    }
    finish(tx, &viewer, id).await
}

/// Discarding an unpublished move puts the project back in its published committee, which is a move like any other: only for those who can publish there.
fn require_move_back(
    viewer: &Viewer,
    project: &ProjectRow,
    published: &ProjectDoc,
) -> AppResult<()> {
    let back_to = published.committee;
    if back_to == project.committee || permissions::can_publish_projects_for(viewer, back_to) {
        return Ok(());
    }
    let committee = back_to.name();
    Err(AppError::Forbidden(format!(
        "Discarding would move {} back to {committee}, and only admins and the director of {committee} can move a project there.",
        published.name
    )))
}

// ---------------------------------------------------------------------------
// PATCH /api/projects/:id/settings

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsInput {
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    featured: Option<bool>,
}

async fn settings(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<SettingsInput>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    if !permissions::can_curate_projects(&viewer) {
        return Err(AppError::Forbidden(
            "Only admins can change a project's address or feature it.".into(),
        ));
    }
    let new_slug = match input.slug.as_deref() {
        None => None,
        Some(raw) => {
            Some(doc::check_slug(raw).map_err(|message| AppError::field("slug", message))?)
        }
    };
    let mut tx = state.db.begin().await?;
    if input.featured == Some(true) {
        wait_to_feature(&mut tx).await?;
    }
    let project = lock(&mut tx, id).await?;
    let name = project.draft.name.clone();
    if let Some(slug) = new_slug.filter(|slug| *slug != project.slug) {
        change_slug(&mut tx, &project, &slug).await?;
        record(
            &mut tx,
            &viewer,
            "project.slug",
            id,
            project.committee,
            format!("Moved {name} to /projects/{slug}/"),
        )
        .await?;
    }
    if let Some(featured) = input
        .featured
        .filter(|featured| *featured != project.featured)
    {
        set_featured(&mut tx, id, featured).await?;
        let summary = if featured {
            format!("Featured {name} on the Projects page")
        } else {
            format!("Stopped featuring {name}")
        };
        record(
            &mut tx,
            &viewer,
            "project.feature",
            id,
            project.committee,
            summary,
        )
        .await?;
    }
    finish(tx, &viewer, id).await
}

/// Renames the project's address. The old one keeps working through a redirect.
async fn change_slug(conn: &mut PgConnection, project: &ProjectRow, slug: &str) -> AppResult<()> {
    let taken: Option<String> =
        sqlx::query_scalar("SELECT draft->>'name' FROM projects WHERE slug = $1 AND id <> $2")
            .bind(slug)
            .bind(project.id)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some(other) = taken {
        return Err(AppError::field(
            "slug",
            format!("{other} already uses /projects/{slug}/. Choose another address."),
        ));
    }
    let redirected_from_other: Option<String> = sqlx::query_scalar(
        "SELECT p.draft->>'name' FROM project_slug_redirects r JOIN projects p ON p.id = r.project_id
         WHERE r.old_slug = $1 AND r.project_id <> $2",
    )
    .bind(slug)
    .bind(project.id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(other) = redirected_from_other {
        return Err(AppError::field(
            "slug",
            format!(
                "/projects/{slug}/ used to be {other}\u{2019}s address and still leads there. Choose another address."
            ),
        ));
    }
    // Going back to an old address: it is the project's own again, not a redirect.
    sqlx::query("DELETE FROM project_slug_redirects WHERE old_slug = $1")
        .bind(slug)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO project_slug_redirects (old_slug, project_id) VALUES ($1, $2)
         ON CONFLICT (old_slug) DO UPDATE SET project_id = EXCLUDED.project_id",
    )
    .bind(&project.slug)
    .bind(project.id)
    .execute(&mut *conn)
    .await?;
    sqlx::query("UPDATE projects SET slug = $2 WHERE id = $1")
        .bind(project.id)
        .bind(slug)
        .execute(conn)
        .await?;
    Ok(())
}

/// Two admins featuring different projects at once take turns; otherwise neither would see the other's choice to clear it. Held until the transaction ends, and taken before any project row is locked.
async fn wait_to_feature(conn: &mut PgConnection) -> AppResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('projects.featured'))")
        .execute(conn)
        .await?;
    Ok(())
}

/// One project is featured at a time; featuring one stops featuring the others. Call `wait_to_feature` first. A unique index (migration 0201) backs this up.
async fn set_featured(conn: &mut PgConnection, id: Uuid, featured: bool) -> AppResult<()> {
    if featured {
        sqlx::query("UPDATE projects SET featured = false WHERE featured AND id <> $1")
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    sqlx::query("UPDATE projects SET featured = $2 WHERE id = $1")
        .bind(id)
        .bind(featured)
        .execute(conn)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// PUT /api/projects/:id/editors

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditorsInput {
    user_ids: Vec<Uuid>,
}

const MAX_EDITORS: usize = 50;

async fn set_editors(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<EditorsInput>,
) -> AppResult<Json<ProjectEditorResponse>> {
    let id = parse_id(&id)?;
    let mut user_ids = input.user_ids;
    user_ids.sort_unstable();
    user_ids.dedup();
    if user_ids.len() > MAX_EDITORS {
        return Err(AppError::field(
            "userIds",
            format!("A project can have up to {MAX_EDITORS} editors."),
        ));
    }
    let mut tx = state.db.begin().await?;
    let project = lock(&mut tx, id).await?;
    if !permissions::can_publish_projects_for(&viewer, project.committee) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can choose who edits {}.",
            project.committee.name(),
            project.draft.name
        )));
    }
    let leads: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT user_id FROM role_assignments
         WHERE role = 'lead' AND committee = $1 AND user_id = ANY($2)",
    )
    .bind(project.committee)
    .bind(&user_ids)
    .fetch_all(&mut *tx)
    .await?;
    if leads.len() != user_ids.len() {
        let committee = project.committee.name();
        return Err(AppError::field(
            "userIds",
            format!(
                "Only leads in {committee} can edit {committee} projects. Add the person as a lead on the People page first."
            ),
        ));
    }
    let before: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM project_editors WHERE project_id = $1 ORDER BY user_id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    if before != user_ids {
        sqlx::query("DELETE FROM project_editors WHERE project_id = $1 AND user_id <> ALL($2)")
            .bind(id)
            .bind(&user_ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO project_editors (project_id, user_id, added_by)
             SELECT $1, user_id, $3 FROM unnest($2::uuid[]) AS user_id
             ON CONFLICT (project_id, user_id) DO NOTHING",
        )
        .bind(id)
        .bind(&user_ids)
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
        record(
            &mut tx,
            &viewer,
            "project.editors",
            id,
            project.committee,
            format!("Changed who can edit {}", project.draft.name),
        )
        .await?;
    }
    finish(tx, &viewer, id).await
}
