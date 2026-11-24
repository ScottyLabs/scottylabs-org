//! Who may do what. The single source of truth for the rules in docs/implementation/contract.md ("Viewer and permissions"). Handlers must check these;
//! the UI hiding a control is never the check.

use serde::Serialize;
use uuid::Uuid;

use crate::auth::Viewer;
use crate::models::{Committee, Role, TickerSource};

pub fn is_admin(viewer: &Viewer) -> bool {
    viewer.roles.iter().any(|a| a.role == Role::Admin)
}

pub fn directs(viewer: &Viewer, committee: Committee) -> bool {
    viewer
        .roles
        .iter()
        .any(|a| a.role == Role::Director && a.committee == Some(committee))
}

pub fn leads(viewer: &Viewer, committee: Committee) -> bool {
    viewer
        .roles
        .iter()
        .any(|a| a.role == Role::Lead && a.committee == Some(committee))
}

fn committees_with(viewer: &Viewer, role: Role) -> Vec<Committee> {
    let mut list: Vec<Committee> = viewer
        .roles
        .iter()
        .filter(|a| a.role == role)
        .filter_map(|a| a.committee)
        .collect();
    list.sort();
    list.dedup();
    list
}

pub fn director_of(viewer: &Viewer) -> Vec<Committee> {
    committees_with(viewer, Role::Director)
}

pub fn lead_of(viewer: &Viewer) -> Vec<Committee> {
    committees_with(viewer, Role::Lead)
}

pub fn can_use_portal(viewer: &Viewer) -> bool {
    !viewer.roles.is_empty()
}

/// How a post from a source goes live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posting {
    Direct,
    NeedsApproval,
}

pub fn ticker_posting(viewer: &Viewer, source: TickerSource) -> Option<Posting> {
    if is_admin(viewer) {
        return Some(Posting::Direct);
    }
    let committee = source.committee()?;
    if directs(viewer, committee) {
        return Some(Posting::Direct);
    }
    let lead_can_post = viewer
        .roles
        .iter()
        .any(|a| a.role == Role::Lead && a.committee == Some(committee) && a.can_post_ticker);
    lead_can_post.then_some(Posting::NeedsApproval)
}

pub fn ticker_sources(viewer: &Viewer) -> Vec<TickerSource> {
    TickerSource::ALL
        .iter()
        .copied()
        .filter(|source| ticker_posting(viewer, *source).is_some())
        .collect()
}

/// Extend, end and edit live announcements from a source.
pub fn can_manage_ticker(viewer: &Viewer, source: TickerSource) -> bool {
    is_admin(viewer) || source.committee().is_some_and(|c| directs(viewer, c))
}

pub fn ticker_manages(viewer: &Viewer) -> Vec<TickerSource> {
    TickerSource::ALL
        .iter()
        .copied()
        .filter(|source| can_manage_ticker(viewer, *source))
        .collect()
}

pub fn can_reorder_ticker(viewer: &Viewer) -> bool {
    is_admin(viewer) || !director_of(viewer).is_empty()
}

/// Whether the viewer may edit an announcement in the given state.
/// `pending_or_sent_back` is true for announcements that are not live yet.
pub fn can_edit_announcement(
    viewer: &Viewer,
    source: TickerSource,
    created_by: Option<Uuid>,
    pending_or_sent_back: bool,
) -> bool {
    if can_manage_ticker(viewer, source) {
        return true;
    }
    pending_or_sent_back
        && created_by == Some(viewer.user_id)
        && ticker_posting(viewer, source).is_some()
}

pub fn can_edit_page(viewer: &Viewer, committee: Committee) -> bool {
    is_admin(viewer) || directs(viewer, committee)
}

pub fn editable_pages(viewer: &Viewer) -> Vec<Committee> {
    Committee::ALL
        .iter()
        .copied()
        .filter(|c| can_edit_page(viewer, *c))
        .collect()
}

/// Create, publish and archive projects owned by a committee.
pub fn can_publish_projects_for(viewer: &Viewer, committee: Committee) -> bool {
    is_admin(viewer) || directs(viewer, committee)
}

pub fn project_committees(viewer: &Viewer) -> Vec<Committee> {
    Committee::ALL
        .iter()
        .copied()
        .filter(|c| can_publish_projects_for(viewer, *c))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectAccess {
    None,
    /// Edit the draft; publishing creates an approval.
    Lead,
    /// Edit, publish, archive.
    Full,
}

pub fn project_access(viewer: &Viewer, owner: Committee, project_id: Uuid) -> ProjectAccess {
    if can_publish_projects_for(viewer, owner) {
        ProjectAccess::Full
    } else if viewer.lead_projects.contains(&project_id) {
        ProjectAccess::Lead
    } else {
        ProjectAccess::None
    }
}

/// Featured flag and slug changes.
pub fn can_curate_projects(viewer: &Viewer) -> bool {
    is_admin(viewer)
}

pub fn can_approve_for(viewer: &Viewer, committee: Committee) -> bool {
    is_admin(viewer) || directs(viewer, committee)
}

pub fn approves_for(viewer: &Viewer) -> Vec<Committee> {
    Committee::ALL
        .iter()
        .copied()
        .filter(|c| can_approve_for(viewer, *c))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeopleScope {
    All,
    Committees(Vec<Committee>),
    None,
}

pub fn people_scope(viewer: &Viewer) -> PeopleScope {
    if is_admin(viewer) {
        return PeopleScope::All;
    }
    let directs = director_of(viewer);
    if directs.is_empty() {
        PeopleScope::None
    } else {
        PeopleScope::Committees(directs)
    }
}

/// Add, change or remove an assignment of `role` in `committee`.
pub fn can_manage_role(viewer: &Viewer, role: Role, committee: Option<Committee>) -> bool {
    if is_admin(viewer) {
        return true;
    }
    role == Role::Lead && committee.is_some_and(|c| directs(viewer, c))
}

pub fn can_sync_governance(viewer: &Viewer) -> bool {
    is_admin(viewer)
}

/// Sent to the browser in `GET /api/me`. Mirrors `Permissions` in types.ts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsDto {
    pub can_use_portal: bool,
    pub is_admin: bool,
    pub director_of: Vec<Committee>,
    pub lead_of: Vec<Committee>,
    pub ticker_sources: Vec<TickerSource>,
    pub ticker_needs_approval: bool,
    pub ticker_manages: Vec<TickerSource>,
    pub editable_pages: Vec<Committee>,
    pub project_committees: Vec<Committee>,
    pub editable_project_ids: Vec<Uuid>,
    pub approves_for: Vec<Committee>,
    pub people_scope: PeopleScopeDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum PeopleScopeDto {
    All(&'static str),
    Committees(Vec<Committee>),
}

pub fn summary(viewer: &Viewer) -> PermissionsDto {
    let sources = ticker_sources(viewer);
    let needs_approval = !sources.is_empty()
        && sources
            .iter()
            .all(|s| ticker_posting(viewer, *s) == Some(Posting::NeedsApproval));
    PermissionsDto {
        can_use_portal: can_use_portal(viewer),
        is_admin: is_admin(viewer),
        director_of: director_of(viewer),
        lead_of: lead_of(viewer),
        ticker_sources: sources,
        ticker_needs_approval: needs_approval,
        ticker_manages: ticker_manages(viewer),
        editable_pages: editable_pages(viewer),
        project_committees: project_committees(viewer),
        editable_project_ids: viewer.lead_projects.clone(),
        approves_for: approves_for(viewer),
        people_scope: match people_scope(viewer) {
            PeopleScope::All => PeopleScopeDto::All("all"),
            PeopleScope::Committees(list) => PeopleScopeDto::Committees(list),
            PeopleScope::None => PeopleScopeDto::Committees(Vec::new()),
        },
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::auth::{Assignment, Viewer};
    use crate::models::AuthMode;

    pub fn viewer(roles: &[(Role, Option<Committee>, bool)], lead_projects: Vec<Uuid>) -> Viewer {
        Viewer {
            user_id: Uuid::now_v7(),
            andrew_id: "someone".into(),
            name: "Some One".into(),
            email: None,
            session_id: Uuid::now_v7(),
            csrf_token: "csrf".into(),
            auth_mode: AuthMode::Dev,
            roles: roles
                .iter()
                .map(|(role, committee, can_post_ticker)| Assignment {
                    id: Uuid::now_v7(),
                    role: *role,
                    committee: *committee,
                    can_post_ticker: *can_post_ticker,
                })
                .collect(),
            lead_projects,
        }
    }

    #[test]
    fn members_without_roles_get_nothing() {
        let v = viewer(&[], vec![]);
        assert!(!can_use_portal(&v));
        assert!(ticker_sources(&v).is_empty());
        assert!(editable_pages(&v).is_empty());
        assert_eq!(people_scope(&v), PeopleScope::None);
        assert!(!can_reorder_ticker(&v));
    }

    #[test]
    fn admins_can_do_everything() {
        let v = viewer(&[(Role::Admin, None, true)], vec![]);
        assert!(can_use_portal(&v));
        assert_eq!(
            ticker_posting(&v, TickerSource::Club),
            Some(Posting::Direct)
        );
        assert_eq!(ticker_sources(&v).len(), TickerSource::ALL.len());
        assert!(can_edit_page(&v, Committee::Finance));
        assert_eq!(
            project_access(&v, Committee::Tech, Uuid::now_v7()),
            ProjectAccess::Full
        );
        assert!(can_manage_role(&v, Role::Director, Some(Committee::Tech)));
        assert!(can_manage_role(&v, Role::Admin, None));
        assert!(can_curate_projects(&v));
        assert_eq!(people_scope(&v), PeopleScope::All);
    }

    #[test]
    fn directors_are_scoped_to_their_committee() {
        let v = viewer(&[(Role::Director, Some(Committee::Events), true)], vec![]);
        assert_eq!(
            ticker_posting(&v, TickerSource::Events),
            Some(Posting::Direct)
        );
        assert_eq!(ticker_posting(&v, TickerSource::Tech), None);
        assert_eq!(ticker_posting(&v, TickerSource::Club), None);
        assert!(can_manage_ticker(&v, TickerSource::Events));
        assert!(!can_manage_ticker(&v, TickerSource::Tech));
        assert!(can_reorder_ticker(&v));
        assert!(can_edit_page(&v, Committee::Events));
        assert!(!can_edit_page(&v, Committee::Tech));
        assert_eq!(
            project_access(&v, Committee::Events, Uuid::now_v7()),
            ProjectAccess::Full
        );
        assert_eq!(
            project_access(&v, Committee::Tech, Uuid::now_v7()),
            ProjectAccess::None
        );
        assert!(can_approve_for(&v, Committee::Events));
        assert!(!can_approve_for(&v, Committee::Design));
        assert!(can_manage_role(&v, Role::Lead, Some(Committee::Events)));
        assert!(!can_manage_role(&v, Role::Lead, Some(Committee::Tech)));
        assert!(!can_manage_role(
            &v,
            Role::Director,
            Some(Committee::Events)
        ));
        assert!(!can_manage_role(&v, Role::Admin, None));
        assert!(!can_curate_projects(&v));
        assert_eq!(
            people_scope(&v),
            PeopleScope::Committees(vec![Committee::Events])
        );
    }

    #[test]
    fn leads_need_approval_and_only_edit_given_projects() {
        let nova = Uuid::now_v7();
        let v = viewer(&[(Role::Lead, Some(Committee::Events), true)], vec![nova]);
        assert_eq!(
            ticker_posting(&v, TickerSource::Events),
            Some(Posting::NeedsApproval)
        );
        assert!(summary(&v).ticker_needs_approval);
        assert!(!can_manage_ticker(&v, TickerSource::Events));
        assert!(!can_reorder_ticker(&v));
        assert!(!can_edit_page(&v, Committee::Events));
        assert_eq!(
            project_access(&v, Committee::Events, nova),
            ProjectAccess::Lead
        );
        assert_eq!(
            project_access(&v, Committee::Events, Uuid::now_v7()),
            ProjectAccess::None
        );
        assert!(!can_approve_for(&v, Committee::Events));
        assert_eq!(people_scope(&v), PeopleScope::None);

        let own = v.user_id;
        assert!(can_edit_announcement(
            &v,
            TickerSource::Events,
            Some(own),
            true
        ));
        assert!(!can_edit_announcement(
            &v,
            TickerSource::Events,
            Some(own),
            false
        ));
        assert!(!can_edit_announcement(
            &v,
            TickerSource::Events,
            Some(Uuid::now_v7()),
            true
        ));
    }

    #[test]
    fn leads_without_ticker_permission_cannot_post() {
        let v = viewer(&[(Role::Lead, Some(Committee::Events), false)], vec![]);
        assert_eq!(ticker_posting(&v, TickerSource::Events), None);
        assert!(ticker_sources(&v).is_empty());
        assert!(!summary(&v).ticker_needs_approval);
    }
}
