//! "Needs attention": what the viewer should act on, each with one action, in the order the contract gives: approvals waiting for them, their own submissions sent back, announcements about to leave the ticker, pages with unpublished changes, projects missing a cover, summary or links, and pages not published for 90 days.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::{Scope, when};
use crate::approvals_core::SubjectType;
use crate::error::AppResult;
use crate::models::{Committee, PersonRef};

/// Mirrors `AttentionItem` in types.ts.
#[derive(Debug, Serialize)]
pub struct AttentionItem {
    kind: Kind,
    severity: Severity,
    title: String,
    detail: String,
    action: Action,
    at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Approval,
    SentBack,
    Expiring,
    UnpublishedPage,
    ProjectIncomplete,
    StalePage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Severity {
    Info,
    Warning,
    Neutral,
}

#[derive(Debug, Serialize)]
struct Action {
    label: &'static str,
    href: String,
}

impl AttentionItem {
    fn new(kind: Kind, title: String, detail: String, action: Action, at: DateTime<Utc>) -> Self {
        let severity = match kind {
            Kind::Approval | Kind::UnpublishedPage => Severity::Info,
            Kind::SentBack | Kind::Expiring => Severity::Warning,
            Kind::ProjectIncomplete | Kind::StalePage => Severity::Neutral,
        };
        Self {
            kind,
            severity,
            title,
            detail,
            action,
            at,
        }
    }
}

/// How long before it leaves the ticker an announcement needs attention.
const EXPIRING_WITHIN: &str = "3 days";
/// How long a published page may go without being published again.
const STALE_AFTER: &str = "90 days";

pub async fn collect(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
) -> AppResult<Vec<AttentionItem>> {
    let mut items = waiting_approvals(conn, scope, now).await?;
    items.extend(sent_back(conn, scope).await?);
    items.extend(expiring(conn, scope, now).await?);
    let (unpublished, changed_pages) = unpublished_pages(conn, scope, now).await?;
    items.extend(unpublished);
    items.extend(incomplete_projects(conn, scope, now).await?);
    items.extend(stale_pages(conn, scope, now, &changed_pages).await?);
    Ok(items)
}

/// A person by name, or "you" for the viewer.
fn who(
    scope: &Scope,
    id: Option<Uuid>,
    andrew_id: Option<String>,
    name: Option<String>,
) -> Option<String> {
    let (id, andrew_id) = (id?, andrew_id?);
    if id == scope.user_id {
        return Some("you".into());
    }
    Some(PersonRef::new(id, andrew_id, name.unwrap_or_default()).name)
}

fn quoted(text: &str) -> String {
    format!("\u{201c}{}\u{201d}", text.trim())
}

// ---------------------------------------------------------------------------
// Approvals waiting for the viewer

#[derive(FromRow)]
struct WaitingRow {
    id: Uuid,
    subject_type: SubjectType,
    created_at: DateTime<Utc>,
    project_name: Option<String>,
    submitter_id: Option<Uuid>,
    submitter_andrew_id: Option<String>,
    submitter_name: Option<String>,
}

async fn waiting_approvals(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
) -> AppResult<Vec<AttentionItem>> {
    let rows = sqlx::query_as::<_, WaitingRow>(
        "SELECT a.id, a.subject_type, a.created_at, a.snapshot->>'name' AS project_name,
                u.id AS submitter_id, u.andrew_id AS submitter_andrew_id, u.name AS submitter_name
         FROM approvals a LEFT JOIN users u ON u.id = a.submitted_by
         WHERE a.status = 'waiting' AND a.committee = ANY($1)
         ORDER BY a.created_at, a.id",
    )
    .bind(&scope.approves_for)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let submitter = match (row.submitter_id, row.submitter_andrew_id) {
                (Some(id), Some(andrew_id)) => {
                    PersonRef::new(id, andrew_id, row.submitter_name.unwrap_or_default()).name
                }
                _ => "A lead".into(),
            };
            let title = match row.subject_type {
                SubjectType::Announcement => format!("{submitter} wants to post to the ticker"),
                SubjectType::Project => format!(
                    "{submitter} wants to publish {}",
                    row.project_name.as_deref().unwrap_or("a project")
                ),
            };
            AttentionItem::new(
                Kind::Approval,
                title,
                format!(
                    "Waiting for your approval since {}",
                    when::day_or_clock(row.created_at, now)
                ),
                Action {
                    label: "Review",
                    href: format!("/portal/?review={}", row.id),
                },
                row.created_at,
            )
        })
        .collect())
}

// ---------------------------------------------------------------------------
// The viewer's own submissions that were sent back

#[derive(FromRow)]
struct SentBackRow {
    subject_type: SubjectType,
    subject_id: Uuid,
    note: Option<String>,
    decided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    message: Option<String>,
    project_name: Option<String>,
    decider_id: Option<Uuid>,
    decider_andrew_id: Option<String>,
    decider_name: Option<String>,
}

/// Only the latest submission for each subject counts, and only while the subject is still there to edit.
async fn sent_back(conn: &mut PgConnection, scope: &Scope) -> AppResult<Vec<AttentionItem>> {
    let rows = sqlx::query_as::<_, SentBackRow>(
        "SELECT a.subject_type, a.subject_id, a.note, a.decided_at, a.created_at,
                a.snapshot->>'message' AS message, a.snapshot->>'name' AS project_name,
                d.id AS decider_id, d.andrew_id AS decider_andrew_id, d.name AS decider_name
         FROM approvals a LEFT JOIN users d ON d.id = a.decided_by
         WHERE a.submitted_by = $1 AND a.status = 'sent_back'
           AND NOT EXISTS (
               SELECT 1 FROM approvals newer
               WHERE newer.subject_type = a.subject_type AND newer.subject_id = a.subject_id
                 AND newer.created_at > a.created_at)
           AND CASE a.subject_type
                 WHEN 'announcement' THEN EXISTS (
                     SELECT 1 FROM announcements n WHERE n.id = a.subject_id AND n.state = 'sent_back')
                 ELSE EXISTS (
                     SELECT 1 FROM projects p WHERE p.id = a.subject_id AND p.state <> 'archived')
               END
         ORDER BY a.decided_at DESC NULLS LAST, a.id",
    )
    .bind(scope.user_id)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let decider = who(
                scope,
                row.decider_id,
                row.decider_andrew_id,
                row.decider_name,
            )
            .unwrap_or_else(|| "A director".into());
            let (title, href) = match row.subject_type {
                SubjectType::Announcement => (
                    format!(
                        "{decider} sent back {}",
                        row.message
                            .as_deref()
                            .map_or_else(|| "your announcement".into(), quoted)
                    ),
                    format!("/portal/ticker/{}/", row.subject_id),
                ),
                SubjectType::Project => (
                    format!(
                        "{decider} sent back your changes to {}",
                        row.project_name.as_deref().unwrap_or("a project")
                    ),
                    format!("/portal/projects/{}/", row.subject_id),
                ),
            };
            let detail = row.note.filter(|note| !note.trim().is_empty()).map_or_else(
                || "Edit it and send it again.".into(),
                |note| format!("Note: {note}"),
            );
            AttentionItem::new(
                Kind::SentBack,
                title,
                detail,
                Action {
                    label: "Edit",
                    href,
                },
                row.decided_at.unwrap_or(row.created_at),
            )
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Announcements leaving the ticker soon

#[derive(FromRow)]
struct ExpiringRow {
    id: Uuid,
    message: String,
    expires_at: DateTime<Utc>,
    poster_id: Option<Uuid>,
    poster_andrew_id: Option<String>,
    poster_name: Option<String>,
}

async fn expiring(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
) -> AppResult<Vec<AttentionItem>> {
    let rows = sqlx::query_as::<_, ExpiringRow>(&format!(
        "SELECT n.id, n.message, n.expires_at,
                u.id AS poster_id, u.andrew_id AS poster_andrew_id, u.name AS poster_name
         FROM announcements n LEFT JOIN users u ON u.id = n.created_by
         WHERE n.state = 'live' AND n.expires_at > $2
           AND n.expires_at <= $2 + interval '{EXPIRING_WITHIN}'
           AND n.committee = ANY($1)
         ORDER BY n.expires_at, n.id"
    ))
    .bind(&scope.ticker_sources)
    .bind(now)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let detail = who(scope, row.poster_id, row.poster_andrew_id, row.poster_name)
                .map_or_else(
                    || "Ticker".into(),
                    |poster| format!("Ticker, posted by {poster}"),
                );
            AttentionItem::new(
                Kind::Expiring,
                format!(
                    "{} leaves the ticker {}",
                    quoted(&row.message),
                    when::relative(row.expires_at, now)
                ),
                detail,
                Action {
                    label: "Extend",
                    href: format!("/portal/ticker/?highlight={}", row.id),
                },
                row.expires_at,
            )
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Committee pages

#[derive(FromRow)]
struct ChangedPageRow {
    committee: Committee,
    draft_updated_at: DateTime<Utc>,
    never_published: bool,
    editor_id: Option<Uuid>,
    editor_andrew_id: Option<String>,
    editor_name: Option<String>,
}

fn page_href(committee: Committee) -> String {
    format!("/portal/committees/{}/", committee.slug())
}

/// Pages the viewer edits whose draft is not what visitors see, and which they are.
async fn unpublished_pages(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
) -> AppResult<(Vec<AttentionItem>, Vec<Committee>)> {
    let rows = sqlx::query_as::<_, ChangedPageRow>(
        "SELECT cp.committee, cp.draft_updated_at, cp.published IS NULL AS never_published,
                u.id AS editor_id, u.andrew_id AS editor_andrew_id, u.name AS editor_name
         FROM committee_pages cp
         JOIN committees c ON c.slug = cp.committee
         LEFT JOIN users u ON u.id = cp.draft_updated_by
         WHERE cp.committee = ANY($1) AND cp.draft IS DISTINCT FROM cp.published
         ORDER BY c.sort_order",
    )
    .bind(&scope.pages)
    .fetch_all(conn)
    .await?;
    let changed = rows.iter().map(|row| row.committee).collect();
    let items = rows
        .into_iter()
        .map(|row| {
            let name = row.committee.name();
            let title = if row.never_published {
                format!("The {name} page has not been published yet")
            } else {
                format!("The {name} page has changes that are not published")
            };
            let edited = when::relative(row.draft_updated_at, now);
            let detail = match who(scope, row.editor_id, row.editor_andrew_id, row.editor_name) {
                Some(editor) => format!("Committee pages, edited by {editor} {edited}"),
                None => format!("Committee pages, edited {edited}"),
            };
            AttentionItem::new(
                Kind::UnpublishedPage,
                title,
                detail,
                Action {
                    label: "Review",
                    href: page_href(row.committee),
                },
                row.draft_updated_at,
            )
        })
        .collect();
    Ok((items, changed))
}

#[derive(FromRow)]
struct StalePageRow {
    committee: Committee,
    published_at: DateTime<Utc>,
    publisher_id: Option<Uuid>,
    publisher_andrew_id: Option<String>,
    publisher_name: Option<String>,
}

/// Published pages the viewer edits that nobody has published for a long time. Pages already listed for their unpublished changes are left out.
async fn stale_pages(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
    already_listed: &[Committee],
) -> AppResult<Vec<AttentionItem>> {
    let rows = sqlx::query_as::<_, StalePageRow>(&format!(
        "SELECT cp.committee, cp.published_at,
                u.id AS publisher_id, u.andrew_id AS publisher_andrew_id, u.name AS publisher_name
         FROM committee_pages cp
         JOIN committees c ON c.slug = cp.committee
         LEFT JOIN users u ON u.id = cp.published_by
         WHERE cp.committee = ANY($1) AND cp.published IS NOT NULL
           AND cp.published_at < $2 - interval '{STALE_AFTER}'
         ORDER BY c.sort_order"
    ))
    .bind(&scope.pages)
    .bind(now)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|row| !already_listed.contains(&row.committee))
        .map(|row| {
            let detail = who(
                scope,
                row.publisher_id,
                row.publisher_andrew_id,
                row.publisher_name,
            )
            .map_or_else(
                || "Committee pages".into(),
                |publisher| format!("Committee pages, last published by {publisher}"),
            );
            AttentionItem::new(
                Kind::StalePage,
                format!(
                    "The {} page has not changed since {}",
                    row.committee.name(),
                    when::month(row.published_at, now)
                ),
                detail,
                Action {
                    label: "Open",
                    href: page_href(row.committee),
                },
                row.published_at,
            )
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Projects missing what visitors need

#[derive(FromRow)]
struct ProjectRow {
    id: Uuid,
    state: String,
    name: String,
    created_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    no_cover: bool,
    no_summary: bool,
    no_links: bool,
}

/// What a project is missing, in the order the editor asks for it, with the action that fixes the first.
fn missing(row: &ProjectRow) -> Option<(String, &'static str)> {
    let parts: Vec<(&str, &'static str)> = [
        (row.no_cover, "cover image", "Add image"),
        (row.no_summary, "summary", "Add summary"),
        (row.no_links, "links", "Add link"),
    ]
    .into_iter()
    .filter(|(is_missing, ..)| *is_missing)
    .map(|(_, what, action)| (what, action))
    .collect();
    let (_, action) = *parts.first()?;
    let names: Vec<&str> = parts.iter().map(|(what, _)| *what).collect();
    let list = match names.as_slice() {
        [only] => (*only).to_owned(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
        [] => return None,
    };
    Some((list, action))
}

/// Projects in the viewer's scope whose draft has no cover, no summary, or neither a site nor a repository link.
async fn incomplete_projects(
    conn: &mut PgConnection,
    scope: &Scope,
    now: DateTime<Utc>,
) -> AppResult<Vec<AttentionItem>> {
    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT p.id, p.state, p.created_at, p.published_at, p.updated_at,
                coalesce(nullif(btrim(p.draft->>'name'), ''), p.slug) AS name,
                CASE WHEN jsonb_typeof(p.draft->'media') = 'array'
                     THEN jsonb_array_length(p.draft->'media') = 0 ELSE true END AS no_cover,
                coalesce(btrim(p.draft->>'summary'), '') = '' AS no_summary,
                coalesce(p.draft->>'siteUrl', '') = '' AND coalesce(p.draft->>'repoUrl', '') = '' AS no_links
         FROM projects p
         WHERE p.state <> 'archived' AND ($1 OR p.committee = ANY($2) OR p.id = ANY($3))
         ORDER BY lower(coalesce(nullif(btrim(p.draft->>'name'), ''), p.slug)), p.id",
    )
    .bind(scope.admin)
    .bind(&scope.project_committees)
    .bind(&scope.lead_projects)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let (what, label) = missing(&row)?;
            let detail = match row.published_at.filter(|_| row.state == "published") {
                Some(published) => format!("Projects, published {}", when::day(published, now)),
                None => format!("Projects, draft since {}", when::day(row.created_at, now)),
            };
            Some(AttentionItem::new(
                Kind::ProjectIncomplete,
                format!("{} has no {what}", row.name),
                detail,
                Action {
                    label,
                    href: format!("/portal/projects/{}/", row.id),
                },
                row.updated_at,
            ))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(no_cover: bool, no_summary: bool, no_links: bool) -> ProjectRow {
        ProjectRow {
            id: Uuid::now_v7(),
            state: "draft".into(),
            name: "Terrier".into(),
            created_at: Utc::now(),
            published_at: None,
            updated_at: Utc::now(),
            no_cover,
            no_summary,
            no_links,
        }
    }

    #[test]
    fn missing_parts_are_listed_with_one_action() {
        assert_eq!(missing(&project(false, false, false)), None);
        assert_eq!(
            missing(&project(true, false, false)),
            Some(("cover image".into(), "Add image"))
        );
        assert_eq!(
            missing(&project(false, true, true)),
            Some(("summary or links".into(), "Add summary"))
        );
        assert_eq!(
            missing(&project(true, true, true)),
            Some(("cover image, summary or links".into(), "Add image"))
        );
    }
}
