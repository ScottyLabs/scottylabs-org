//! Projects: the public Projects page and project pages, and the portal's list and editor.
//! See "Projects" in docs/implementation/contract.md.
//!
//! Other route modules can list published projects as cards with `super::projects::public::published_cards`, for example for a committee page's projects block.

mod approval;
mod doc;
mod portal;
pub(super) mod public;
mod search;
#[cfg(test)]
mod tests;

use axum::Router;

use crate::state::AppState;

pub use approval::{apply_approval, apply_send_back};

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(public::router())
        .merge(portal::router())
}
