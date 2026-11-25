//! The audit log behind "Recent activity". Write an entry in the same transaction as the change.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgConnection, PgExecutor};
use uuid::Uuid;

use crate::auth::Viewer;
use crate::error::AppResult;
use crate::models::{Committee, PersonRef};
use crate::permissions::{self, PeopleScope};

/// `subject_type` values, so activity links know where to point.
pub mod subject {
    pub const ANNOUNCEMENT: &str = "announcement";
    pub const COMMITTEE_PAGE: &str = "committee_page";
    pub const PROJECT: &str = "project";
    pub const PERSON: &str = "person";
    pub const APPROVAL: &str = "approval";
}

/// One change, described for people, for example "Published the Tech page".
pub struct Entry<'a> {
    pub actor: Option<Uuid>,
    /// Machine name such as `announcement.publish`.
    pub action: &'a str,
    pub subject_type: &'a str,
    pub subject_id: Option<String>,
    pub committee: Option<Committee>,
    pub summary: String,
}

pub async fn record(conn: &mut PgConnection, entry: Entry<'_>) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, subject_type, subject_id, committee, summary)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(entry.actor)
    .bind(entry.action)
    .bind(entry.subject_type)
    .bind(entry.subject_id)
    .bind(entry.committee)
    .bind(entry.summary)
    .execute(conn)
    .await?;
    Ok(())
}

/// One row of "Recent activity". Mirrors `ActivityEntry` in types.ts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub id: Uuid,
    pub actor: Option<PersonRef>,
    pub summary: String,
    pub at: DateTime<Utc>,
    pub href: Option<String>,
}

pub const DEFAULT_ACTIVITY: i64 = 10;
pub const MAX_ACTIVITY: i64 = 50;

#[derive(FromRow)]
struct ActivityRow {
    id: Uuid,
    summary: String,
    created_at: DateTime<Utc>,
    subject_type: String,
    subject_id: Option<String>,
    committee: Option<Committee>,
    actor_id: Option<Uuid>,
    actor_andrew_id: Option<String>,
    actor_name: Option<String>,
}

/// The newest entries the viewer may see, newest first: everything for admins; otherwise their own actions, and entries about their committees that name only what they may open (see [`permissions::activity_scope`]). `limit` is clamped to 1 through 50.
/// Entries link only to pages the viewer can open.
pub async fn recent<'e>(
    db: impl PgExecutor<'e>,
    viewer: &Viewer,
    limit: i64,
) -> AppResult<Vec<ActivityEntry>> {
    let scope = permissions::activity_scope(viewer);
    let slugs = |list: &[Committee]| list.iter().map(|c| c.slug()).collect::<Vec<_>>();
    let projects: Vec<String> = scope
        .projects
        .iter()
        .flatten()
        .map(Uuid::to_string)
        .collect();
    // Each branch reads its own index and stops at `limit`, instead of one filter over the whole log.
    // In committees the viewer only belongs to, an announcement shows while it is not waiting for approval or is the viewer's own (they manage no source there), and a project only when the viewer may open it.
    let rows = sqlx::query_as::<_, ActivityRow>(
        "WITH picked AS (
             (SELECT a.* FROM audit_log a WHERE $1
              ORDER BY a.created_at DESC, a.id DESC LIMIT $7)
             UNION
             (SELECT a.* FROM audit_log a WHERE a.actor_id = $3
              ORDER BY a.created_at DESC, a.id DESC LIMIT $7)
             UNION
             (SELECT a.* FROM audit_log a WHERE a.committee = ANY($2)
              ORDER BY a.created_at DESC, a.id DESC LIMIT $7)
             UNION
             (SELECT a.* FROM audit_log a
              WHERE a.committee = ANY($4)
                AND CASE a.subject_type
                      WHEN 'announcement' THEN EXISTS (
                          SELECT 1 FROM announcements n
                          WHERE n.id = CASE WHEN a.subject_id ~* $8 THEN a.subject_id::uuid END
                            AND (n.state NOT IN ('pending_approval', 'sent_back')
                                 OR n.created_by = $3))
                      WHEN 'project' THEN $5 OR a.subject_id = ANY($6)
                      ELSE true
                    END
              ORDER BY a.created_at DESC, a.id DESC LIMIT $7)
         )
         SELECT a.id, a.summary, a.created_at, a.subject_type, a.subject_id, a.committee,
                u.id AS actor_id, u.andrew_id AS actor_andrew_id, u.name AS actor_name
         FROM picked a LEFT JOIN users u ON u.id = a.actor_id
         ORDER BY a.created_at DESC, a.id DESC
         LIMIT $7",
    )
    .bind(scope.everything)
    .bind(slugs(&scope.committees))
    .bind(viewer.user_id)
    .bind(slugs(&scope.limited_committees))
    .bind(scope.projects.is_none())
    .bind(&projects)
    .bind(limit.clamp(1, MAX_ACTIVITY))
    .bind(UUID_PATTERN)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|row| row.into_entry(viewer)).collect())
}

/// A UUID in its usual text form, so `subject_id` is cast only when it is one.
const UUID_PATTERN: &str = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$";

impl ActivityRow {
    fn into_entry(self, viewer: &Viewer) -> ActivityEntry {
        let actor = match (self.actor_id, self.actor_andrew_id) {
            (Some(id), Some(andrew_id)) => Some(PersonRef::new(
                id,
                andrew_id,
                self.actor_name.unwrap_or_default(),
            )),
            _ => None,
        };
        ActivityEntry {
            href: href_for(
                viewer,
                &self.subject_type,
                self.subject_id.as_deref(),
                self.committee,
            ),
            id: self.id,
            actor,
            summary: self.summary,
            at: self.created_at,
        }
    }
}

/// [`href`], when the viewer may open that page. Everyone in the portal can open the ticker, committee pages (read only) and the overview; people and projects are narrower.
fn href_for(
    viewer: &Viewer,
    subject_type: &str,
    subject_id: Option<&str>,
    committee: Option<Committee>,
) -> Option<String> {
    let can_open = match subject_type {
        subject::PERSON => permissions::people_scope(viewer) != PeopleScope::None,
        subject::PROJECT => subject_id
            .and_then(|id| Uuid::parse_str(id).ok())
            .is_some_and(|id| permissions::can_view_project(viewer, id)),
        _ => true,
    };
    can_open
        .then(|| href(subject_type, subject_id, committee))
        .flatten()
}

/// The portal page that shows an entry's subject, where there is one, whoever is looking.
/// Activity lists use [`recent`], which also checks that the viewer can open it.
pub fn href(
    subject_type: &str,
    subject_id: Option<&str>,
    committee: Option<Committee>,
) -> Option<String> {
    match subject_type {
        subject::ANNOUNCEMENT => Some("/portal/ticker/".into()),
        subject::COMMITTEE_PAGE => committee
            .or_else(|| subject_id.and_then(Committee::from_slug))
            .map(|c| format!("/portal/committees/{}/", c.slug())),
        subject::PROJECT => subject_id
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(|id| format!("/portal/projects/{id}/")),
        subject::PERSON => Some("/portal/people/".into()),
        subject::APPROVAL => Some("/portal/".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_point_at_portal_pages() {
        let project = Uuid::now_v7();
        assert_eq!(
            href(subject::PROJECT, Some(&project.to_string()), None),
            Some(format!("/portal/projects/{project}/"))
        );
        assert_eq!(href(subject::PROJECT, Some("../evil"), None), None);
        assert_eq!(
            href(subject::COMMITTEE_PAGE, None, Some(Committee::Tech)),
            Some("/portal/committees/tech/".into())
        );
        assert_eq!(
            href(subject::COMMITTEE_PAGE, Some("design"), None),
            Some("/portal/committees/design/".into())
        );
        assert_eq!(
            href(subject::ANNOUNCEMENT, Some("x"), None),
            Some("/portal/ticker/".into())
        );
        assert_eq!(href("something_else", None, None), None);
    }

    #[test]
    fn links_need_a_page_the_viewer_can_open() {
        use crate::models::Role;
        use crate::permissions::tests::viewer;

        let nova = Uuid::now_v7().to_string();
        let other = Uuid::now_v7().to_string();
        let events = Some(Committee::Events);
        let lead = viewer(
            &[(Role::Lead, events, true)],
            vec![Uuid::parse_str(&nova).unwrap()],
        );
        let director = viewer(&[(Role::Director, events, true)], vec![]);

        assert_eq!(href_for(&lead, subject::PERSON, None, events), None);
        assert_eq!(
            href_for(&director, subject::PERSON, None, events),
            Some("/portal/people/".into())
        );
        assert_eq!(
            href_for(&lead, subject::PROJECT, Some(&nova), events),
            Some(format!("/portal/projects/{nova}/"))
        );
        assert_eq!(
            href_for(&lead, subject::PROJECT, Some(&other), events),
            None
        );
        assert_eq!(
            href_for(&director, subject::PROJECT, Some(&other), events),
            Some(format!("/portal/projects/{other}/"))
        );
        assert_eq!(
            href_for(&lead, subject::COMMITTEE_PAGE, None, events),
            Some("/portal/committees/events/".into())
        );
        assert_eq!(
            href_for(&lead, subject::ANNOUNCEMENT, None, events),
            Some("/portal/ticker/".into())
        );
    }
}
