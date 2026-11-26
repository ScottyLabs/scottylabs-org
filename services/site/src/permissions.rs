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

/// Open an announcement in the portal. Live, expired and ended ones are open to everyone in the portal; one waiting for approval (`waiting`) only to its author and those who manage its source.
pub fn can_view_announcement(
    viewer: &Viewer,
    source: TickerSource,
    created_by: Option<Uuid>,
    waiting: bool,
) -> bool {
    !waiting || created_by == Some(viewer.user_id) || can_manage_ticker(viewer, source)
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

/// Open a project in the portal, read only unless [`project_access`] allows more. Mirrors `GET /api/projects`: admins and directors see every project, leads the ones they edit.
pub fn can_view_project(viewer: &Viewer, project_id: Uuid) -> bool {
    is_admin(viewer)
        || !director_of(viewer).is_empty()
        || viewer.lead_projects.contains(&project_id)
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

/// End someone's sessions everywhere, or reset the link between their Andrew ID and a Keycloak account (sign-in recovery).
pub fn can_manage_sign_in(viewer: &Viewer) -> bool {
    is_admin(viewer)
}

/// Which audit entries "Recent activity" shows. Entries name what they are about, so they follow the rules for opening it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityScope {
    /// Admins see every entry.
    pub everything: bool,
    /// Committees whose entries the viewer sees in full: those whose ticker, page, projects and approvals they manage.
    pub committees: Vec<Committee>,
    /// Committees the viewer belongs to without managing them. Their entries about announcements show only while the viewer may open the announcement ([`can_view_announcement`]), and those about projects only for projects the viewer may open (`projects`); the rest show in full.
    pub limited_committees: Vec<Committee>,
    /// The projects the viewer may open ([`can_view_project`]): `None` for every project.
    pub projects: Option<Vec<Uuid>>,
}

/// The viewer's own actions always show, whatever this says.
pub fn activity_scope(viewer: &Viewer) -> ActivityScope {
    let committees: Vec<Committee> = Committee::ALL
        .iter()
        .copied()
        .filter(|c| can_approve_for(viewer, *c) && can_manage_ticker(viewer, (*c).into()))
        .collect();
    let mut limited_committees: Vec<Committee> = viewer
        .roles
        .iter()
        .filter_map(|a| a.committee)
        .filter(|c| !committees.contains(c))
        .collect();
    limited_committees.sort();
    limited_committees.dedup();
    let sees_every_project = is_admin(viewer) || !director_of(viewer).is_empty();
    ActivityScope {
        everything: is_admin(viewer),
        committees,
        limited_committees,
        projects: (!sees_every_project).then(|| viewer.lead_projects.clone()),
    }
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
    fn projects_are_visible_to_directors_and_their_leads() {
        let nova = Uuid::now_v7();
        let other = Uuid::now_v7();
        let admin = viewer(&[(Role::Admin, None, true)], vec![]);
        let director = viewer(&[(Role::Director, Some(Committee::Events), true)], vec![]);
        let lead = viewer(&[(Role::Lead, Some(Committee::Events), true)], vec![nova]);
        let member = viewer(&[], vec![]);
        assert!(can_view_project(&admin, other));
        assert!(can_view_project(&director, other));
        assert!(can_view_project(&lead, nova));
        assert!(!can_view_project(&lead, other));
        assert!(!can_view_project(&member, other));
    }

    #[test]
    fn waiting_announcements_are_for_their_author_and_managers() {
        let lead = viewer(&[(Role::Lead, Some(Committee::Events), true)], vec![]);
        let director = viewer(&[(Role::Director, Some(Committee::Events), true)], vec![]);
        let other = Some(Uuid::now_v7());
        let events = TickerSource::Events;
        assert!(can_view_announcement(&lead, events, other, false));
        assert!(!can_view_announcement(&lead, events, other, true));
        assert!(can_view_announcement(
            &lead,
            events,
            Some(lead.user_id),
            true
        ));
        assert!(can_view_announcement(&director, events, other, true));
        assert!(!can_view_announcement(
            &director,
            TickerSource::Tech,
            other,
            true
        ));
    }

    #[test]
    fn activity_follows_what_the_viewer_may_open() {
        let nova = Uuid::now_v7();
        let admin = activity_scope(&viewer(&[(Role::Admin, None, true)], vec![]));
        assert!(admin.everything);
        assert_eq!(admin.projects, None);

        let lead = activity_scope(&viewer(
            &[(Role::Lead, Some(Committee::Events), true)],
            vec![nova],
        ));
        assert!(!lead.everything);
        assert!(lead.committees.is_empty());
        assert_eq!(lead.limited_committees, [Committee::Events]);
        assert_eq!(lead.projects, Some(vec![nova]));

        // A director sees their committee in full, every project, and only some of another committee they lead in.
        let both = activity_scope(&viewer(
            &[
                (Role::Director, Some(Committee::Tech), true),
                (Role::Lead, Some(Committee::Events), true),
            ],
            vec![],
        ));
        assert_eq!(both.committees, [Committee::Tech]);
        assert_eq!(both.limited_committees, [Committee::Events]);
        assert_eq!(both.projects, None);
    }

    #[test]
    fn leads_without_ticker_permission_cannot_post() {
        let v = viewer(&[(Role::Lead, Some(Committee::Events), false)], vec![]);
        assert_eq!(ticker_posting(&v, TickerSource::Events), None);
        assert!(ticker_sources(&v).is_empty());
        assert!(!summary(&v).ticker_needs_approval);
    }

    #[test]
    fn only_admins_manage_sign_in() {
        assert!(can_manage_sign_in(&viewer(
            &[(Role::Admin, None, true)],
            vec![]
        )));
        for roles in [
            vec![(Role::Director, Some(Committee::Events), true)],
            vec![(Role::Lead, Some(Committee::Events), true)],
            vec![],
        ] {
            assert!(!can_manage_sign_in(&viewer(&roles, vec![])));
        }
    }
}
