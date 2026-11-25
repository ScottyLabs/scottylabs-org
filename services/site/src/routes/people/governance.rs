//! Committee directors and heads from governance's `data/committees.yaml`. Each listed director becomes director of their committee and each listed head a lead in it (source `governance`). It runs when an admin presses Sync (`POST /api/people/sync-governance`), at startup, and every six hours while the service runs. It never makes anyone an admin.
//!
//! Tech is left out of the head sync: committees.yaml lists no Tech heads, because Tech's leads live in governance's team files.
//!
//! The People page has the last word. When it removes someone's director or lead role, moves it to another committee or turns a director into a lead, [`opt_out`] remembers that, and the sync gives the person no role in that committee again until governance stops listing them there.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use super::{clear_lead_projects, display_name, find_or_create_user, record};
use crate::auth::{PortalViewer, Viewer, normalize_andrew_id};
use crate::error::{AppError, AppResult};
use crate::models::{Committee, Role};
use crate::permissions;
use crate::routes::common::{self, no_store};
use crate::state::AppState;

const GOVERNANCE_URL: &str =
    "https://git.cmu.dev/ScottyLabs/governance/raw/branch/main/data/committees.yaml";
const MAX_GOVERNANCE_BYTES: usize = 1024 * 1024;
/// How often the service syncs on its own while it runs.
const SYNC_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// A background sync still running after this long is stopped and its changes rolled back.
const SYNC_TIMEOUT: Duration = Duration::from_secs(60);
/// Taken for the length of a sync, so the Sync button and the background task never apply at once.
const SYNC_LOCK: i64 = 724_620;
/// Text quoted from governance in a note is cut to this many characters.
const MAX_QUOTED_CHARS: usize = 64;

/// Mirrors `GovernanceSyncResponse` in types.ts.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceSyncResponse {
    /// Directors made, directors changed or removed, and directors already in place.
    added: u32,
    updated: u32,
    unchanged: u32,
    /// Heads made leads, governance leads removed, and heads who already had a role in their committee.
    heads_added: u32,
    heads_removed: u32,
    heads_unchanged: u32,
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
    /// Roles outside any team, such as Head of Logistics.
    #[serde(default)]
    roles: BTreeMap<String, GovernanceRole>,
    #[serde(default)]
    teams: BTreeMap<String, GovernanceTeam>,
}

#[derive(Deserialize)]
struct GovernanceTeam {
    #[serde(default)]
    roles: BTreeMap<String, GovernanceRole>,
}

#[derive(Deserialize)]
struct GovernanceRole {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    members: Vec<String>,
}

/// Someone governance lists in a head role.
#[derive(Debug, PartialEq, Eq)]
pub struct Head {
    pub committee: Committee,
    pub andrew_id: String,
    /// The role's name, such as "Head of Logistics", for the audit log.
    pub role: String,
}

/// What governance lists, and notes about entries that were skipped.
pub struct Listed {
    /// Each committee's director, in the site's committee order.
    pub directors: Vec<(Committee, String)>,
    /// Each head once per committee, Tech excepted.
    pub heads: Vec<Head>,
    /// The committees in the file. Opt-outs are forgotten only in these, so a committee missing from the file keeps them.
    pub committees: Vec<Committee>,
    /// The committees whose heads were read. Governance leads are removed only from these, so a committee missing from the file keeps its leads.
    pub head_committees: Vec<Committee>,
    pub notes: Vec<String>,
}

impl Listed {
    /// The Andrew IDs governance lists in `committee`, as director or as a head.
    fn people_in(&self, committee: Committee) -> Vec<&str> {
        let directors = self
            .directors
            .iter()
            .filter(|(listed, _)| *listed == committee)
            .map(|(_, andrew_id)| andrew_id.as_str());
        let heads = self
            .heads
            .iter()
            .filter(|head| head.committee == committee)
            .map(|head| head.andrew_id.as_str());
        directors.chain(heads).collect()
    }
}

/// A name from the governance file in curly quotes, cut short, since the file is not ours.
fn quoted(text: &str) -> String {
    let short: String = text.chars().take(MAX_QUOTED_CHARS).collect();
    common::quoted(&short)
}

/// Reads `data/committees.yaml`. The organization director is not a committee director and is ignored; governance never makes anyone an admin.
pub fn parse_committees(yaml: &str) -> Result<Listed, String> {
    let file: GovernanceFile = serde_norway::from_str(yaml).map_err(|error| error.to_string())?;
    let mut listed = Listed {
        directors: Vec::new(),
        heads: Vec::new(),
        committees: Vec::new(),
        head_committees: Vec::new(),
        notes: Vec::new(),
    };
    for (key, entry) in &file.committees {
        let Some(committee) = Committee::from_slug(key) else {
            listed.notes.push(format!(
                "Skipped {}: the site has no committee by that name.",
                quoted(key)
            ));
            continue;
        };
        listed.committees.push(committee);
        match entry.director.as_deref().map(normalize_andrew_id) {
            Some(Some(andrew_id)) => listed.directors.push((committee, andrew_id)),
            Some(None) => listed.notes.push(format!(
                "Skipped {}: its director is not a valid Andrew ID.",
                committee.name()
            )),
            None => listed.notes.push(format!(
                "Governance lists no director for {}.",
                committee.name()
            )),
        }
        if committee == Committee::Tech {
            if has_roles(entry) {
                listed.notes.push(
                    "Skipped the heads listed under Tech. Tech leads come from the governance team files, so add them on this page."
                        .into(),
                );
            }
            continue;
        }
        listed
            .heads
            .extend(heads_of(committee, entry, &mut listed.notes));
        listed.head_committees.push(committee);
    }
    listed.directors.sort_by_key(|(committee, _)| *committee);
    listed.heads.sort_by_key(|head| head.committee);
    listed.committees.sort();
    listed.head_committees.sort();
    Ok(listed)
}

fn has_roles(entry: &GovernanceCommittee) -> bool {
    !entry.roles.is_empty() || entry.teams.values().any(|team| !team.roles.is_empty())
}

/// The members of a committee's own roles and of its teams' roles, each person once.
fn heads_of(
    committee: Committee,
    entry: &GovernanceCommittee,
    notes: &mut Vec<String>,
) -> Vec<Head> {
    let roles = entry
        .roles
        .iter()
        .chain(entry.teams.values().flat_map(|team| team.roles.iter()));
    let mut heads: Vec<Head> = Vec::new();
    for (key, role) in roles {
        let role_name = role
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(key);
        for member in &role.members {
            let Some(andrew_id) = normalize_andrew_id(member) else {
                notes.push(format!(
                    "Skipped {} in {}: it is not a valid Andrew ID.",
                    quoted(member),
                    committee.name()
                ));
                continue;
            };
            // Someone holding two head roles in one committee still gets one lead role.
            if !heads.iter().any(|head| head.andrew_id == andrew_id) {
                heads.push(Head {
                    committee,
                    andrew_id,
                    role: role_name.to_owned(),
                });
            }
        }
    }
    heads
}

async fn fetch_governance(http: &reqwest::Client, url: &str) -> AppResult<String> {
    let unreachable = || {
        AppError::Upstream(
            "We couldn't reach the governance repository. Try again in a few minutes.".into(),
        )
    };
    let mut response = http.get(url).send().await.map_err(|error| {
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
            return Err(AppError::Upstream(
                "The governance committees file is larger than expected, so it was not read."
                    .into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body)
        .map_err(|_| AppError::Upstream("The governance committees file is not valid text.".into()))
}

/// Remembers that the People page took `user_id`'s role in `committee` away or lowered it, so the sync does not give one back while governance still lists them there. Called in the same transaction as the change.
pub(super) async fn opt_out(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO governance_opt_outs (user_id, committee) VALUES ($1, $2)
         ON CONFLICT (user_id, committee) DO NOTHING",
    )
    .bind(user_id)
    .bind(committee)
    .execute(conn)
    .await?;
    Ok(())
}

async fn has_opted_out(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
) -> AppResult<bool> {
    let opted_out = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM governance_opt_outs WHERE user_id = $1 AND committee = $2)",
    )
    .bind(user_id)
    .bind(committee)
    .fetch_one(conn)
    .await?;
    Ok(opted_out)
}

/// Forgets the opt-outs in `committee` of everyone governance no longer lists there (`listed` holds its director and heads), so a later listing counts again.
async fn forget_opt_outs(
    conn: &mut PgConnection,
    committee: Committee,
    listed: &[&str],
) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM governance_opt_outs o USING users u
         WHERE u.id = o.user_id AND o.committee = $1 AND NOT (u.andrew_id = ANY($2))",
    )
    .bind(committee)
    .bind(listed)
    .execute(conn)
    .await?;
    Ok(())
}

/// Makes each listed director a director of their committee (source `governance`), unless the People page opted them out there, and removes directors that an earlier sync added but governance no longer lists. `actor` is the admin who pressed Sync, or `None` when the service syncs on its own.
pub async fn apply_directors(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
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
        let opted_out = !matches!(existing, Some((_, Role::Director)))
            && has_opted_out(conn, user_id, *committee).await?;
        match existing {
            Some((_, Role::Director)) => report.unchanged += 1,
            _ if opted_out => report.messages.push(format!(
                "Governance lists {shown} as director of {}, but their role there was removed or changed on this page, so it was not given back. Make them director here if they should be.",
                committee.name()
            )),
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
                    actor,
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
                    actor,
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
                actor,
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

/// Makes each head a lead of their committee (can post to the ticker, no projects, source `governance`) unless they already have a role there, which is left as it is, or the People page opted them out there. Then, in each of `committees`: removes governance leads whom governance no longer lists, with their projects; says when governance stops listing a head whose lead role the portal keeps (a lead edited on the People page has source `portal`); and remembers who is listed for the next run.
pub async fn apply_heads(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
    heads: &[Head],
    committees: &[Committee],
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    let mut listed_users = Vec::with_capacity(heads.len());
    for head in heads {
        listed_users.push((head.committee, grant_head(conn, actor, head, report).await?));
    }
    for committee in committees {
        let listed: Vec<&str> = heads
            .iter()
            .filter(|head| head.committee == *committee)
            .map(|head| head.andrew_id.as_str())
            .collect();
        let user_ids: Vec<Uuid> = listed_users
            .iter()
            .filter(|(listed_in, _)| listed_in == committee)
            .map(|(_, user_id)| *user_id)
            .collect();
        remove_dropped_leads(conn, actor, *committee, &listed, report).await?;
        report_kept_leads(conn, actor, *committee, &listed, report).await?;
        remember_heads(conn, *committee, &user_ids).await?;
    }
    Ok(())
}

/// Makes one head a lead, when they have no role in the committee and were not opted out there. Returns their user id.
async fn grant_head(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
    head: &Head,
    report: &mut GovernanceSyncResponse,
) -> AppResult<Uuid> {
    let (user_id, name) = find_or_create_user(conn, &head.andrew_id, None).await?;
    let shown = display_name(&name, &head.andrew_id);
    let has_role: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM role_assignments WHERE user_id = $1 AND committee = $2)",
    )
    .bind(user_id)
    .bind(head.committee)
    .fetch_one(&mut *conn)
    .await?;
    if has_role {
        report.heads_unchanged += 1;
        return Ok(user_id);
    }
    if has_opted_out(conn, user_id, head.committee).await? {
        report.messages.push(format!(
            "Governance lists {shown} as a head in {}, but their role there was removed or changed on this page, so it was not given back. Add them as a lead here if they should be one.",
            head.committee.name()
        ));
        return Ok(user_id);
    }
    let granted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, source)
         SELECT $1, $2, 'lead', $3, true, 'governance'
         WHERE NOT EXISTS (SELECT 1 FROM role_assignments WHERE user_id = $2 AND committee = $3)
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(head.committee)
    .fetch_optional(&mut *conn)
    .await?;
    if granted.is_none() {
        report.heads_unchanged += 1;
        return Ok(user_id);
    }
    record(
        conn,
        actor,
        "role.sync",
        user_id,
        Some(head.committee),
        format!(
            "Made {shown} a lead in {} from governance ({})",
            head.committee.name(),
            head.role
        ),
    )
    .await?;
    report.heads_added += 1;
    Ok(user_id)
}

/// Removes the governance leads in `committee` whom governance no longer lists (`listed`), with their projects.
async fn remove_dropped_leads(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
    committee: Committee,
    listed: &[&str],
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    let dropped: Vec<(Uuid, String, String)> = sqlx::query_as(
        "DELETE FROM role_assignments r USING users u
         WHERE u.id = r.user_id AND r.role = 'lead' AND r.committee = $1
           AND r.source = 'governance' AND NOT (u.andrew_id = ANY($2))
         RETURNING r.user_id, u.andrew_id, u.name",
    )
    .bind(committee)
    .bind(listed)
    .fetch_all(&mut *conn)
    .await?;
    for (user_id, andrew_id, name) in dropped {
        clear_lead_projects(conn, user_id, committee).await?;
        let shown = display_name(&name, &andrew_id);
        record(
            conn,
            actor,
            "role.sync",
            user_id,
            Some(committee),
            format!(
                "Removed {shown} as a lead in {}; governance no longer lists them as a head",
                committee.name()
            ),
        )
        .await?;
        report.messages.push(format!(
            "{shown} is no longer a lead in {}. Governance no longer lists them as a head.",
            committee.name()
        ));
        report.heads_removed += 1;
    }
    Ok(())
}

/// Says, once, which heads from the last run governance no longer lists but who are still leads in `committee` because the portal keeps their role. Runs after [`remove_dropped_leads`], so every lead left is the portal's.
async fn report_kept_leads(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
    committee: Committee,
    listed: &[&str],
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    let kept: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT u.id, u.andrew_id, u.name
         FROM governance_heads g JOIN users u ON u.id = g.user_id
         WHERE g.committee = $1 AND NOT (u.andrew_id = ANY($2))
           AND EXISTS (
               SELECT 1 FROM role_assignments r
               WHERE r.user_id = g.user_id AND r.committee = g.committee AND r.role = 'lead'
           )
         ORDER BY u.andrew_id",
    )
    .bind(committee)
    .bind(listed)
    .fetch_all(&mut *conn)
    .await?;
    for (user_id, andrew_id, name) in kept {
        let shown = display_name(&name, &andrew_id);
        record(
            conn,
            actor,
            "role.sync",
            user_id,
            Some(committee),
            format!(
                "Kept {shown} as a lead in {}; governance no longer lists them as a head",
                committee.name()
            ),
        )
        .await?;
        report.messages.push(format!(
            "{shown} is still a lead in {}, but governance no longer lists them as a head. Remove them here if they should no longer be a lead.",
            committee.name()
        ));
    }
    Ok(())
}

/// Replaces the heads remembered for `committee` with the ones governance lists now.
async fn remember_heads(
    conn: &mut PgConnection,
    committee: Committee,
    user_ids: &[Uuid],
) -> AppResult<()> {
    sqlx::query("DELETE FROM governance_heads WHERE committee = $1 AND NOT (user_id = ANY($2))")
        .bind(committee)
        .bind(user_ids)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO governance_heads (committee, user_id)
         SELECT $1, unnest($2::uuid[])
         ON CONFLICT (committee, user_id) DO NOTHING",
    )
    .bind(committee)
    .bind(user_ids)
    .execute(conn)
    .await?;
    Ok(())
}

/// Fetches the committees file and applies directors and heads in one transaction.
async fn sync_from(
    db: &PgPool,
    http: &reqwest::Client,
    url: &str,
    actor: Option<&Viewer>,
) -> AppResult<GovernanceSyncResponse> {
    let yaml = fetch_governance(http, url).await?;
    let mut listed = parse_committees(&yaml).map_err(|error| {
        tracing::warn!(error = %error, "governance committees file did not parse");
        AppError::Upstream(
            "The governance committees file could not be read. Check data/committees.yaml in governance, then try again.".into(),
        )
    })?;
    let mut report = GovernanceSyncResponse {
        messages: std::mem::take(&mut listed.notes),
        ..GovernanceSyncResponse::default()
    };
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(SYNC_LOCK)
        .execute(&mut *tx)
        .await?;
    apply_directors(&mut tx, actor, &listed.directors, &mut report).await?;
    apply_heads(
        &mut tx,
        actor,
        &listed.heads,
        &listed.head_committees,
        &mut report,
    )
    .await?;
    for committee in &listed.committees {
        forget_opt_outs(&mut tx, *committee, &listed.people_in(*committee)).await?;
    }
    tx.commit().await?;
    Ok(report)
}

pub(super) async fn sync(
    State(state): State<AppState>,
    viewer: PortalViewer,
) -> AppResult<Response> {
    if !permissions::can_sync_governance(&viewer) {
        return Err(AppError::Forbidden(
            "Only admins can sync directors and heads from governance.".into(),
        ));
    }
    let report = sync_from(&state.db, &state.http, GOVERNANCE_URL, Some(&viewer)).await?;
    Ok(no_store(Json(report).into_response()))
}

/// Syncs now and then every six hours, in the background. A failed or slow sync only logs a warning and leaves roles as they were; it never holds up or stops the service.
pub fn spawn_sync(state: AppState) {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(SYNC_INTERVAL);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            sync_in_background(&state.db, &state.http, GOVERNANCE_URL).await;
        }
    });
}

async fn sync_in_background(db: &PgPool, http: &reqwest::Client, url: &str) {
    match tokio::time::timeout(SYNC_TIMEOUT, sync_from(db, http, url, None)).await {
        Ok(Ok(report)) => tracing::info!(
            directors_added = report.added,
            directors_updated = report.updated,
            heads_added = report.heads_added,
            heads_removed = report.heads_removed,
            messages = ?report.messages,
            "synced directors and heads from governance"
        ),
        Ok(Err(error)) => tracing::warn!(
            error = %match &error {
                AppError::Internal(cause) => format!("{cause:#}"),
                other => other.to_string(),
            },
            "could not sync directors and heads from governance; roles are unchanged"
        ),
        Err(_) => tracing::warn!(
            "syncing directors and heads from governance took over a minute and was stopped; roles are unchanged"
        ),
    }
}

#[cfg(test)]
mod tests;
