//! Extractors for [`Viewer`] and [`PortalViewer`]. Platform track: load the session from the cookie, check expiry, refresh `last_seen_at`, and load roles and lead projects.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use super::{PortalViewer, Viewer};
use crate::error::AppError;
use crate::state::AppState;

impl FromRequestParts<AppState> for Viewer {
    type Rejection = AppError;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Err(AppError::Unauthenticated)
    }
}

impl FromRequestParts<AppState> for PortalViewer {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let viewer = Viewer::from_request_parts(parts, state).await?;
        if crate::permissions::can_use_portal(&viewer) {
            Ok(Self(viewer))
        } else {
            Err(AppError::Forbidden(
                "Your account has no role in the portal yet. Ask the director of your committee to add you.".into(),
            ))
        }
    }
}
