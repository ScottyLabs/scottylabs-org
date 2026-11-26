//! People and roles: listing, searching, adding and changing role assignments, and syncing directors and heads from governance.
//!
//! Who may change what is decided by `crate::permissions`. On top of it: nobody removes their own admin role, the last admin stays, a lead's projects belong to the lead's committee, a person has at most one role per committee, and only admins give someone a second committee. Removing a director or lead role here, moving it to another committee or turning a director into a lead stops the governance sync from giving it back (see `governance::opt_out`).

mod governance;

pub use governance::spawn_sync as spawn_governance_sync;

use std::collections::HashMap;

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::audit::{self, subject};
use crate::auth::groups;
use crate::auth::roles::{RoleAssignmentDto, assignments_for};
use crate::auth::{PortalViewer, Viewer, normalize_andrew_id};
use crate::error::{AppError, AppResult, Problems};
use crate::json::Json as JsonBody;
use crate::models::{Committee, PersonRef, Role, check_line};
use crate::permissions::{self, PeopleScope};
use crate::routes::common::{self, no_store, query_param};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/people", get(list).post(add))
        .route("/people/search", get(search))
        .route(
            "/people/assignments/{id}",
            patch(update_assignment).delete(remove_assignment),
        )
        .route("/people/sync-governance", post(governance::sync))
}

// ---------------------------------------------------------------------------
// Shapes

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersonDetail {
    #[serde(flatten)]
    person: PersonRef,
    /// Null unless the viewer is an admin.
    email: Option<String>,
    /// For admins, whether the person ever signed in; for directors, whether they signed in since getting their role in the director's committee.
    has_signed_in: bool,
    /// Null unless the viewer is an admin.
    last_seen_at: Option<DateTime<Utc>>,
}

/// Mirrors `PersonRow` in types.ts.
#[derive(Serialize)]
pub struct PersonRow {
    person: PersonDetail,
    roles: Vec<PeopleRole>,
}

/// A role as the People page shows it: `RoleAssignment` and what admins need to review roles governance does not list. Mirrors `PeopleRoleAssignment` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PeopleRole {
    #[serde(flatten)]
    assignment: RoleAssignmentDto,
    /// When the role last changed, and who changed it (null for the governance sync and sign-in).
    changed_at: DateTime<Utc>,
    changed_by: Option<PersonRef>,
    /// Whether governance listed this person in this role at the last successful sync: a director as their committee's director, a lead as one of its heads. Null for admins, Tech leads (governance's team files, not synced) and before the first sync.
    governance_listed: Option<bool>,
}

#[derive(FromRow)]
struct RoleReview {
    id: Uuid,
    changed_at: DateTime<Utc>,
    changer_id: Option<Uuid>,
    changer_andrew_id: Option<String>,
    changer_name: Option<String>,
    governance_listed: Option<bool>,
}

/// What a viewer may learn about the people they see.
enum Disclosure {
    /// Admins: everything.
    Full,
    /// Directors: roles in the committees they direct, and sign-ins only since those roles were given.
    Committees(Vec<Committee>),
}

impl Disclosure {
    fn of(viewer: &Viewer) -> Self {
        if permissions::sees_contact_details(viewer) {
            Self::Full
        } else {
            Self::Committees(permissions::director_of(viewer))
        }
    }
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    andrew_id: String,
    name: String,
    email: Option<String>,
    last_seen_at: Option<DateTime<Utc>>,
    has_signed_in: bool,
}

const USER_COLUMNS: &str = "u.id, u.andrew_id, u.name, u.email, u.last_seen_at,
     (u.keycloak_sub IS NOT NULL OR u.last_seen_at IS NOT NULL) AS has_signed_in";

/// When each role last changed and whether governance lists it, keyed by assignment id.
async fn role_reviews(
    conn: &mut PgConnection,
    user_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, RoleReview>> {
    let rows = sqlx::query_as::<_, RoleReview>(
        "WITH synced AS (
             SELECT EXISTS (
                 SELECT 1 FROM governance_sync_status WHERE last_success_at IS NOT NULL
             ) AS done
         )
         SELECT r.id, r.updated_at AS changed_at,
                c.id AS changer_id, c.andrew_id AS changer_andrew_id, c.name AS changer_name,
                CASE
                    WHEN r.role = 'admin' OR NOT synced.done THEN NULL
                    WHEN r.role = 'director' THEN EXISTS (
                        SELECT 1 FROM governance_directors g
                        WHERE g.committee = r.committee AND g.user_id = r.user_id)
                    WHEN r.committee = 'tech' THEN NULL
                    ELSE EXISTS (
                        SELECT 1 FROM governance_heads g
                        WHERE g.committee = r.committee AND g.user_id = r.user_id)
                END AS governance_listed
         FROM role_assignments r
         CROSS JOIN synced
         LEFT JOIN users c ON c.id = r.updated_by
         WHERE r.user_id = ANY($1)",
    )
    .bind(user_ids)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(|row| (row.id, row)).collect())
}

async fn person_rows(
    conn: &mut PgConnection,
    viewer: &Viewer,
    users: Vec<UserRow>,
) -> AppResult<Vec<PersonRow>> {
    let disclosure = Disclosure::of(viewer);
    let ids: Vec<Uuid> = users.iter().map(|u| u.id).collect();
    let mut roles = assignments_for(&mut *conn, &ids).await?;
    let mut reviews = role_reviews(conn, &ids).await?;
    Ok(users
        .into_iter()
        .map(|user| {
            let mut assignments = roles.remove(&user.id).unwrap_or_default();
            if let Disclosure::Committees(committees) = &disclosure {
                assignments.retain(|role| role.committee.is_some_and(|c| committees.contains(&c)));
            }
            let (email, last_seen_at, has_signed_in) = match &disclosure {
                Disclosure::Full => (user.email, user.last_seen_at, user.has_signed_in),
                // Only what managing the role needs: whether they have used it yet.
                Disclosure::Committees(_) => (
                    None,
                    None,
                    user.last_seen_at.is_some_and(|seen| {
                        assignments.iter().any(|role| seen >= role.appointed_at)
                    }),
                ),
            };
            PersonRow {
                roles: assignments
                    .into_iter()
                    .map(|assignment| {
                        let review = reviews.remove(&assignment.id);
                        PeopleRole {
                            changed_at: review
                                .as_ref()
                                .map_or(assignment.appointed_at, |r| r.changed_at),
                            changed_by: review.as_ref().and_then(|r| {
                                Some(PersonRef::new(
                                    r.changer_id?,
                                    r.changer_andrew_id.clone()?,
                                    r.changer_name.clone().unwrap_or_default(),
                                ))
                            }),
                            governance_listed: review.and_then(|r| r.governance_listed),
                            assignment,
                        }
                    })
                    .collect(),
                person: PersonDetail {
                    person: PersonRef::new(user.id, user.andrew_id, user.name),
                    email,
                    has_signed_in,
                    last_seen_at,
                },
            }
        })
        .collect())
}

async fn person_row(
    conn: &mut PgConnection,
    viewer: &Viewer,
    user_id: Uuid,
) -> AppResult<PersonRow> {
    let user = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users u WHERE u.id = $1"
    ))
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await?;
    person_rows(conn, viewer, vec![user])
        .await?
        .pop()
        .ok_or_else(|| AppError::not_found("person"))
}

// ---------------------------------------------------------------------------
// Listing and search

#[derive(Serialize)]
struct PeopleResponse {
    people: Vec<PersonRow>,
    /// The last governance sync, for admins only.
    #[serde(skip_serializing_if = "Option::is_none")]
    governance: Option<governance::SyncStatus>,
}

/// The committees the list covers, or `None` for everyone with a role.
fn list_scope(viewer: &Viewer, filter: Option<Committee>) -> AppResult<Option<Vec<Committee>>> {
    match (permissions::people_scope(viewer), filter) {
        (PeopleScope::None, _) => Err(AppError::Forbidden(
            "Only admins and directors can see people and roles.".into(),
        )),
        (PeopleScope::All, None) => Ok(None),
        (PeopleScope::All, Some(committee)) => Ok(Some(vec![committee])),
        (PeopleScope::Committees(list), None) => Ok(Some(list)),
        (PeopleScope::Committees(list), Some(committee)) if list.contains(&committee) => {
            Ok(Some(vec![committee]))
        }
        (PeopleScope::Committees(_), Some(committee)) => Err(AppError::Forbidden(format!(
            "You can only see people in committees you direct, not {}.",
            committee.name()
        ))),
    }
}

async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(query): RawQuery,
) -> AppResult<Response> {
    let filter =
        match query_param(query.as_deref(), "committee") {
            None => None,
            Some(slug) => Some(Committee::from_slug(&slug).ok_or_else(|| {
                AppError::field("committee", "Choose one of the seven committees.")
            })?),
        };
    let scope = list_scope(&viewer, filter)?;
    let slugs: Option<Vec<&str>> =
        scope.map(|list| list.into_iter().map(Committee::slug).collect());
    let mut conn = state.db.acquire().await?;
    let users = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users u
         WHERE EXISTS (
             SELECT 1 FROM role_assignments r
             WHERE r.user_id = u.id AND ($1::text[] IS NULL OR r.committee = ANY($1))
         )
         ORDER BY lower(coalesce(nullif(u.name, ''), u.andrew_id)), u.andrew_id"
    ))
    .bind(slugs)
    .fetch_all(&mut *conn)
    .await?;
    let people = person_rows(&mut conn, &viewer, users).await?;
    let governance = if permissions::can_sync_governance(&viewer) {
        Some(governance::status(&mut conn).await?)
    } else {
        None
    };
    Ok(no_store(
        Json(PeopleResponse { people, governance }).into_response(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    #[serde(flatten)]
    person: PersonRef,
    /// For admins only. Directors get what adding a lead needs: the name and Andrew ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    has_signed_in: Option<bool>,
}

#[derive(Serialize)]
struct PeopleSearchResponse {
    people: Vec<SearchResult>,
}

const SEARCH_LIMIT: i64 = 10;
const MAX_QUERY_LEN: usize = 64;
/// Shorter searches answer with no one, so nobody can list everyone a letter at a time.
const MIN_QUERY_LEN: usize = 2;

/// Escapes `%`, `_` and the escape character itself for `LIKE ... ESCAPE '\'`.
pub fn escape_like(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

async fn search(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(query): RawQuery,
) -> AppResult<Response> {
    if permissions::people_scope(&viewer) == PeopleScope::None {
        return Err(AppError::Forbidden(
            "Only admins and directors can look people up.".into(),
        ));
    }
    let q: String = query_param(query.as_deref(), "q")
        .unwrap_or_default()
        .trim()
        .chars()
        .take(MAX_QUERY_LEN)
        .collect();
    let people = if q.chars().count() < MIN_QUERY_LEN {
        Vec::new()
    } else {
        let escaped = escape_like(&q.to_lowercase());
        sqlx::query_as::<_, UserRow>(&format!(
            "SELECT {USER_COLUMNS} FROM users u
             WHERE u.andrew_id LIKE $1 ESCAPE '\\' OR u.name ILIKE $1 ESCAPE '\\'
             ORDER BY (u.andrew_id = $3) DESC,
                      (u.andrew_id LIKE $2 ESCAPE '\\' OR u.name ILIKE $2 ESCAPE '\\') DESC,
                      lower(coalesce(nullif(u.name, ''), u.andrew_id)), u.andrew_id
             LIMIT $4"
        ))
        .bind(format!("%{escaped}%"))
        .bind(format!("{escaped}%"))
        .bind(q.to_lowercase())
        .bind(SEARCH_LIMIT)
        .fetch_all(&state.db)
        .await?
    };
    let full = permissions::sees_contact_details(&viewer);
    let people = people
        .into_iter()
        .map(|user| SearchResult {
            person: PersonRef::new(user.id, user.andrew_id, user.name),
            has_signed_in: full.then_some(user.has_signed_in),
        })
        .collect();
    Ok(no_store(
        Json(PeopleSearchResponse { people }).into_response(),
    ))
}

// ---------------------------------------------------------------------------
// Rules shared by add, change and remove

/// "an admin", "director of Events", "a lead in Events".
fn role_phrase(role: Role, committee: Option<Committee>) -> String {
    let name = committee.map_or("", Committee::name);
    match role {
        Role::Admin => "an admin".into(),
        Role::Director => format!("director of {name}"),
        Role::Lead => format!("a lead in {name}"),
    }
}

fn display_name(name: &str, andrew_id: &str) -> String {
    if name.trim().is_empty() {
        andrew_id.to_owned()
    } else {
        name.trim().to_owned()
    }
}

/// Checks that the role and committee fit together: admins have none, the others need one.
fn check_role_shape(role: Role, committee: Option<Committee>) -> AppResult<()> {
    match (role, committee) {
        (Role::Admin, Some(_)) => Err(AppError::field(
            "committee",
            "Admins are not tied to a committee. Leave the committee empty.",
        )),
        (Role::Director | Role::Lead, None) => Err(AppError::field(
            "committee",
            "Choose the committee for this role.",
        )),
        _ => Ok(()),
    }
}

/// Authorization for one (role, committee) through `permissions`, with a message that says why not.
fn require_manage(viewer: &Viewer, role: Role, committee: Option<Committee>) -> AppResult<()> {
    if permissions::can_manage_role(viewer, role, committee) {
        return Ok(());
    }
    Err(AppError::Forbidden(match (role, committee) {
        (Role::Admin | Role::Director, _) => {
            "Only admins can add or change directors and admins.".into()
        }
        (Role::Lead, Some(committee)) => format!(
            "You can only manage leads in committees you direct, and you don't direct {}.",
            committee.name()
        ),
        (Role::Lead, None) => "Choose the committee for this role.".into(),
    }))
}

/// Locks every admin assignment and counts them, so two admins cannot remove each other at once.
async fn lock_admin_count(conn: &mut PgConnection) -> AppResult<usize> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM role_assignments WHERE role = 'admin' ORDER BY id FOR UPDATE",
    )
    .fetch_all(conn)
    .await?;
    Ok(ids.len())
}

/// The rules for taking away an admin role.
async fn check_admin_removal(
    conn: &mut PgConnection,
    viewer: &Viewer,
    owner: Uuid,
) -> AppResult<()> {
    if owner == viewer.user_id {
        return Err(AppError::Forbidden(
            "You can't remove your own admin role. Ask another admin to do it.".into(),
        ));
    }
    if lock_admin_count(conn).await? <= 1 {
        return Err(AppError::Conflict(
            "There must always be at least one admin. Add another admin first.".into(),
        ));
    }
    Ok(())
}

#[derive(FromRow)]
struct OtherRole {
    id: Uuid,
    role: Role,
    committee: Option<Committee>,
}

async fn roles_of(conn: &mut PgConnection, user_id: Uuid) -> AppResult<Vec<OtherRole>> {
    let roles = sqlx::query_as::<_, OtherRole>(
        "SELECT id, role, committee FROM role_assignments WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_all(conn)
    .await?;
    Ok(roles)
}

/// One role per committee; a second committee only from an admin. `except` is the assignment being changed.
fn check_committee_rules(
    viewer: &Viewer,
    person: &str,
    existing: &[OtherRole],
    except: Option<Uuid>,
    role: Role,
    committee: Option<Committee>,
) -> AppResult<()> {
    let others = existing.iter().filter(|other| Some(other.id) != except);
    let mut other_committees = Vec::new();
    for other in others {
        if other.role == role && other.committee == committee {
            return Err(AppError::Conflict(format!(
                "{person} is already {}.",
                role_phrase(role, committee)
            )));
        }
        if let Some(other_committee) = other.committee {
            if Some(other_committee) == committee {
                return Err(AppError::Conflict(format!(
                    "{person} is already {}. Change that role instead of adding another.",
                    role_phrase(other.role, other.committee)
                )));
            }
            other_committees.push(other_committee);
        }
    }
    if committee.is_some() && !other_committees.is_empty() && !permissions::is_admin(viewer) {
        return Err(AppError::Forbidden(format!(
            "{person} already has a role in {}. Only an admin can give someone a second committee.",
            other_committees[0].name()
        )));
    }
    Ok(())
}

/// Checks lead projects: they must exist and belong to the lead's committee.
async fn check_projects(
    conn: &mut PgConnection,
    committee: Committee,
    project_ids: &[Uuid],
) -> AppResult<Vec<Uuid>> {
    let mut ids = project_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(ids);
    }
    let owned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM projects WHERE id = ANY($1) AND committee = $2")
            .bind(&ids)
            .bind(committee)
            .fetch_one(conn)
            .await?;
    if usize::try_from(owned).ok() != Some(ids.len()) {
        return Err(AppError::field(
            "projectIds",
            format!(
                "Leads in {} can only be given projects owned by {}.",
                committee.name(),
                committee.name()
            ),
        ));
    }
    Ok(ids)
}

/// Replaces a lead's projects in one committee.
async fn set_lead_projects(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
    project_ids: &[Uuid],
    added_by: Uuid,
) -> AppResult<()> {
    clear_lead_projects(conn, user_id, committee).await?;
    if !project_ids.is_empty() {
        sqlx::query(
            "INSERT INTO project_editors (project_id, user_id, added_by)
             SELECT id, $2, $3 FROM unnest($1::uuid[]) AS id
             ON CONFLICT (project_id, user_id) DO NOTHING",
        )
        .bind(project_ids)
        .bind(user_id)
        .bind(added_by)
        .execute(conn)
        .await?;
    }
    Ok(())
}

async fn clear_lead_projects(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM project_editors pe USING projects p
         WHERE pe.project_id = p.id AND pe.user_id = $1 AND p.committee = $2",
    )
    .bind(user_id)
    .bind(committee)
    .execute(conn)
    .await?;
    Ok(())
}

/// Writes a role change to the audit log, with the role before and after it as `details`.
async fn record(
    conn: &mut PgConnection,
    viewer: Option<&Viewer>,
    action: &str,
    user_id: Uuid,
    committee: Option<Committee>,
    summary: String,
    details: serde_json::Value,
) -> AppResult<()> {
    audit::record_details(
        conn,
        audit::Entry {
            actor: viewer.map(|v| v.user_id),
            action,
            subject_type: subject::PERSON,
            subject_id: Some(user_id.to_string()),
            committee,
            summary,
        },
        details,
    )
    .await
}

/// One role as the audit log's `details` keep it. `projects` is `None` for roles that have none.
fn role_state(
    andrew_id: &str,
    role: Role,
    committee: Option<Committee>,
    can_post_ticker: bool,
    projects: Option<&[Uuid]>,
) -> serde_json::Value {
    json!({
        "andrewId": andrew_id,
        "role": role,
        "committee": committee,
        "canPostTicker": can_post_ticker,
        "projectIds": projects,
    })
}

/// A lead's projects in one committee, for the audit log.
async fn lead_project_ids(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar(
        "SELECT pe.project_id FROM project_editors pe JOIN projects p ON p.id = pe.project_id
         WHERE pe.user_id = $1 AND p.committee = $2 ORDER BY pe.project_id",
    )
    .bind(user_id)
    .bind(committee)
    .fetch_all(conn)
    .await?;
    Ok(ids)
}

// ---------------------------------------------------------------------------
// Adding

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddPersonInput {
    andrew_id: String,
    #[serde(default)]
    name: Option<String>,
    role: Role,
    #[serde(default)]
    committee: Option<Committee>,
    #[serde(default = "default_true")]
    can_post_ticker: bool,
    #[serde(default)]
    project_ids: Vec<Uuid>,
}

fn default_true() -> bool {
    true
}

const MAX_NAME_LEN: usize = 100;

struct ValidPerson {
    andrew_id: String,
    name: Option<String>,
}

fn check_person(input: &AddPersonInput) -> AppResult<ValidPerson> {
    let mut problems = Problems::new();
    let andrew_id = normalize_andrew_id(&input.andrew_id);
    if andrew_id.is_none() {
        problems.add(
            "andrewId",
            "Enter an Andrew ID: lowercase letters and numbers, without @andrew.cmu.edu.",
        );
    }
    let name = match input.name.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(raw) => match check_line(raw, 1, MAX_NAME_LEN, "the name") {
            Ok(name) => Some(name),
            Err(message) => {
                problems.add("name", message);
                None
            }
        },
    };
    if input.role != Role::Lead && !input.project_ids.is_empty() {
        problems.add(
            "projectIds",
            "Only leads are given projects. Directors and admins can edit all of theirs.",
        );
    }
    problems.finish()?;
    Ok(ValidPerson {
        andrew_id: andrew_id.unwrap_or_default(),
        name,
    })
}

/// Finds a person by Andrew ID, or adds them so the role applies when they first sign in.
async fn find_or_create_user(
    conn: &mut PgConnection,
    andrew_id: &str,
    name: Option<&str>,
) -> AppResult<(Uuid, String)> {
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM users WHERE andrew_id = $1 FOR UPDATE")
            .bind(andrew_id)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some((id, current)) = existing {
        if current.trim().is_empty()
            && let Some(name) = name
        {
            sqlx::query("UPDATE users SET name = $2 WHERE id = $1")
                .bind(id)
                .bind(name)
                .execute(&mut *conn)
                .await?;
            return Ok((id, name.to_owned()));
        }
        return Ok((id, current));
    }
    let id = Uuid::now_v7();
    let name = name.unwrap_or_default();
    sqlx::query(
        "INSERT INTO users (id, andrew_id, name) VALUES ($1, $2, $3)
         ON CONFLICT (andrew_id) DO NOTHING",
    )
    .bind(id)
    .bind(andrew_id)
    .bind(name)
    .execute(&mut *conn)
    .await?;
    // Someone may have added them a moment ago; read back whichever row won.
    let row: (Uuid, String) =
        sqlx::query_as("SELECT id, name FROM users WHERE andrew_id = $1 FOR UPDATE")
            .bind(andrew_id)
            .fetch_one(&mut *conn)
            .await?;
    Ok(row)
}

async fn add(
    State(state): State<AppState>,
    viewer: PortalViewer,
    JsonBody(input): JsonBody<AddPersonInput>,
) -> AppResult<Response> {
    check_role_shape(input.role, input.committee)?;
    require_manage(&viewer, input.role, input.committee)?;
    let person = check_person(&input)?;
    let can_post_ticker = input.role != Role::Lead || input.can_post_ticker;

    let mut tx = state.db.begin().await?;
    // The viewer's roles as they are now, in case one was removed while this request ran.
    let viewer = permissions::recheck(&mut tx, &viewer).await?;
    require_manage(&viewer, input.role, input.committee)?;
    let projects = match (input.role, input.committee) {
        (Role::Lead, Some(committee)) => {
            check_projects(&mut tx, committee, &input.project_ids).await?
        }
        _ => Vec::new(),
    };
    let (user_id, name) =
        find_or_create_user(&mut tx, &person.andrew_id, person.name.as_deref()).await?;
    let shown = display_name(&name, &person.andrew_id);
    let existing = roles_of(&mut tx, user_id).await?;
    check_committee_rules(
        &viewer,
        &shown,
        &existing,
        None,
        input.role,
        input.committee,
    )?;
    let other_roles: Vec<Role> = existing.iter().map(|other| other.role).collect();
    if !permissions::can_appoint_person_with(&viewer, &other_roles) {
        return Err(AppError::Forbidden(format!(
            "{shown} already has a role in the portal. Only an admin can give them another one."
        )));
    }

    sqlx::query(
        "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, appointed_by, source, updated_by)
         VALUES ($1, $2, $3, $4, $5, $6, 'portal', $6)",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(input.role)
    .bind(input.committee)
    .bind(can_post_ticker)
    .bind(viewer.user_id)
    .execute(&mut *tx)
    .await?;
    if let (Role::Lead, Some(committee)) = (input.role, input.committee) {
        set_lead_projects(&mut tx, user_id, committee, &projects, viewer.user_id).await?;
    }
    if input.role == Role::Admin {
        groups::opt_back_in(&mut tx, user_id).await?;
    }
    let verb = if input.role == Role::Lead {
        "Added"
    } else {
        "Made"
    };
    let joiner = if input.role == Role::Lead { "as " } else { "" };
    let after = role_state(
        &person.andrew_id,
        input.role,
        input.committee,
        can_post_ticker,
        (input.role == Role::Lead).then_some(projects.as_slice()),
    );
    record(
        &mut tx,
        Some(&viewer),
        "role.add",
        user_id,
        input.committee,
        format!(
            "{verb} {shown} {joiner}{}",
            role_phrase(input.role, input.committee)
        ),
        audit::change(serde_json::Value::Null, after),
    )
    .await?;
    let row = person_row(&mut tx, &viewer, user_id).await?;
    tx.commit().await?;
    Ok(no_store((StatusCode::CREATED, Json(row)).into_response()))
}

// ---------------------------------------------------------------------------
// Changing and removing

/// Deserializes a field that may be missing (outer `None`) or null (inner `None`).
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAssignmentInput {
    #[serde(default)]
    role: Option<Role>,
    #[serde(default, deserialize_with = "present")]
    committee: Option<Option<Committee>>,
    #[serde(default)]
    can_post_ticker: Option<bool>,
    #[serde(default)]
    project_ids: Option<Vec<Uuid>>,
}

#[derive(FromRow)]
struct AssignmentRow {
    user_id: Uuid,
    role: Role,
    committee: Option<Committee>,
    can_post_ticker: bool,
    andrew_id: String,
    name: String,
}

async fn lock_assignment(conn: &mut PgConnection, id: Uuid) -> AppResult<AssignmentRow> {
    sqlx::query_as::<_, AssignmentRow>(
        "SELECT r.user_id, r.role, r.committee, r.can_post_ticker, u.andrew_id, u.name
         FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE r.id = $1 FOR UPDATE OF r",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| AppError::not_found("role"))
}

fn parse_id(raw: &str) -> AppResult<Uuid> {
    common::parse_id(raw, "role")
}

async fn update_assignment(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<UpdateAssignmentInput>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let viewer = permissions::recheck(&mut tx, &viewer).await?;
    let current = lock_assignment(&mut tx, id).await?;
    require_manage(&viewer, current.role, current.committee)?;

    let role = input.role.unwrap_or(current.role);
    let committee = match (role, input.committee) {
        (Role::Admin, None) => None,
        (_, Some(chosen)) => chosen,
        (_, None) => current.committee,
    };
    check_role_shape(role, committee)?;
    require_manage(&viewer, role, committee)?;
    if role != Role::Lead
        && input
            .project_ids
            .as_ref()
            .is_some_and(|ids| !ids.is_empty())
    {
        return Err(AppError::field(
            "projectIds",
            "Only leads are given projects. Directors and admins can edit all of theirs.",
        ));
    }

    let shown = display_name(&current.name, &current.andrew_id);
    let moved = role != current.role || committee != current.committee;
    if current.role == Role::Admin && role != Role::Admin {
        check_admin_removal(&mut tx, &viewer, current.user_id).await?;
        // Without this, the Keycloak admin group would give the admin role back at their next session check.
        groups::opt_out(&mut tx, current.user_id, viewer.user_id).await?;
    } else if role == Role::Admin && current.role != Role::Admin {
        groups::opt_back_in(&mut tx, current.user_id).await?;
    }
    if moved {
        let existing = roles_of(&mut tx, current.user_id).await?;
        check_committee_rules(&viewer, &shown, &existing, Some(id), role, committee)?;
    }
    let projects = match (role, committee, &input.project_ids) {
        (Role::Lead, Some(c), Some(ids)) => Some(check_projects(&mut tx, c, ids).await?),
        _ => None,
    };
    let can_post_ticker = if role == Role::Lead {
        input.can_post_ticker.unwrap_or(current.can_post_ticker)
    } else {
        true
    };
    let projects_before = match (current.role, current.committee) {
        (Role::Lead, Some(c)) => Some(lead_project_ids(&mut tx, current.user_id, c).await?),
        _ => None,
    };
    let before = role_state(
        &current.andrew_id,
        current.role,
        current.committee,
        current.can_post_ticker,
        projects_before.as_deref(),
    );

    if moved {
        sqlx::query(
            "UPDATE role_assignments
             SET role = $2, committee = $3, can_post_ticker = $4, appointed_by = $5,
                 source = 'portal', created_at = now(), updated_at = now(), updated_by = $5
             WHERE id = $1",
        )
        .bind(id)
        .bind(role)
        .bind(committee)
        .bind(can_post_ticker)
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
        if let (Role::Lead, Some(old)) = (current.role, current.committee) {
            clear_lead_projects(&mut tx, current.user_id, old).await?;
        }
        // Without this, the governance sync would give back the role just moved or lowered.
        if let Some(old) = current.committee
            && !(role == Role::Director && committee == Some(old))
        {
            governance::opt_out(&mut tx, current.user_id, old, current.role).await?;
        }
    } else {
        // A lead edited here is the portal's from now on, so a governance sync no longer removes it.
        sqlx::query(
            "UPDATE role_assignments
             SET can_post_ticker = $2,
                 source = CASE WHEN role = 'lead' THEN 'portal' ELSE source END,
                 updated_at = now(), updated_by = $3
             WHERE id = $1",
        )
        .bind(id)
        .bind(can_post_ticker)
        .bind(viewer.user_id)
        .execute(&mut *tx)
        .await?;
    }
    if let (Some(ids), Some(c)) = (&projects, committee) {
        set_lead_projects(&mut tx, current.user_id, c, ids, viewer.user_id).await?;
    }

    let summary = if moved {
        format!(
            "Changed {shown} from {} to {}",
            role_phrase(current.role, current.committee),
            role_phrase(role, committee)
        )
    } else {
        format!(
            "Updated what {shown} can do as {}",
            role_phrase(role, committee)
        )
    };
    let projects_after = match (role, committee) {
        (Role::Lead, Some(c)) => Some(lead_project_ids(&mut tx, current.user_id, c).await?),
        _ => None,
    };
    let after = role_state(
        &current.andrew_id,
        role,
        committee,
        can_post_ticker,
        projects_after.as_deref(),
    );
    record(
        &mut tx,
        Some(&viewer),
        "role.update",
        current.user_id,
        committee,
        summary,
        audit::change(before, after),
    )
    .await?;
    let row = person_row(&mut tx, &viewer, current.user_id).await?;
    tx.commit().await?;
    Ok(no_store(Json(row).into_response()))
}

async fn remove_assignment(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let viewer = permissions::recheck(&mut tx, &viewer).await?;
    let current = lock_assignment(&mut tx, id).await?;
    require_manage(&viewer, current.role, current.committee)?;
    if current.role == Role::Admin {
        check_admin_removal(&mut tx, &viewer, current.user_id).await?;
        // Without this, the Keycloak admin group would give the role back at their next session check, from any session they or someone else still holds.
        groups::opt_out(&mut tx, current.user_id, viewer.user_id).await?;
    }
    let projects_before = match (current.role, current.committee) {
        (Role::Lead, Some(c)) => Some(lead_project_ids(&mut tx, current.user_id, c).await?),
        _ => None,
    };
    let before = role_state(
        &current.andrew_id,
        current.role,
        current.committee,
        current.can_post_ticker,
        projects_before.as_deref(),
    );
    sqlx::query("DELETE FROM role_assignments WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if let (Role::Lead, Some(committee)) = (current.role, current.committee) {
        clear_lead_projects(&mut tx, current.user_id, committee).await?;
    }
    // Without this, the governance sync would give the role back while governance lists them.
    if let Some(committee) = current.committee {
        governance::opt_out(&mut tx, current.user_id, committee, current.role).await?;
    }
    // Someone left with no role is signed out everywhere, so a copied cookie stops working and no session gets an admin role back from the Keycloak group.
    crate::auth::session::end_if_roleless(&mut *tx, current.user_id).await?;
    let shown = display_name(&current.name, &current.andrew_id);
    record(
        &mut tx,
        Some(&viewer),
        "role.remove",
        current.user_id,
        current.committee,
        format!(
            "Removed {shown} as {}",
            role_phrase(current.role, current.committee)
        ),
        audit::change(before, serde_json::Value::Null),
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::NO_CONTENT.into_response()))
}

#[cfg(test)]
mod tests;
