//! Sign-in, sessions and the request extractors for signed-in people.
//! Owned by the platform track (docs/implementation/contract.md).

pub mod csrf;
pub mod extract;
pub mod oidc;
pub mod session;

use uuid::Uuid;

use crate::models::{AuthMode, Committee, PersonRef, Role};

/// The signed-in person making a request, with their roles.
#[derive(Clone, Debug)]
pub struct Viewer {
    pub user_id: Uuid,
    pub andrew_id: String,
    pub name: String,
    pub email: Option<String>,
    pub session_id: Uuid,
    pub csrf_token: String,
    pub auth_mode: AuthMode,
    pub roles: Vec<Assignment>,
    /// Projects this person may edit as a lead.
    pub lead_projects: Vec<Uuid>,
}

#[derive(Clone, Debug)]
pub struct Assignment {
    pub id: Uuid,
    pub role: Role,
    pub committee: Option<Committee>,
    pub can_post_ticker: bool,
}

impl Viewer {
    pub fn person(&self) -> PersonRef {
        PersonRef::new(self.user_id, self.andrew_id.clone(), self.name.clone())
    }
}

/// A [`Viewer`] with at least one role. Every portal endpoint except `/api/me` takes this.
#[derive(Clone, Debug)]
pub struct PortalViewer(pub Viewer);

impl std::ops::Deref for PortalViewer {
    type Target = Viewer;

    fn deref(&self) -> &Viewer {
        &self.0
    }
}
