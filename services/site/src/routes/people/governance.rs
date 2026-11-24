//! POST /api/people/sync-governance: makes each committee director listed in governance's `data/committees.yaml` a director of that committee. It never makes anyone an admin.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{clear_lead_projects, display_name, find_or_create_user, no_store, record};
use crate::auth::{PortalViewer, Viewer, normalize_andrew_id};
use crate::error::{AppError, AppResult};
use crate::models::{Committee, Role};
use crate::permissions;
use crate::state::AppState;

const GOVERNANCE_URL: &str =
    "https://git.cmu.dev/ScottyLabs/governance/raw/branch/main/data/committees.yaml";
const MAX_GOVERNANCE_BYTES: usize = 1024 * 1024;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceSyncResponse {
    added: u32,
    updated: u32,
    unchanged: u32,
    messages: Vec<String>,
}

#[derive(Deserialize)]
struct GovernanceFile {
    committees: BTreeMap<String, GovernanceCommittee>,
}

#[derive(Deserialize)]
struct GovernanceCommittee {
    #[serde(default)]
    director: Option<String>,
}

/// What governance lists: each committee's director in the site's committee order, and notes about entries that were skipped.
pub struct Listed {
    pub directors: Vec<(Committee, String)>,
    pub notes: Vec<String>,
}

/// Reads `data/committees.yaml`. The organization director is not a committee director and is ignored; governance never makes anyone an admin.
pub fn parse_directors(yaml: &str) -> Result<Listed, String> {
    let file: GovernanceFile = serde_norway::from_str(yaml).map_err(|error| error.to_string())?;
    let mut directors = Vec::new();
    let mut messages = Vec::new();
    for (key, entry) in &file.committees {
        let Some(committee) = Committee::from_slug(key) else {
            messages.push(format!(
                "Skipped \u{201c}{key}\u{201d}: the site has no committee by that name."
            ));
            continue;
        };
        match entry.director.as_deref().map(normalize_andrew_id) {
            Some(Some(andrew_id)) => directors.push((committee, andrew_id)),
            Some(None) => messages.push(format!(
                "Skipped {}: its director is not a valid Andrew ID.",
                committee.name()
            )),
            None => messages.push(format!(
                "Governance lists no director for {}.",
                committee.name()
            )),
        }
    }
    directors.sort_by_key(|(committee, _)| *committee);
    Ok(Listed {
        directors,
        notes: messages,
    })
}

async fn fetch_governance(http: &reqwest::Client) -> AppResult<String> {
    let unreachable = || {
        AppError::BadRequest(
            "We couldn't reach the governance repository. Try again in a few minutes.".into(),
        )
    };
    let mut response = http.get(GOVERNANCE_URL).send().await.map_err(|error| {
        tracing::warn!(error = %error, "governance fetch failed");
        unreachable()
    })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "governance fetch returned an error");
        return Err(unreachable());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unreachable())? {
        if body.len() + chunk.len() > MAX_GOVERNANCE_BYTES {
            return Err(AppError::BadRequest(
                "The governance committees file is larger than expected, so it was not read."
                    .into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).map_err(|_| {
        AppError::BadRequest("The governance committees file is not valid text.".into())
    })
}

/// Makes each listed director a director of their committee (source `governance`), and removes directors that an earlier sync added but governance no longer lists.
pub async fn apply_directors(
    conn: &mut PgConnection,
    viewer: &Viewer,
    directors: &[(Committee, String)],
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    for (committee, andrew_id) in directors {
        let (user_id, name) = find_or_create_user(conn, andrew_id, None).await?;
        let shown = display_name(&name, andrew_id);
        let existing: Option<(Uuid, Role)> = sqlx::query_as(
            "SELECT id, role FROM role_assignments WHERE user_id = $1 AND committee = $2 FOR UPDATE",
        )
        .bind(user_id)
        .bind(committee)
        .fetch_optional(&mut *conn)
        .await?;
        match existing {
            Some((_, Role::Director)) => report.unchanged += 1,
            Some((assignment, _)) => {
                sqlx::query(
                    "UPDATE role_assignments
                     SET role = 'director', can_post_ticker = true, appointed_by = NULL,
                         source = 'governance', created_at = now()
                     WHERE id = $1",
                )
                .bind(assignment)
                .execute(&mut *conn)
                .await?;
                clear_lead_projects(conn, user_id, *committee).await?;
                record(
                    conn,
                    Some(viewer),
                    "role.sync",
                    user_id,
                    Some(*committee),
                    format!(
                        "Made {shown} director of {} from governance",
                        committee.name()
                    ),
                )
                .await?;
                report.updated += 1;
            }
            None => {
                sqlx::query(
                    "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, source)
                     VALUES ($1, $2, 'director', $3, true, 'governance')",
                )
                .bind(Uuid::now_v7())
                .bind(user_id)
                .bind(committee)
                .execute(&mut *conn)
                .await?;
                record(
                    conn,
                    Some(viewer),
                    "role.sync",
                    user_id,
                    Some(*committee),
                    format!(
                        "Made {shown} director of {} from governance",
                        committee.name()
                    ),
                )
                .await?;
                report.added += 1;
            }
        }

        let replaced: Vec<(Uuid, String, String)> = sqlx::query_as(
            "DELETE FROM role_assignments r USING users u
             WHERE u.id = r.user_id AND r.role = 'director' AND r.committee = $1
               AND r.source = 'governance' AND r.user_id <> $2
             RETURNING r.user_id, u.andrew_id, u.name",
        )
        .bind(committee)
        .bind(user_id)
        .fetch_all(&mut *conn)
        .await?;
        for (old_id, old_andrew_id, old_name) in replaced {
            let old = display_name(&old_name, &old_andrew_id);
            record(
                conn,
                Some(viewer),
                "role.sync",
                old_id,
                Some(*committee),
                format!(
                    "Removed {old} as director of {}; governance lists {shown} now",
                    committee.name()
                ),
            )
            .await?;
            report.messages.push(format!(
                "{old} is no longer director of {}. Governance lists {shown}.",
                committee.name()
            ));
            report.updated += 1;
        }
    }
    Ok(())
}

pub(super) async fn sync(
    State(state): State<AppState>,
    viewer: PortalViewer,
) -> AppResult<Response> {
    if !permissions::can_sync_governance(&viewer) {
        return Err(AppError::Forbidden(
            "Only admins can sync directors from governance.".into(),
        ));
    }
    let yaml = fetch_governance(&state.http).await?;
    let listed = parse_directors(&yaml).map_err(|error| {
        tracing::warn!(error = %error, "governance committees file did not parse");
        AppError::BadRequest(
            "The governance committees file could not be read. Check data/committees.yaml in governance, then try again.".into(),
        )
    })?;
    let mut report = GovernanceSyncResponse {
        messages: listed.notes,
        ..GovernanceSyncResponse::default()
    };
    let mut tx = state.db.begin().await?;
    apply_directors(&mut tx, &viewer, &listed.directors, &mut report).await?;
    tx.commit().await?;
    Ok(no_store(Json(report).into_response()))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::{self, Req, UserSpec};

    // ---------------------------------------------------------------------------
    // Governance

    const GOVERNANCE_SAMPLE: &str = r#"
    # yaml-language-server: $schema=../schemas/committees.schema.json
    director: tkanz
    committees:
      events:
        name: Events
        director: evanfost
        roles:
          head-of-logistics:
            name: Head of Logistics
            members: [spchoksh, helenf]
      tech:
        name: Tech
        director: apallati
      design:
        name: Design
        director: jeanchen
        teams:
          branding:
            name: Branding
            roles:
              head-of-branding:
                name: Head of Branding
                members: [yelimk2]
      marketing:
        name: Marketing
        director: someone
      finance:
        name: Finance
        director: "Not An ID"
    "#;

    #[test]
    fn parses_directors_from_the_governance_file() {
        let listed = parse_directors(GOVERNANCE_SAMPLE).unwrap();
        assert_eq!(
            listed.directors,
            [
                (Committee::Events, "evanfost".to_owned()),
                (Committee::Tech, "apallati".to_owned()),
                (Committee::Design, "jeanchen".to_owned()),
            ]
        );
        assert_eq!(listed.notes.len(), 2);
        assert!(listed.notes.iter().any(|m| m.contains("marketing")));
        assert!(listed.notes.iter().any(|m| m.contains("Finance")));
        assert!(parse_directors("committees: [1, 2]").is_err());
        assert!(parse_directors(": : :").is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn governance_sync_adds_updates_and_replaces_directors(db: PgPool) {
        testing::ensure_committees(&db).await;
        let admin = testing::user(&db, "govadmin", &[(Role::Admin, None)]).await;
        // Already director of Events by hand: unchanged.
        testing::user(
            &db,
            "evanfost",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        // A lead in Design who governance now lists as director: upgraded.
        let projects = testing::project(&db, Committee::Design, "brand-kit").await;
        UserSpec::new("jeanchen")
            .lead(Committee::Design, true)
            .edits(projects)
            .create(&db)
            .await;
        // A director of Tech from an earlier sync whom governance no longer lists: removed.
        let old = testing::user(&db, "oldtech", &[]).await;
        sqlx::query("INSERT INTO role_assignments (id, user_id, role, committee, source) VALUES ($1, $2, 'director', 'tech', 'governance')")
            .bind(Uuid::now_v7())
            .bind(old.id)
            .execute(&db)
            .await
            .unwrap();

        let viewer = crate::permissions::tests::viewer(&[(Role::Admin, None, true)], vec![]);
        let viewer = Viewer {
            user_id: admin.id,
            ..viewer
        };
        let listed = parse_directors(GOVERNANCE_SAMPLE).unwrap();
        let directors = listed.directors;
        let mut report = GovernanceSyncResponse {
            messages: listed.notes,
            ..GovernanceSyncResponse::default()
        };
        let mut tx = db.begin().await.unwrap();
        apply_directors(&mut tx, &viewer, &directors, &mut report)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!((report.added, report.updated, report.unchanged), (1, 2, 1));
        assert!(
            report
                .messages
                .iter()
                .any(|m| m
                    == "Test oldtech is no longer director of Tech. Governance lists apallati.")
        );

        let roles: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT u.andrew_id, r.role, r.committee, r.source FROM role_assignments r JOIN users u ON u.id = r.user_id ORDER BY u.andrew_id",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            roles,
            [
                (
                    "apallati".into(),
                    "director".into(),
                    Some("tech".into()),
                    "governance".into()
                ),
                (
                    "evanfost".into(),
                    "director".into(),
                    Some("events".into()),
                    "portal".into()
                ),
                ("govadmin".into(), "admin".into(), None, "portal".into()),
                (
                    "jeanchen".into(),
                    "director".into(),
                    Some("design".into()),
                    "governance".into()
                ),
            ]
        );
        // The organization director in governance is never made an admin.
        let tkanz: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE andrew_id = 'tkanz'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(tkanz, 0);
        let editors: i64 = sqlx::query_scalar("SELECT count(*) FROM project_editors")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(editors, 0);
        let audited: i64 =
            sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'role.sync'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(audited, 3);

        // Running it again changes nothing.
        let mut again = GovernanceSyncResponse::default();
        let mut tx = db.begin().await.unwrap();
        apply_directors(&mut tx, &viewer, &directors, &mut again)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!((again.added, again.updated, again.unchanged), (0, 0, 3));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn only_admins_sync_governance(db: PgPool) {
        let app = testing::app(db.clone()).await;
        let director = testing::user(
            &db,
            "govdirector",
            &[(Role::Director, Some(Committee::Events))],
        )
        .await;
        let res = Req::post("/api/people/sync-governance")
            .user(&director)
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN);
    }
}
