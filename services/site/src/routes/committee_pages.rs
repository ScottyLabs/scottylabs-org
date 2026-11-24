//! Committee page endpoints, public and portal. Committees track. See docs/implementation/contract.md.

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}
