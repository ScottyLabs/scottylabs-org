//! Approvals: what leads submitted, for directors and admins to approve or send back. See "Approvals" in docs/implementation/contract.md.
//!
//! A decision locks the approval, checks `permissions::can_approve_for`, and calls the subject's hook (`routes::apply_announcement_approval` and the others) in the same transaction, so the subject and the approval change together or not at all.

use axum::extract::{FromRequest, Path, RawQuery, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::approvals_core::{self, ApprovalRow, ApprovalStatus, SubjectType};
use crate::audit::{self, subject};
use crate::auth::{PortalViewer, Viewer};
use crate::error::{AppError, AppResult};
use crate::json::Json as JsonBody;
use crate::models::{
    Committee, ImageRef, PersonRef, TickerSource, check_https_url, check_video_url,
};
use crate::permissions;
use crate::richtext;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/approvals", get(list))
        .route("/approvals/{id}/approve", post(approve))
        .route("/approvals/{id}/send-back", post(send_back))
}

/// Longest note an approver can leave, in characters.
pub const MAX_NOTE_CHARS: usize = 500;
const WAITING_LIMIT: i64 = 100;
const DECIDED_LIMIT: i64 = 50;

// ---------------------------------------------------------------------------
// What was submitted

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnnouncementKind {
    Event,
    ProjectUpdate,
    Milestone,
    ClubNews,
}

/// Mirrors `AnnouncementInput` in types.ts: the announcement as the lead sent it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnouncementSnapshot {
    kind: AnnouncementKind,
    committee: TickerSource,
    message: String,
    #[serde(default)]
    link_url: Option<String>,
    expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProjectStatus {
    Live,
    InDevelopment,
    Annual,
    Ongoing,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProjectKind {
    App,
    Event,
    Tool,
    Brand,
    Program,
    Fundraiser,
    Other,
}

/// Mirrors `ProjectDoc` in types.ts: the project as the lead asked to publish it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSnapshot {
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    description: String,
    committee: Committee,
    #[serde(default)]
    built_with: Vec<Committee>,
    status: ProjectStatus,
    kind: ProjectKind,
    #[serde(default)]
    site_url: Option<String>,
    #[serde(default)]
    repo_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    #[serde(default)]
    media: Vec<ImageRef>,
}

/// Keeps a stored link only when it is still one the editors would accept, so a bad row can never put a script URL on the page.
fn safe_link(url: Option<String>, check: fn(&str) -> Result<String, String>) -> Option<String> {
    url.and_then(|url| check(&url).ok())
}

impl AnnouncementSnapshot {
    fn cleaned(mut self) -> Self {
        self.link_url = safe_link(self.link_url, check_https_url);
        self
    }
}

impl ProjectSnapshot {
    fn cleaned(mut self) -> Self {
        self.description = richtext::sanitize(&self.description);
        self.site_url = safe_link(self.site_url, check_https_url);
        self.repo_url = safe_link(self.repo_url, check_https_url);
        self.video_url = safe_link(self.video_url, check_video_url);
        self.media.retain(|image| image.check(false).is_ok());
        self
    }
}

/// The snapshot as the announcement or the project it is about. A snapshot that no longer reads as either is left out and logged, so one odd row cannot break the list.
fn parse_snapshot(
    id: Uuid,
    subject_type: SubjectType,
    snapshot: serde_json::Value,
) -> (Option<AnnouncementSnapshot>, Option<ProjectSnapshot>) {
    let parsed = match subject_type {
        SubjectType::Announcement => serde_json::from_value::<AnnouncementSnapshot>(snapshot)
            .map(|s| (Some(s.cleaned()), None)),
        SubjectType::Project => {
            serde_json::from_value::<ProjectSnapshot>(snapshot).map(|s| (None, Some(s.cleaned())))
        }
    };
    parsed.unwrap_or_else(|error| {
        tracing::warn!(%id, %error, "an approval snapshot does not match its subject type");
        (None, None)
    })
}

// ---------------------------------------------------------------------------
// The list

/// Mirrors `Approval` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalDto {
    id: Uuid,
    status: ApprovalStatus,
    note: Option<String>,
    decided_by: Option<PersonRef>,
    decided_at: Option<DateTime<Utc>>,
    subject_type: SubjectType,
    subject_id: Uuid,
    committee: Committee,
    submitted_by: Option<PersonRef>,
    submitted_at: DateTime<Utc>,
    announcement: Option<AnnouncementSnapshot>,
    project: Option<ProjectSnapshot>,
    href: String,
    can_decide: bool,
}

#[derive(Serialize)]
struct ApprovalListResponse {
    approvals: Vec<ApprovalDto>,
}

#[derive(FromRow)]
struct ApprovalListRow {
    id: Uuid,
    subject_type: SubjectType,
    subject_id: Uuid,
    committee: Committee,
    snapshot: serde_json::Value,
    status: ApprovalStatus,
    note: Option<String>,
    decided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    submitter_id: Option<Uuid>,
    submitter_andrew_id: Option<String>,
    submitter_name: Option<String>,
    decider_id: Option<Uuid>,
    decider_andrew_id: Option<String>,
    decider_name: Option<String>,
}

const SELECT_APPROVALS: &str =
    "SELECT a.id, a.subject_type, a.subject_id, a.committee, a.snapshot, a.status, a.note,
        a.decided_at, a.created_at,
        s.id AS submitter_id, s.andrew_id AS submitter_andrew_id, s.name AS submitter_name,
        d.id AS decider_id, d.andrew_id AS decider_andrew_id, d.name AS decider_name
 FROM approvals a
 LEFT JOIN users s ON s.id = a.submitted_by
 LEFT JOIN users d ON d.id = a.decided_by";

fn person(id: Option<Uuid>, andrew_id: Option<String>, name: Option<String>) -> Option<PersonRef> {
    match (id, andrew_id) {
        (Some(id), Some(andrew_id)) => {
            Some(PersonRef::new(id, andrew_id, name.unwrap_or_default()))
        }
        _ => None,
    }
}

/// The portal page that shows what an approval is about.
fn subject_href(subject_type: SubjectType, subject_id: Uuid) -> String {
    match subject_type {
        SubjectType::Announcement => format!("/portal/ticker/{subject_id}/"),
        SubjectType::Project => format!("/portal/projects/{subject_id}/"),
    }
}

impl ApprovalListRow {
    fn into_dto(self, viewer: &Viewer) -> ApprovalDto {
        let (announcement, project) = parse_snapshot(self.id, self.subject_type, self.snapshot);
        ApprovalDto {
            can_decide: self.status == ApprovalStatus::Waiting
                && permissions::can_approve_for(viewer, self.committee),
            href: subject_href(self.subject_type, self.subject_id),
            id: self.id,
            status: self.status,
            note: self.note,
            decided_by: person(self.decider_id, self.decider_andrew_id, self.decider_name),
            decided_at: self.decided_at,
            subject_type: self.subject_type,
            subject_id: self.subject_id,
            committee: self.committee,
            submitted_by: person(
                self.submitter_id,
                self.submitter_andrew_id,
                self.submitter_name,
            ),
            submitted_at: self.created_at,
            announcement,
            project,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusFilter {
    Waiting,
    Decided,
}

#[derive(Debug, PartialEq, Eq)]
struct ListQuery {
    status: StatusFilter,
    mine: bool,
}

/// `?status=waiting|decided` (waiting when left out) and `?mine=true|false`.
fn parse_query(raw: Option<&str>) -> AppResult<ListQuery> {
    let mut query = ListQuery {
        status: StatusFilter::Waiting,
        mine: false,
    };
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match (key.as_ref(), value.trim()) {
            ("status", "waiting" | "") => query.status = StatusFilter::Waiting,
            ("status", "decided") => query.status = StatusFilter::Decided,
            ("status", _) => {
                return Err(AppError::field("status", "Choose waiting or decided."));
            }
            ("mine", "true" | "1") => query.mine = true,
            ("mine", "false" | "0" | "") => query.mine = false,
            ("mine", _) => return Err(AppError::field("mine", "Use mine=true or mine=false.")),
            _ => {}
        }
    }
    Ok(query)
}

async fn list(
    State(state): State<AppState>,
    viewer: PortalViewer,
    RawQuery(raw): RawQuery,
) -> AppResult<Json<ApprovalListResponse>> {
    let query = parse_query(raw.as_deref())?;
    let committees: Vec<&'static str> = permissions::approves_for(&viewer)
        .into_iter()
        .map(Committee::slug)
        .collect();
    if !query.mine && committees.is_empty() {
        return Err(AppError::Forbidden(
            "Only directors and admins review submissions. Your own submissions are listed on the overview."
                .into(),
        ));
    }
    let (statuses, order, limit): (&[&str], &str, i64) = match query.status {
        StatusFilter::Waiting => (&["waiting"], "a.created_at ASC, a.id", WAITING_LIMIT),
        StatusFilter::Decided => (
            &["approved", "sent_back"],
            "a.decided_at DESC NULLS LAST, a.id DESC",
            DECIDED_LIMIT,
        ),
    };
    let submitter = query.mine.then_some(viewer.user_id);
    let rows = sqlx::query_as::<_, ApprovalListRow>(&format!(
        "{SELECT_APPROVALS}
         WHERE a.status = ANY($1)
           AND CASE WHEN $2::uuid IS NULL THEN a.committee = ANY($3) ELSE a.submitted_by = $2 END
         ORDER BY {order}
         LIMIT $4"
    ))
    .bind(statuses)
    .bind(submitter)
    .bind(&committees)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let approvals = rows.into_iter().map(|row| row.into_dto(&viewer)).collect();
    Ok(Json(ApprovalListResponse { approvals }))
}

async fn load_one(conn: &mut PgConnection, viewer: &Viewer, id: Uuid) -> AppResult<ApprovalDto> {
    let row = sqlx::query_as::<_, ApprovalListRow>(&format!("{SELECT_APPROVALS} WHERE a.id = $1"))
        .bind(id)
        .fetch_one(conn)
        .await?;
    Ok(row.into_dto(viewer))
}

// ---------------------------------------------------------------------------
// Deciding

#[derive(Debug, Default, Deserialize)]
struct DecisionInput {
    #[serde(default)]
    note: Option<String>,
}

/// Trims the note and checks its length. `required` is for sending back, where the lead needs to know what to change.
fn check_note(raw: Option<&str>, required: bool) -> AppResult<Option<String>> {
    let note = raw.map(str::trim).unwrap_or_default().replace("\r\n", "\n");
    if note.is_empty() {
        return if required {
            Err(AppError::field(
                "note",
                "Write a note that says what to change.",
            ))
        } else {
            Ok(None)
        };
    }
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::field(
            "note",
            format!("Keep the note to {MAX_NOTE_CHARS} characters or fewer."),
        ));
    }
    if note
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(AppError::field(
            "note",
            "Remove the special characters from the note.",
        ));
    }
    Ok(Some(note))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Approve,
    SendBack,
}

impl Decision {
    fn status(self) -> ApprovalStatus {
        match self {
            Self::Approve => ApprovalStatus::Approved,
            Self::SendBack => ApprovalStatus::SentBack,
        }
    }

    fn action(self) -> &'static str {
        match self {
            Self::Approve => "approval.approve",
            Self::SendBack => "approval.send_back",
        }
    }
}

/// Approving takes an optional `{ "note" }`, so a request without a body is fine.
async fn approve(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    request: Request,
) -> AppResult<Json<ApprovalDto>> {
    let input = if request.headers().contains_key(CONTENT_TYPE) {
        JsonBody::<DecisionInput>::from_request(request, &state)
            .await?
            .0
    } else {
        DecisionInput::default()
    };
    let note = check_note(input.note.as_deref(), false)?;
    decide(&state, &viewer, &id, Decision::Approve, note)
        .await
        .map(Json)
}

async fn send_back(
    State(state): State<AppState>,
    viewer: PortalViewer,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<DecisionInput>,
) -> AppResult<Json<ApprovalDto>> {
    let note = check_note(input.note.as_deref(), true)?;
    decide(&state, &viewer, &id, Decision::SendBack, note)
        .await
        .map(Json)
}

async fn decide(
    state: &AppState,
    viewer: &Viewer,
    raw_id: &str,
    decision: Decision,
    note: Option<String>,
) -> AppResult<ApprovalDto> {
    let id = Uuid::parse_str(raw_id).map_err(|_| AppError::not_found("submission"))?;
    let mut tx = state.db.begin().await?;
    let approval = approvals_core::lock_for_decision(&mut tx, id)
        .await?
        .ok_or_else(|| AppError::not_found("submission"))?;
    if !permissions::can_approve_for(viewer, approval.committee) {
        return Err(AppError::Forbidden(format!(
            "Only admins and the director of {} can approve or send back its submissions.",
            approval.committee.name()
        )));
    }
    require_waiting(&approval)?;
    apply_to_subject(&mut tx, &approval, viewer, decision).await?;
    sqlx::query(
        "UPDATE approvals SET status = $2, note = $3, decided_by = $4, decided_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(decision.status())
    .bind(&note)
    .bind(viewer.user_id)
    .execute(&mut *tx)
    .await?;
    record(&mut tx, viewer, &approval, decision).await?;
    let dto = load_one(&mut tx, viewer, id).await?;
    tx.commit().await?;
    Ok(dto)
}

fn require_waiting(approval: &ApprovalRow) -> AppResult<()> {
    let message = match approval.status {
        ApprovalStatus::Waiting => return Ok(()),
        ApprovalStatus::Approved => "This was already approved.",
        ApprovalStatus::SentBack => "This was already sent back.",
        ApprovalStatus::Withdrawn => {
            "The lead changed or removed this after sending it, so there is nothing to decide here."
        }
    };
    Err(AppError::Conflict(message.into()))
}

/// Publishes or returns the subject through its own module, inside this transaction.
async fn apply_to_subject(
    conn: &mut PgConnection,
    approval: &ApprovalRow,
    viewer: &Viewer,
    decision: Decision,
) -> AppResult<()> {
    match (decision, approval.subject_type) {
        (Decision::Approve, SubjectType::Announcement) => {
            crate::routes::apply_announcement_approval(conn, approval, viewer).await
        }
        (Decision::Approve, SubjectType::Project) => {
            crate::routes::apply_project_approval(conn, approval, viewer).await
        }
        (Decision::SendBack, SubjectType::Announcement) => {
            crate::routes::apply_announcement_send_back(conn, approval, viewer).await
        }
        (Decision::SendBack, SubjectType::Project) => {
            crate::routes::apply_project_send_back(conn, approval, viewer).await
        }
    }
}

/// "Approved Sara Chokshi's announcement", "Sent Sara Chokshi's changes to Nova back".
fn decision_summary(decision: Decision, submitter: Option<&str>, project: Option<&str>) -> String {
    let owner = submitter.map_or_else(
        || "a lead\u{2019}s".to_owned(),
        |name| format!("{name}\u{2019}s"),
    );
    let what = match project {
        Some(name) => format!("{owner} changes to {name}"),
        None => format!("{owner} announcement"),
    };
    match decision {
        Decision::Approve => format!("Approved {what}"),
        Decision::SendBack => format!("Sent {what} back"),
    }
}

async fn record(
    conn: &mut PgConnection,
    viewer: &Viewer,
    approval: &ApprovalRow,
    decision: Decision,
) -> AppResult<()> {
    let submitter: Option<(String, String)> =
        sqlx::query_as("SELECT andrew_id, name FROM users WHERE id = $1")
            .bind(approval.submitted_by)
            .fetch_optional(&mut *conn)
            .await?;
    let submitter =
        submitter.map(|(andrew_id, name)| PersonRef::new(Uuid::nil(), andrew_id, name).name);
    let (subject_type, project) = match approval.subject_type {
        SubjectType::Announcement => (subject::ANNOUNCEMENT, None),
        SubjectType::Project => (
            subject::PROJECT,
            Some(approval.snapshot["name"].as_str().unwrap_or("a project")),
        ),
    };
    audit::record(
        conn,
        audit::Entry {
            actor: Some(viewer.user_id),
            action: decision.action(),
            subject_type,
            subject_id: Some(approval.subject_id.to_string()),
            committee: Some(approval.committee),
            summary: decision_summary(decision, submitter.as_deref(), project),
        },
    )
    .await
}

#[cfg(test)]
mod tests;
