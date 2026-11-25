//! The home page ticker: the public feed and the portal's announcement endpoints. See "Ticker" in docs/implementation/contract.md.
//!
//! Who may do what comes from `crate::permissions`. On top of it: at most six announcements are live at once, an announcement leaves the ticker on its own at `expires_at`, and a lead's announcement goes live only when a director approves it through `apply_approval`.

mod model;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};

use axum::extract::{Path, RawQuery, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::approvals_core::{self, ApprovalRow, ApprovalStatus, SubjectType};
use crate::audit::{self, subject};
use crate::auth::{PortalViewer, Viewer};
use crate::error::{AppError, AppResult};
use crate::json::Json as JsonBody;
use crate::models::{Committee, PersonRef, TickerSource};
use crate::permissions::{self, Posting};
use crate::routes::common::{self, no_store, quoted};
use crate::state::AppState;

use model::{
    AnnouncementInput, AnnouncementKind, AnnouncementPatch, AnnouncementState, Row, Valid,
    approval_too_late, validate,
};

/// At most this many announcements are live at once.
pub const MAX_LIVE: i64 = 6;
/// Expired and ended announcements stay listed this long, so they can be posted again.
const EXPIRED_KEPT_DAYS: i64 = 90;
/// "Extend" takes 1 to this many days.
const MAX_EXTEND_DAYS: i64 = 30;
/// Held while counting or reordering the live set, so two requests cannot both take the last slot.
const LIVE_SET_LOCK: i64 = 724_610;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/public/ticker", get(public_ticker))
        .route("/announcements", get(list).post(create))
        .route("/announcements/order", put(reorder))
        .route(
            "/announcements/{id}",
            get(show).patch(update).delete(remove),
        )
        .route("/announcements/{id}/extend", post(extend))
        .route("/announcements/{id}/end", post(end))
        .route("/announcements/{id}/repost", post(repost))
}

// ---------------------------------------------------------------------------
// Public feed

/// Mirrors `PublicAnnouncement` in types.ts. Only what visitors see.
#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct PublicAnnouncement {
    id: Uuid,
    kind: AnnouncementKind,
    committee: TickerSource,
    message: String,
    link_url: Option<String>,
}

#[derive(Serialize)]
struct PublicTickerResponse {
    announcements: Vec<PublicAnnouncement>,
}

/// Live announcements at request time, so expiry needs no deploy and no scheduled job.
async fn public_ticker(State(state): State<AppState>) -> AppResult<Response> {
    let announcements = sqlx::query_as::<_, PublicAnnouncement>(
        "SELECT id, kind, committee, message, link_url FROM announcements
         WHERE state = 'live' AND ended_at IS NULL AND starts_at <= $1 AND expires_at > $1
         ORDER BY position, starts_at, id",
    )
    .bind(Utc::now())
    .fetch_all(&state.db)
    .await?;
    let mut response = Json(PublicTickerResponse { announcements }).into_response();
    let headers = response.headers_mut();
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, stale-while-revalidate=600"),
    );
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    Ok(response)
}

// ---------------------------------------------------------------------------
// Shapes for the portal

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

/// Mirrors `Announcement` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnouncementDto {
    id: Uuid,
    kind: AnnouncementKind,
    committee: TickerSource,
    message: String,
    link_url: Option<String>,
    state: AnnouncementState,
    starts_at: Option<DateTime<Utc>>,
    expires_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    position: i32,
    created_by: Option<PersonRef>,
    approved_by: Option<PersonRef>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    approval: Option<ApprovalRef>,
    can_edit: bool,
    can_manage: bool,
}

#[derive(Serialize)]
struct AnnouncementResponse {
    announcement: AnnouncementDto,
}

#[derive(Serialize)]
struct Slots {
    used: i64,
    total: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct TabCounts {
    live: i64,
    waiting: i64,
    expired: i64,
}

/// The directors who approve a lead's announcements from one committee.
#[derive(Serialize)]
struct ApproverGroup {
    committee: Committee,
    people: Vec<PersonRef>,
}

/// Mirrors `AnnouncementListResponse` in types.ts.
#[derive(Serialize)]
struct AnnouncementListResponse {
    announcements: Vec<AnnouncementDto>,
    slots: Slots,
    counts: TabCounts,
    approvers: Vec<ApproverGroup>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Live,
    Waiting,
    Expired,
}

impl Tab {
    fn parse(query: Option<&str>) -> AppResult<Self> {
        let value = url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
            .find(|(key, _)| key == "tab")
            .map(|(_, value)| value.trim().to_owned());
        match value.as_deref() {
            None | Some("" | "live") => Ok(Self::Live),
            Some("waiting") => Ok(Self::Waiting),
            Some("expired") => Ok(Self::Expired),
            Some(_) => Err(AppError::field(
                "tab",
                "Choose the live, waiting or expired announcements.",
            )),
        }
    }
}

fn one(announcement: AnnouncementDto) -> Json<AnnouncementResponse> {
    Json(AnnouncementResponse { announcement })
}

// ---------------------------------------------------------------------------
// Reading

// The tabs as SQL conditions. `$1` is now, `$2` the days expired ones are kept, `$3` the viewer and `$4` the sources they manage: a waiting announcement shows to its author and its managers.
const LIVE: &str = "state = 'live' AND expires_at > $1";
const WAITING: &str =
    "state IN ('pending_approval', 'sent_back') AND (created_by = $3 OR committee = ANY($4))";
const EXPIRED: &str = "((state = 'live' AND expires_at <= $1) OR state = 'ended')
     AND coalesce(ended_at, expires_at) > $1 - make_interval(days => $2)";

/// Sources whose waiting announcements the viewer sees besides their own.
fn managed_slugs(viewer: &Viewer) -> Vec<&'static str> {
    permissions::ticker_manages(viewer)
        .into_iter()
        .map(TickerSource::slug)
        .collect()
}

fn kept_days() -> i32 {
    i32::try_from(EXPIRED_KEPT_DAYS).unwrap_or(90)
}

async fn rows_for(
    conn: &mut PgConnection,
    viewer: &Viewer,
    tab: Tab,
    now: DateTime<Utc>,
) -> AppResult<Vec<Row>> {
    let columns = Row::COLUMNS;
    let rows =
        match tab {
            Tab::Live => sqlx::query_as::<_, Row>(&format!(
                "SELECT {columns} FROM announcements WHERE {LIVE} ORDER BY position, starts_at, id"
            ))
            .bind(now)
            .fetch_all(conn)
            .await?,
            // Every condition takes the same four parameters, so the unused ones are still referenced.
            Tab::Waiting => {
                sqlx::query_as::<_, Row>(&format!(
                    "SELECT {columns} FROM announcements
                 WHERE {WAITING} AND $1::timestamptz IS NOT NULL AND $2::int IS NOT NULL
                 ORDER BY created_at DESC, id DESC"
                ))
                .bind(now)
                .bind(kept_days())
                .bind(viewer.user_id)
                .bind(managed_slugs(viewer))
                .fetch_all(conn)
                .await?
            }
            Tab::Expired => {
                sqlx::query_as::<_, Row>(&format!(
                    "SELECT {columns} FROM announcements WHERE {EXPIRED}
                 ORDER BY coalesce(ended_at, expires_at) DESC, id DESC"
                ))
                .bind(now)
                .bind(kept_days())
                .fetch_all(conn)
                .await?
            }
        };
    Ok(rows)
}

async fn tab_counts(
    conn: &mut PgConnection,
    viewer: &Viewer,
    now: DateTime<Utc>,
) -> AppResult<TabCounts> {
    let counts = sqlx::query_as::<_, TabCounts>(&format!(
        "SELECT count(*) FILTER (WHERE {LIVE}) AS live,
                count(*) FILTER (WHERE {WAITING}) AS waiting,
                count(*) FILTER (WHERE {EXPIRED}) AS expired
         FROM announcements"
    ))
    .bind(now)
    .bind(kept_days())
    .bind(viewer.user_id)
    .bind(managed_slugs(viewer))
    .fetch_one(conn)
    .await?;
    Ok(counts)
}

/// Directors of each committee the viewer posts to with approval, so the composer can name them.
async fn approvers_for(conn: &mut PgConnection, viewer: &Viewer) -> AppResult<Vec<ApproverGroup>> {
    let committees: Vec<Committee> = permissions::ticker_sources(viewer)
        .into_iter()
        .filter(|source| {
            permissions::ticker_posting(viewer, *source) == Some(Posting::NeedsApproval)
        })
        .filter_map(TickerSource::committee)
        .collect();
    if committees.is_empty() {
        return Ok(Vec::new());
    }
    let slugs: Vec<&str> = committees.iter().map(|c| c.slug()).collect();
    let rows = sqlx::query_as::<_, (Committee, Uuid, String, String)>(
        "SELECT r.committee, u.id, u.andrew_id, u.name
         FROM role_assignments r JOIN users u ON u.id = r.user_id
         WHERE r.role = 'director' AND r.committee = ANY($1)
         ORDER BY lower(coalesce(nullif(u.name, ''), u.andrew_id)), u.andrew_id",
    )
    .bind(&slugs)
    .fetch_all(conn)
    .await?;
    Ok(committees
        .into_iter()
        .map(|committee| ApproverGroup {
            committee,
            people: rows
                .iter()
                .filter(|(c, ..)| *c == committee)
                .map(|(_, id, andrew_id, name)| {
                    PersonRef::new(*id, andrew_id.clone(), name.clone())
                })
                .collect(),
        })
        .collect())
}

async fn people(
    conn: &mut PgConnection,
    ids: &BTreeSet<Uuid>,
) -> AppResult<HashMap<Uuid, PersonRef>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids: Vec<Uuid> = ids.iter().copied().collect();
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, andrew_id, name FROM users WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, andrew_id, name)| (id, PersonRef::new(id, andrew_id, name)))
        .collect())
}

/// Builds the portal view of each row: people, the latest approval and what the viewer may do.
async fn describe(
    conn: &mut PgConnection,
    viewer: &Viewer,
    rows: Vec<Row>,
    now: DateTime<Utc>,
) -> AppResult<Vec<AnnouncementDto>> {
    let row_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let mut latest =
        approvals_core::latest_of_each(&mut *conn, SubjectType::Announcement, &row_ids).await?;
    let approvals: Vec<_> = rows.iter().map(|row| latest.remove(&row.id)).collect();
    let ids: BTreeSet<Uuid> = rows
        .iter()
        .flat_map(|row| [row.created_by, row.approved_by])
        .chain(
            approvals
                .iter()
                .map(|a| a.as_ref().and_then(|a| a.decided_by)),
        )
        .flatten()
        .collect();
    let people = people(conn, &ids).await?;
    let person = |id: Option<Uuid>| id.and_then(|id| people.get(&id).cloned());
    Ok(rows
        .into_iter()
        .zip(approvals)
        .map(|(row, approval)| {
            let open = row.is_open(now);
            AnnouncementDto {
                can_edit: open
                    && permissions::can_edit_announcement(
                        viewer,
                        row.committee,
                        row.created_by,
                        row.is_waiting(),
                    ),
                can_manage: permissions::can_manage_ticker(viewer, row.committee),
                state: row.shown_state(now),
                approval: approval.map(|a| ApprovalRef {
                    id: a.id,
                    status: a.status,
                    note: a.note,
                    decided_by: person(a.decided_by),
                    decided_at: a.decided_at,
                }),
                created_by: person(row.created_by),
                approved_by: person(row.approved_by),
                id: row.id,
                kind: row.kind,
                committee: row.committee,
                message: row.message,
                link_url: row.link_url,
                starts_at: row.starts_at,
                expires_at: row.expires_at,
                ended_at: row.ended_at,
                position: row.position,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        })
        .collect())
}

async fn describe_one(
    conn: &mut PgConnection,
    viewer: &Viewer,
    row: Row,
    now: DateTime<Utc>,
) -> AppResult<AnnouncementDto> {
    describe(conn, viewer, vec![row], now)
        .await?
        .pop()
        .ok_or_else(|| AppError::not_found("announcement"))
}

async fn list_response(
    conn: &mut PgConnection,
    viewer: &Viewer,
    tab: Tab,
    now: DateTime<Utc>,
) -> AppResult<AnnouncementListResponse> {
    let rows = rows_for(conn, viewer, tab, now).await?;
    let announcements = describe(conn, viewer, rows, now).await?;
    let counts = tab_counts(conn, viewer, now).await?;
    let approvers = approvers_for(conn, viewer).await?;
    Ok(AnnouncementListResponse {
        announcements,
        slots: Slots {
            used: counts.live,
            total: MAX_LIVE,
        },
        counts,
        approvers,
    })
}

async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(query): RawQuery,
) -> AppResult<Response> {
    let tab = Tab::parse(query.as_deref())?;
    let mut conn = state.db.acquire().await?;
    let body = list_response(&mut conn, &viewer, tab, Utc::now()).await?;
    Ok(no_store(Json(body).into_response()))
}

/// Announcement ids come from the path; anything that is not one is an announcement we don't have.
fn parse_id(raw: &str) -> AppResult<Uuid> {
    common::parse_id(raw, "announcement")
}

/// Waiting announcements are private to their author and the people who manage the source.
fn can_see(viewer: &Viewer, row: &Row) -> bool {
    permissions::can_view_announcement(viewer, row.committee, row.created_by, row.is_waiting())
}

async fn show(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    let mut conn = state.db.acquire().await?;
    let row = find(&mut conn, id, false)
        .await?
        .filter(|row| can_see(&viewer, row))
        .ok_or_else(|| AppError::not_found("announcement"))?;
    let announcement = describe_one(&mut conn, &viewer, row, Utc::now()).await?;
    Ok(no_store(one(announcement).into_response()))
}

async fn find(conn: &mut PgConnection, id: Uuid, lock: bool) -> AppResult<Option<Row>> {
    let row = sqlx::query_as::<_, Row>(&format!(
        "SELECT {} FROM announcements WHERE id = $1{}",
        Row::COLUMNS,
        if lock { " FOR UPDATE" } else { "" }
    ))
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row)
}

/// Locks the announcement for a change. Its waiting approval is locked first, in the same order `routes/approvals.rs` uses, so an edit and an approval cannot deadlock.
async fn lock(conn: &mut PgConnection, id: Uuid) -> AppResult<Row> {
    sqlx::query(
        "SELECT id FROM approvals
         WHERE subject_type = 'announcement' AND subject_id = $1 AND status = 'waiting'
         FOR UPDATE",
    )
    .bind(id)
    .execute(&mut *conn)
    .await?;
    find(conn, id, true)
        .await?
        .ok_or_else(|| AppError::not_found("announcement"))
}

// ---------------------------------------------------------------------------
// The six slots

async fn lock_live_set(conn: &mut PgConnection) -> AppResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(LIVE_SET_LOCK)
        .execute(conn)
        .await?;
    Ok(())
}

/// Fails when all six slots are taken. `then` finishes "End an announcement first, then ...".
async fn ensure_free_slot(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
    then: &str,
) -> AppResult<()> {
    let used: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM announcements WHERE {LIVE}"))
        .bind(now)
        .fetch_one(conn)
        .await?;
    if used >= MAX_LIVE {
        return Err(AppError::Conflict(format!(
            "All {MAX_LIVE} ticker slots are in use. End an announcement first, then {then}."
        )));
    }
    Ok(())
}

/// The position after every live announcement, so a new one joins the end of the loop.
async fn next_position(conn: &mut PgConnection, now: DateTime<Utc>) -> AppResult<i32> {
    let position: i32 = sqlx::query_scalar(&format!(
        "SELECT coalesce(max(position) + 1, 0) FROM announcements WHERE {LIVE}"
    ))
    .bind(now)
    .fetch_one(conn)
    .await?;
    Ok(position)
}

// ---------------------------------------------------------------------------
// Writing

async fn log(
    conn: &mut PgConnection,
    actor: &Viewer,
    action: &str,
    row: &Row,
    summary: String,
) -> AppResult<()> {
    audit::record(
        conn,
        audit::Entry {
            actor: Some(actor.user_id),
            action,
            subject_type: subject::ANNOUNCEMENT,
            subject_id: Some(row.id.to_string()),
            committee: row.committee.committee(),
            summary,
        },
    )
    .await
}

fn require_posting(viewer: &Viewer, source: TickerSource) -> AppResult<Posting> {
    permissions::ticker_posting(viewer, source).ok_or_else(|| {
        AppError::Forbidden(format!(
            "You can't post to the ticker for {}. Pick a committee you post for.",
            source.name()
        ))
    })
}

fn require_manage(viewer: &Viewer, source: TickerSource) -> AppResult<()> {
    if permissions::can_manage_ticker(viewer, source) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can change its live announcements.",
            source.name()
        )))
    }
}

/// Saves a new announcement: live at once for admins and directors, or waiting for a director's approval for leads.
async fn post_new(
    conn: &mut PgConnection,
    viewer: &Viewer,
    valid: &Valid,
    now: DateTime<Utc>,
) -> AppResult<Row> {
    let posting = require_posting(viewer, valid.committee)?;
    let id = Uuid::now_v7();
    let row = match posting {
        Posting::Direct => {
            lock_live_set(conn).await?;
            ensure_free_slot(conn, now, "post this one").await?;
            let position = next_position(conn, now).await?;
            let row = insert(conn, id, viewer, valid, "live", Some(now), position).await?;
            log(
                conn,
                viewer,
                "announcement.publish",
                &row,
                format!("Posted {} to the ticker", quoted(&row.message)),
            )
            .await?;
            row
        }
        Posting::NeedsApproval => {
            let committee = approval_committee(valid.committee)?;
            let row = insert(conn, id, viewer, valid, "pending_approval", None, 0).await?;
            approvals_core::submit(
                conn,
                SubjectType::Announcement,
                id,
                committee,
                viewer.user_id,
                &serde_json::to_value(row.input())?,
            )
            .await?;
            log(
                conn,
                viewer,
                "announcement.submit",
                &row,
                format!("Sent {} for approval", quoted(&row.message)),
            )
            .await?;
            row
        }
    };
    Ok(row)
}

/// Announcements that wait for approval belong to a committee, whose director decides.
fn approval_committee(source: TickerSource) -> AppResult<Committee> {
    source.committee().ok_or_else(|| {
        AppError::field(
            "committee",
            "Announcements from ScottyLabs as a whole can't wait for approval. Pick a committee.",
        )
    })
}

async fn insert(
    conn: &mut PgConnection,
    id: Uuid,
    viewer: &Viewer,
    valid: &Valid,
    state: &str,
    starts_at: Option<DateTime<Utc>>,
    position: i32,
) -> AppResult<Row> {
    let row = sqlx::query_as::<_, Row>(&format!(
        "INSERT INTO announcements
             (id, kind, committee, message, link_url, state, starts_at, expires_at, position, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING {}",
        Row::COLUMNS
    ))
    .bind(id)
    .bind(valid.kind)
    .bind(valid.committee)
    .bind(&valid.message)
    .bind(&valid.link_url)
    .bind(state)
    .bind(starts_at)
    .bind(valid.expires_at)
    .bind(position)
    .bind(viewer.user_id)
    .fetch_one(conn)
    .await?;
    Ok(row)
}

async fn create(
    State(state): State<AppState>,
    viewer: PortalViewer,
    JsonBody(input): JsonBody<AnnouncementInput>,
) -> AppResult<Response> {
    let now = Utc::now();
    let valid = validate(&input, now)?;
    let mut tx = state.db.begin().await?;
    let row = post_new(&mut tx, &viewer, &valid, now).await?;
    let announcement = describe_one(&mut tx, &viewer, row, now).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, one(announcement)).into_response())
}

/// Whether the viewer may move an announcement they can edit to another source.
fn may_use_source(viewer: &Viewer, row: &Row, source: TickerSource) -> bool {
    if permissions::can_manage_ticker(viewer, row.committee) {
        permissions::can_manage_ticker(viewer, source)
    } else {
        permissions::ticker_posting(viewer, source).is_some()
    }
}

async fn update(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(patch): JsonBody<AnnouncementPatch>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let row = lock(&mut tx, id).await?;
    if !can_see(&viewer, &row) {
        return Err(AppError::not_found("announcement"));
    }
    if !row.is_open(now) {
        return Err(AppError::Conflict(
            "This announcement has left the ticker. Post it again instead.".into(),
        ));
    }
    if !permissions::can_edit_announcement(&viewer, row.committee, row.created_by, row.is_waiting())
    {
        return Err(AppError::Forbidden(
            "You can edit your own announcements while they wait for approval. Ask your director to change live ones."
                .into(),
        ));
    }
    let valid = validate(&patch.apply_to(row.input()), now)?;
    if valid.committee != row.committee && !may_use_source(&viewer, &row, valid.committee) {
        return Err(AppError::field(
            "committee",
            format!(
                "You can't post to the ticker for {}. Pick a committee you post for.",
                valid.committee.name()
            ),
        ));
    }

    let waiting = row.is_waiting();
    let updated = sqlx::query_as::<_, Row>(&format!(
        "UPDATE announcements
         SET kind = $2, committee = $3, message = $4, link_url = $5, expires_at = $6,
             state = CASE WHEN $7 THEN 'pending_approval' ELSE state END,
             updated_at = now()
         WHERE id = $1
         RETURNING {}",
        Row::COLUMNS
    ))
    .bind(id)
    .bind(valid.kind)
    .bind(valid.committee)
    .bind(&valid.message)
    .bind(&valid.link_url)
    .bind(valid.expires_at)
    .bind(waiting)
    .fetch_one(&mut *tx)
    .await?;

    if waiting {
        // Editing a submission sends the new version for approval in place of the old one.
        let committee = approval_committee(updated.committee)?;
        approvals_core::submit(
            &mut tx,
            SubjectType::Announcement,
            id,
            committee,
            updated.created_by.unwrap_or(viewer.user_id),
            &serde_json::to_value(updated.input())?,
        )
        .await?;
        // An approver who fixes a lead's submission keeps it waiting; it stays the lead's.
        let summary = if updated.created_by == Some(viewer.user_id) {
            format!("Sent {} for approval again", quoted(&updated.message))
        } else {
            format!(
                "Edited {} while it waits for approval",
                quoted(&updated.message)
            )
        };
        log(&mut tx, &viewer, "announcement.resubmit", &updated, summary).await?;
    } else {
        log(
            &mut tx,
            &viewer,
            "announcement.edit",
            &updated,
            format!("Edited {} on the ticker", quoted(&updated.message)),
        )
        .await?;
    }
    let announcement = describe_one(&mut tx, &viewer, updated, now).await?;
    tx.commit().await?;
    Ok(one(announcement).into_response())
}

#[derive(Deserialize)]
struct ExtendInput {
    days: i64,
}

async fn extend(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<ExtendInput>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    if !(1..=MAX_EXTEND_DAYS).contains(&input.days) {
        return Err(AppError::field(
            "days",
            format!("Extend by 1 to {MAX_EXTEND_DAYS} days."),
        ));
    }
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let row = lock(&mut tx, id).await?;
    require_manage(&viewer, row.committee)?;
    if !row.is_live(now) {
        return Err(AppError::Conflict(
            "Only live announcements can be extended. Post this one again instead.".into(),
        ));
    }
    let limit = now + TimeDelta::days(model::MAX_DAYS);
    let expires_at = (row.expires_at + TimeDelta::days(input.days)).min(limit);
    // Already at the limit, the only gain is the time since it was last extended.
    if expires_at - row.expires_at < TimeDelta::minutes(1) {
        return Err(AppError::field(
            "days",
            "This announcement already stays as long as it can, 30 days from now.",
        ));
    }
    let updated = set_expiry(&mut tx, id, expires_at).await?;
    let summary = if expires_at == limit {
        format!("Extended {} to the 30-day limit", quoted(&updated.message))
    } else {
        format!(
            "Extended {} by {} day{}",
            quoted(&updated.message),
            input.days,
            if input.days == 1 { "" } else { "s" }
        )
    };
    log(&mut tx, &viewer, "announcement.extend", &updated, summary).await?;
    let announcement = describe_one(&mut tx, &viewer, updated, now).await?;
    tx.commit().await?;
    Ok(one(announcement).into_response())
}

async fn set_expiry(
    conn: &mut PgConnection,
    id: Uuid,
    expires_at: DateTime<Utc>,
) -> AppResult<Row> {
    let row = sqlx::query_as::<_, Row>(&format!(
        "UPDATE announcements SET expires_at = $2, updated_at = now() WHERE id = $1 RETURNING {}",
        Row::COLUMNS
    ))
    .bind(id)
    .bind(expires_at)
    .fetch_one(conn)
    .await?;
    Ok(row)
}

async fn end(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let row = lock(&mut tx, id).await?;
    require_manage(&viewer, row.committee)?;
    if !row.is_live(now) {
        return Err(AppError::Conflict(
            "This announcement is not on the ticker, so there is nothing to end.".into(),
        ));
    }
    let updated = sqlx::query_as::<_, Row>(&format!(
        "UPDATE announcements SET state = 'ended', ended_at = $2, updated_at = now()
         WHERE id = $1 RETURNING {}",
        Row::COLUMNS
    ))
    .bind(id)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    log(
        &mut tx,
        &viewer,
        "announcement.end",
        &updated,
        format!("Took {} off the ticker", quoted(&updated.message)),
    )
    .await?;
    let announcement = describe_one(&mut tx, &viewer, updated, now).await?;
    tx.commit().await?;
    Ok(one(announcement).into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepostInput {
    expires_at: DateTime<Utc>,
}

async fn repost(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<RepostInput>,
) -> AppResult<Response> {
    let id = parse_id(&id)?;
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let original = find(&mut tx, id, false)
        .await?
        .ok_or_else(|| AppError::not_found("announcement"))?;
    require_posting(&viewer, original.committee)?;
    if original.is_open(now) {
        return Err(AppError::Conflict(
            "Only announcements that have left the ticker can be posted again.".into(),
        ));
    }
    let valid = validate(
        &AnnouncementInput {
            expires_at: input.expires_at,
            ..original.input()
        },
        now,
    )?;
    let row = post_new(&mut tx, &viewer, &valid, now).await?;
    let announcement = describe_one(&mut tx, &viewer, row, now).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, one(announcement)).into_response())
}

#[derive(Deserialize)]
struct OrderInput {
    ids: Vec<Uuid>,
}

async fn reorder(
    State(state): State<AppState>,
    viewer: PortalViewer,
    JsonBody(input): JsonBody<OrderInput>,
) -> AppResult<Response> {
    if !permissions::can_reorder_ticker(&viewer) {
        return Err(AppError::Forbidden(
            "Only admins and directors can change the order of the ticker.".into(),
        ));
    }
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    lock_live_set(&mut tx).await?;
    let live: Vec<Uuid> = sqlx::query_scalar(&format!(
        "SELECT id FROM announcements WHERE {LIVE} ORDER BY id FOR UPDATE"
    ))
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    let sent: BTreeSet<Uuid> = input.ids.iter().copied().collect();
    let current: BTreeSet<Uuid> = live.into_iter().collect();
    if sent.len() != input.ids.len() || sent != current {
        return Err(AppError::Conflict(
            "The ticker changed while you were reordering it. Reload the page and try again."
                .into(),
        ));
    }
    let positions: Vec<i32> = (0..).take(input.ids.len()).collect();
    sqlx::query(
        "UPDATE announcements a SET position = o.position, updated_at = now()
         FROM unnest($1::uuid[], $2::int[]) AS o(id, position)
         WHERE a.id = o.id AND a.position <> o.position",
    )
    .bind(&input.ids)
    .bind(&positions)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        audit::Entry {
            actor: Some(viewer.user_id),
            action: "announcement.reorder",
            subject_type: subject::ANNOUNCEMENT,
            subject_id: None,
            committee: None,
            summary: "Changed the order of the ticker".into(),
        },
    )
    .await?;
    let body = list_response(&mut tx, &viewer, Tab::Live, now).await?;
    tx.commit().await?;
    Ok(no_store(Json(body).into_response()))
}

async fn remove(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = parse_id(&id)?;
    let mut tx = state.db.begin().await?;
    let row = lock(&mut tx, id).await?;
    if !can_see(&viewer, &row) {
        return Err(AppError::not_found("announcement"));
    }
    let own_submission = row.is_waiting() && row.created_by == Some(viewer.user_id);
    if !own_submission && !permissions::can_manage_ticker(&viewer, row.committee) {
        return Err(AppError::Forbidden(
            "You can delete your own announcements while they wait for approval. Ask your director to remove others."
                .into(),
        ));
    }
    approvals_core::withdraw_waiting(&mut tx, SubjectType::Announcement, id).await?;
    sqlx::query("DELETE FROM announcements WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    log(
        &mut tx,
        &viewer,
        "announcement.delete",
        &row,
        format!("Deleted {}", quoted(&row.message)),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Approval hooks, called by routes/approvals.rs inside its transaction

/// Locks the announcement an approval is about. It must still be waiting for a decision, and the approval must be its current one: a lead's edit withdraws the old approval and opens a new one, and deciding on the old one would publish an out-of-date version.
async fn lock_submission(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    approver: &Viewer,
    decision: ApprovalStatus,
) -> AppResult<Row> {
    if approval.subject_type != SubjectType::Announcement {
        return Err(AppError::Internal(anyhow::anyhow!(
            "approval {} is about a {}, not an announcement",
            approval.id,
            approval.subject_type
        )));
    }
    if !permissions::can_approve_for(approver, approval.committee) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can decide on its announcements.",
            approval.committee.name()
        )));
    }
    let row = find(conn, approval.subject_id, true)
        .await?
        .ok_or_else(|| AppError::not_found("announcement"))?;
    if row.state != AnnouncementState::PendingApproval {
        return Err(AppError::Conflict(
            "This announcement is no longer waiting for approval.".into(),
        ));
    }
    if !is_current_submission(conn, approval, decision).await? {
        return Err(AppError::Conflict(
            "The lead changed this announcement after sending it. Open the latest version to decide on it."
                .into(),
        ));
    }
    Ok(row)
}

/// Whether the approval is the announcement's open submission. routes/approvals.rs may record its decision before or after calling the hook, so the decision being made counts as open too.
async fn is_current_submission(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    decision: ApprovalStatus,
) -> AppResult<bool> {
    let open = |status: ApprovalStatus| status == ApprovalStatus::Waiting || status == decision;
    if !open(approval.status) {
        return Ok(false);
    }
    let (stored, superseded): (Option<ApprovalStatus>, bool) = sqlx::query_as(
        "SELECT (SELECT status FROM approvals WHERE id = $1),
                EXISTS (SELECT 1 FROM approvals
                        WHERE subject_type = $2 AND subject_id = $3 AND id <> $1 AND status = 'waiting')",
    )
    .bind(approval.id)
    .bind(SubjectType::Announcement)
    .bind(approval.subject_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(stored.is_some_and(open) && !superseded)
}

/// Called by routes/approvals.rs, inside its transaction, when an approver approves a submitted announcement. Publishes exactly the approval's snapshot.
pub async fn apply_approval(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    approver: &Viewer,
) -> AppResult<()> {
    let row = lock_submission(conn, approval, approver, ApprovalStatus::Approved).await?;
    let now = Utc::now();
    let snapshot: AnnouncementInput = serde_json::from_value(approval.snapshot.clone())?;
    if approval_too_late(snapshot.expires_at, now) {
        return Err(AppError::field(
            "expiresAt",
            "This announcement leaves the ticker in less than 5 minutes, or already has. Send it back and ask the lead to pick a later time.",
        ));
    }
    let valid = validate(&snapshot, now)?;
    lock_live_set(conn).await?;
    ensure_free_slot(conn, now, "approve this one").await?;
    let position = next_position(conn, now).await?;
    let updated = sqlx::query_as::<_, Row>(&format!(
        "UPDATE announcements
         SET kind = $2, committee = $3, message = $4, link_url = $5, expires_at = $6,
             state = 'live', starts_at = $7, approved_by = $8, position = $9, updated_at = now()
         WHERE id = $1
         RETURNING {}",
        Row::COLUMNS
    ))
    .bind(row.id)
    .bind(valid.kind)
    .bind(valid.committee)
    .bind(&valid.message)
    .bind(&valid.link_url)
    .bind(valid.expires_at)
    .bind(now)
    .bind(approver.user_id)
    .bind(position)
    .fetch_one(&mut *conn)
    .await?;
    log(
        conn,
        approver,
        "announcement.approve",
        &updated,
        format!("Approved {} for the ticker", quoted(&updated.message)),
    )
    .await
}

/// Called by routes/approvals.rs, inside its transaction, when an approver sends a submitted announcement back with a note. Sets the announcement back to `sent_back` so the lead can edit and resubmit it.
pub async fn apply_send_back(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    approver: &Viewer,
) -> AppResult<()> {
    let row = lock_submission(conn, approval, approver, ApprovalStatus::SentBack).await?;
    let updated = sqlx::query_as::<_, Row>(&format!(
        "UPDATE announcements SET state = 'sent_back', updated_at = now() WHERE id = $1 RETURNING {}",
        Row::COLUMNS
    ))
    .bind(row.id)
    .fetch_one(&mut *conn)
    .await?;
    log(
        conn,
        approver,
        "announcement.send_back",
        &updated,
        format!("Sent {} back for changes", quoted(&updated.message)),
    )
    .await
}
