//! Committee directors and heads from governance's `data/committees.yaml`. Each listed director becomes director of their committee and each listed head a lead in it (source `governance`). It runs when an admin presses Sync (`POST /api/people/sync-governance`), at startup, and every six hours while the service runs. It never makes anyone an admin.
//!
//! Tech is left out of the head sync: committees.yaml lists no Tech heads, because Tech's leads live in governance's team files.
//!
//! The People page has the last word. When it removes someone's director or lead role, moves it to another committee or turns a director into a lead, [`opt_out`] remembers which role was taken away. A director's role taken away keeps the sync from giving the person any role in that committee, a lead's role only from making them a lead again, so removing a lead never blocks governance from appointing them director. Opt-outs are forgotten once governance stops listing the person there.
//!
//! Governance directors and leads whom governance no longer lists are removed, also when governance names no successor. Roles the People page gave are kept and reported. Each run's messages are stored for admins (`GET /api/people`); the service log gets counts only, never names.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use super::{clear_lead_projects, display_name, find_or_create_user, record};
use crate::audit;
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
/// At most this many committees lose their governance director in one run because the file names no director for them. More looks like a broken file, so none are removed and the run says so.
const MAX_DIRECTORLESS_REMOVALS: usize = 2;

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
    /// Committees whose governance director this run removed, to say when one is left without a director.
    #[serde(skip)]
    removed_directors_in: Vec<Committee>,
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

/// Remembers that the People page took `user_id`'s `role` in `committee` away or lowered it, so the sync does not give it back while governance still lists them there. A director's role blocks every role there, a lead's only the lead role; the stronger one wins. Admin roles have no committee and are never synced. Called in the same transaction as the change.
pub(super) async fn opt_out(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
    role: Role,
) -> AppResult<()> {
    let level = if role == Role::Director {
        Role::Director
    } else {
        Role::Lead
    };
    sqlx::query(
        "INSERT INTO governance_opt_outs (user_id, committee, role) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, committee) DO UPDATE
         SET role = CASE WHEN governance_opt_outs.role = 'director' THEN 'director'
                         ELSE EXCLUDED.role END",
    )
    .bind(user_id)
    .bind(committee)
    .bind(level)
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether the People page opted `user_id` out of being given `grant` in `committee`: any opt-out blocks a lead role, only a director's one blocks the director role.
async fn has_opted_out(
    conn: &mut PgConnection,
    user_id: Uuid,
    committee: Committee,
    grant: Role,
) -> AppResult<bool> {
    let opted_out = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM governance_opt_outs
             WHERE user_id = $1 AND committee = $2 AND ($3 = 'lead' OR role = 'director'))",
    )
    .bind(user_id)
    .bind(committee)
    .bind(grant)
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
            && has_opted_out(conn, user_id, *committee, Role::Director).await?;
        match existing {
            Some((_, Role::Director)) => report.unchanged += 1,
            _ if opted_out => report.messages.push(format!(
                "Governance lists {shown} as director of {}, but their role there was removed or changed on this page, so it was not given back. Make them director here if they should be.",
                committee.name()
            )),
            Some((assignment, old_role)) => {
                sqlx::query(
                    "UPDATE role_assignments
                     SET role = 'director', can_post_ticker = true, appointed_by = NULL,
                         source = 'governance', created_at = now(), updated_at = now(),
                         updated_by = NULL
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
                    audit::change(
                        sync_state(andrew_id, old_role, *committee),
                        sync_state(andrew_id, Role::Director, *committee),
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
                    audit::change(
                        serde_json::Value::Null,
                        sync_state(andrew_id, Role::Director, *committee),
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
                audit::change(
                    sync_state(&old_andrew_id, Role::Director, *committee),
                    serde_json::Value::Null,
                ),
            )
            .await?;
            report.messages.push(format!(
                "{old} is no longer director of {}. Governance lists {shown}.",
                committee.name()
            ));
            report.removed_directors_in.push(*committee);
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
    if has_opted_out(conn, user_id, head.committee, Role::Lead).await? {
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
        audit::change(
            serde_json::Value::Null,
            sync_state(&head.andrew_id, Role::Lead, head.committee),
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
            audit::change(
                sync_state(&andrew_id, Role::Lead, committee),
                serde_json::Value::Null,
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
            json!({ "kept": { "andrewId": andrew_id, "role": Role::Lead, "committee": committee } }),
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

fn slugs(committees: &[Committee]) -> Vec<&'static str> {
    committees.iter().map(|c| c.slug()).collect()
}

/// A role the sync gave or took away, as the audit log's `details` keep it.
fn sync_state(andrew_id: &str, role: Role, committee: Committee) -> serde_json::Value {
    json!({ "andrewId": andrew_id, "role": role, "committee": committee, "source": "governance" })
}

/// Removes the governance directors of committees that are in the file but list no valid director, so a director governance dropped does not keep the role until a successor is named. More than [`MAX_DIRECTORLESS_REMOVALS`] committees at once looks like a broken file: nobody is removed and the run says so.
async fn remove_directorless(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
    committees: &[Committee],
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    let affected: Vec<Committee> = sqlx::query_scalar(
        "SELECT DISTINCT r.committee FROM role_assignments r
         JOIN committees c ON c.slug = r.committee
         WHERE r.role = 'director' AND r.source = 'governance' AND r.committee = ANY($1)
         ORDER BY r.committee",
    )
    .bind(slugs(committees))
    .fetch_all(&mut *conn)
    .await?;
    if affected.len() > MAX_DIRECTORLESS_REMOVALS {
        report.messages.push(format!(
            "Governance lists no director for {} committees that have one from governance, more than a sync removes at once, so nobody was removed. Check data/committees.yaml in governance.",
            affected.len()
        ));
        return Ok(());
    }
    for committee in affected {
        let removed: Vec<(Uuid, String, String)> = sqlx::query_as(
            "DELETE FROM role_assignments r USING users u
             WHERE u.id = r.user_id AND r.role = 'director' AND r.committee = $1
               AND r.source = 'governance'
             RETURNING r.user_id, u.andrew_id, u.name",
        )
        .bind(committee)
        .fetch_all(&mut *conn)
        .await?;
        for (user_id, andrew_id, name) in removed {
            let shown = display_name(&name, &andrew_id);
            record(
                conn,
                actor,
                "role.sync",
                user_id,
                Some(committee),
                format!(
                    "Removed {shown} as director of {}; governance no longer lists them",
                    committee.name()
                ),
                audit::change(
                    sync_state(&andrew_id, Role::Director, committee),
                    serde_json::Value::Null,
                ),
            )
            .await?;
            report.messages.push(format!(
                "{shown} is no longer director of {}. Governance lists no director for it.",
                committee.name()
            ));
            report.removed_directors_in.push(committee);
            report.updated += 1;
        }
    }
    Ok(())
}

/// Says which directors of the file's committees governance does not list: roles the People page gave, which the sync keeps. The audit log notes each once, when governance's director for that committee changes; the messages say it on every run. Then remembers the listed directors for the next run and for the People page.
async fn review_directors(
    conn: &mut PgConnection,
    actor: Option<&Viewer>,
    listed: &Listed,
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    let first_run: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM governance_sync_status WHERE last_success_at IS NOT NULL)",
    )
    .fetch_one(&mut *conn)
    .await?;
    for committee in &listed.committees {
        let named = listed
            .directors
            .iter()
            .find(|(listed_in, _)| listed_in == committee)
            .map(|(_, andrew_id)| andrew_id.as_str());
        let listed_user: Option<Uuid> = match named {
            Some(andrew_id) => {
                sqlx::query_scalar("SELECT id FROM users WHERE andrew_id = $1")
                    .bind(andrew_id)
                    .fetch_optional(&mut *conn)
                    .await?
            }
            None => None,
        };
        let previous: Option<Uuid> =
            sqlx::query_scalar("SELECT user_id FROM governance_directors WHERE committee = $1")
                .bind(committee)
                .fetch_optional(&mut *conn)
                .await?;
        let unlisted: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT u.id, u.andrew_id, u.name FROM role_assignments r JOIN users u ON u.id = r.user_id
             WHERE r.role = 'director' AND r.committee = $1
               AND ($2::uuid IS NULL OR r.user_id <> $2)
             ORDER BY u.andrew_id",
        )
        .bind(committee)
        .bind(listed_user)
        .fetch_all(&mut *conn)
        .await?;
        let lists = named.map_or_else(
            || format!("lists no director for {}", committee.name()),
            |andrew_id| format!("lists {andrew_id}"),
        );
        for (user_id, andrew_id, name) in unlisted {
            let shown = display_name(&name, &andrew_id);
            report.messages.push(format!(
                "{shown} is still director of {}, but governance {lists}. Remove them here if they should no longer direct {}.",
                committee.name(),
                committee.name()
            ));
            if first_run || previous != listed_user {
                record(
                    conn,
                    actor,
                    "role.sync",
                    user_id,
                    Some(*committee),
                    format!(
                        "Kept {shown} as director of {}; governance {lists}",
                        committee.name()
                    ),
                    json!({ "kept": { "andrewId": andrew_id, "role": Role::Director, "committee": committee } }),
                )
                .await?;
            }
        }
        if report.removed_directors_in.contains(committee) {
            let has_director: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM role_assignments WHERE role = 'director' AND committee = $1)",
            )
            .bind(committee)
            .fetch_one(&mut *conn)
            .await?;
            if !has_director {
                report.messages.push(format!(
                    "{} has no director now. Make someone director on this page if it should have one.",
                    committee.name()
                ));
            }
        }
        match listed_user {
            Some(user_id) => {
                sqlx::query(
                    "INSERT INTO governance_directors (committee, user_id) VALUES ($1, $2)
                     ON CONFLICT (committee) DO UPDATE SET user_id = EXCLUDED.user_id",
                )
                .bind(committee)
                .bind(user_id)
                .execute(&mut *conn)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM governance_directors WHERE committee = $1")
                    .bind(committee)
                    .execute(&mut *conn)
                    .await?;
            }
        }
    }
    Ok(())
}

/// Says which committees have roles from governance but are missing from the file. Their roles are kept, since a renamed or dropped key is more likely a mistake than an end to every role.
async fn report_missing_committees(
    conn: &mut PgConnection,
    listed: &Listed,
    report: &mut GovernanceSyncResponse,
) -> AppResult<()> {
    let missing: Vec<Committee> = sqlx::query_scalar(
        "SELECT DISTINCT r.committee FROM role_assignments r JOIN committees c ON c.slug = r.committee
         WHERE r.source = 'governance' AND NOT (r.committee = ANY($1))
         ORDER BY r.committee",
    )
    .bind(slugs(&listed.committees))
    .fetch_all(conn)
    .await?;
    for committee in missing {
        report.messages.push(format!(
            "Governance does not list {} any more, so its directors and leads from governance were kept. Check data/committees.yaml in governance, or remove them here.",
            committee.name()
        ));
    }
    Ok(())
}

/// The last governance sync, for admins on the People page. Mirrors `GovernanceSyncStatus` in types.ts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    last_attempt_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
    /// Why the last attempt failed; null when it worked.
    last_error: Option<String>,
    /// What the last successful run said, as in `GovernanceSyncResponse.messages`.
    messages: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct StatusRow {
    last_attempt_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    messages: sqlx::types::Json<Vec<String>>,
}

pub(super) async fn status(conn: &mut PgConnection) -> AppResult<SyncStatus> {
    let row = sqlx::query_as::<_, StatusRow>(
        "SELECT last_attempt_at, last_success_at, last_error, messages FROM governance_sync_status",
    )
    .fetch_optional(conn)
    .await?;
    Ok(match row {
        Some(row) => SyncStatus {
            last_attempt_at: row.last_attempt_at,
            last_success_at: row.last_success_at,
            last_error: row.last_error,
            messages: row.messages.0,
        },
        None => SyncStatus {
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            messages: Vec::new(),
        },
    })
}

/// Stores a successful run, in its transaction.
async fn save_success(conn: &mut PgConnection, report: &GovernanceSyncResponse) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO governance_sync_status (singleton, last_attempt_at, last_success_at, last_error, messages)
         VALUES (true, now(), now(), NULL, $1)
         ON CONFLICT (singleton) DO UPDATE
         SET last_attempt_at = now(), last_success_at = now(), last_error = NULL,
             messages = EXCLUDED.messages",
    )
    .bind(sqlx::types::Json(&report.messages))
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores why a run failed, keeping the last successful run's time and messages.
async fn save_failure(db: &PgPool, error: &AppError) {
    let shown = match error {
        AppError::Upstream(message) => message.clone(),
        _ => "The sync failed. Check the service log, then try again.".to_owned(),
    };
    let saved = sqlx::query(
        "INSERT INTO governance_sync_status (singleton, last_attempt_at, last_error)
         VALUES (true, now(), $1)
         ON CONFLICT (singleton) DO UPDATE SET last_attempt_at = now(), last_error = EXCLUDED.last_error",
    )
    .bind(shown)
    .execute(db)
    .await;
    if let Err(error) = saved {
        tracing::warn!(error = %error, "could not record the failed governance sync");
    }
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
    let directorless: Vec<Committee> = listed
        .committees
        .iter()
        .copied()
        .filter(|committee| !listed.directors.iter().any(|(c, _)| c == committee))
        .collect();
    remove_directorless(&mut tx, actor, &directorless, &mut report).await?;
    apply_heads(
        &mut tx,
        actor,
        &listed.heads,
        &listed.head_committees,
        &mut report,
    )
    .await?;
    review_directors(&mut tx, actor, &listed, &mut report).await?;
    report_missing_committees(&mut tx, &listed, &mut report).await?;
    for committee in &listed.committees {
        forget_opt_outs(&mut tx, *committee, &listed.people_in(*committee)).await?;
    }
    save_success(&mut tx, &report).await?;
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
    let report = match sync_from(&state.db, &state.http, GOVERNANCE_URL, Some(&viewer)).await {
        Ok(report) => report,
        Err(error) => {
            save_failure(&state.db, &error).await;
            return Err(error);
        }
    };
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

/// Logs counts only: the messages name people, and service logs are kept elsewhere for longer. Admins read the messages on the People page.
async fn sync_in_background(db: &PgPool, http: &reqwest::Client, url: &str) {
    match tokio::time::timeout(SYNC_TIMEOUT, sync_from(db, http, url, None)).await {
        Ok(Ok(report)) => tracing::info!(
            directors_added = report.added,
            directors_updated = report.updated,
            heads_added = report.heads_added,
            heads_removed = report.heads_removed,
            messages = report.messages.len(),
            "synced directors and heads from governance"
        ),
        Ok(Err(error)) => {
            tracing::warn!(
                error = %match &error {
                    AppError::Internal(cause) => format!("{cause:#}"),
                    other => other.to_string(),
                },
                "could not sync directors and heads from governance; roles are unchanged"
            );
            save_failure(db, &error).await;
        }
        Err(_) => {
            tracing::warn!(
                "syncing directors and heads from governance took over a minute and was stopped; roles are unchanged"
            );
            save_failure(
                db,
                &AppError::Upstream(
                    "The last automatic sync took over a minute and was stopped. Roles are unchanged."
                        .into(),
                ),
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests;
