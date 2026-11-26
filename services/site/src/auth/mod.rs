//! Sign-in, sessions and the request extractors for signed-in people.
//! Owned by the platform track (docs/implementation/contract.md).

pub mod csrf;
pub mod extract;
pub mod groups;
pub mod oidc;
pub mod revalidate;
pub mod roles;
pub mod session;

use sqlx::FromRow;
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
    /// What a release before 0701 sent as this session's CSRF token, still accepted for tabs it loaded. `None` for sessions made since.
    pub legacy_csrf_token: Option<String>,
    pub auth_mode: AuthMode,
    pub roles: Vec<Assignment>,
    /// Projects this person may edit as a lead.
    pub lead_projects: Vec<Uuid>,
}

#[derive(Clone, Debug, FromRow)]
pub struct Assignment {
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

/// Trims and lowercases an Andrew ID, or `None` when it cannot be one. Mirrors the `users.andrew_id` check.
pub fn normalize_andrew_id(raw: &str) -> Option<String> {
    let id = raw.trim().to_ascii_lowercase();
    let mut chars = id.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_ok =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    (first_ok && rest_ok && id.len() <= 64).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn andrew_ids_follow_the_schema_pattern() {
        assert_eq!(normalize_andrew_id(" TKanz ").as_deref(), Some("tkanz"));
        assert_eq!(normalize_andrew_id("a.b_c-1").as_deref(), Some("a.b_c-1"));
        assert_eq!(
            normalize_andrew_id(&"a".repeat(64)).map(|s| s.len()),
            Some(64)
        );
        for bad in [
            "",
            "-x",
            ".x",
            "a b",
            "tkanz@andrew.cmu.edu",
            "\u{e9}t\u{e9}",
            "a/b",
        ] {
            assert_eq!(normalize_andrew_id(bad), None, "{bad:?}");
        }
        assert_eq!(normalize_andrew_id(&"a".repeat(65)), None);
    }
}
