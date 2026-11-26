//! What visitors see: the Committees index and one page per committee, from published versions only.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderValue;
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{PageRow, all_pages, cleaned_for_reading, committee_from_slug, one_page};
use crate::blocks::{Block, CommitteePageDoc, CommitteeSettings};
use crate::error::{AppError, AppResult};
use crate::models::{Committee, ImageRef};
use crate::routes::projects::public::{ProjectCard, published_cards};
use crate::state::AppState;

/// Mirrors `CommitteeSummary` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitteeSummary {
    #[serde(flatten)]
    settings: CommitteeSettings,
    slug: Committee,
    name: String,
    color: String,
    tint: String,
    tagline: String,
    cover: Option<ImageRef>,
}

impl CommitteeSummary {
    fn new(row: &PageRow, published: &CommitteePageDoc) -> Self {
        let cover = published.cover();
        Self {
            settings: published.settings.clone(),
            slug: row.committee,
            name: row.name.clone(),
            color: row.color.clone(),
            tint: row.tint.clone(),
            tagline: cover.map(|c| c.tagline.clone()).unwrap_or_default(),
            cover: cover.and_then(|c| c.image.clone()),
        }
    }
}

#[derive(Serialize)]
struct PublicCommitteesResponse {
    committees: Vec<CommitteeSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicCommitteePageResponse {
    committee: CommitteeSummary,
    blocks: Vec<Block>,
    projects: Vec<ProjectCard>,
    published_at: Option<DateTime<Utc>>,
}

/// Cached briefly by browsers and the CDN, so a publish shows within about a minute, and readable from any site.
fn public_json(body: impl Serialize) -> Response {
    let mut response = Json(body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, stale-while-revalidate=600"),
    );
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    response
}

/// GET /api/public/committees: every committee with a published page, in the site's order.
pub(super) async fn list(State(state): State<AppState>) -> AppResult<Response> {
    let committees = all_pages(&state.db)
        .await?
        .iter()
        .filter_map(|row| {
            row.published
                .as_ref()
                .map(|published| CommitteeSummary::new(row, &published.0))
        })
        .collect();
    Ok(public_json(PublicCommitteesResponse { committees }))
}

/// GET /api/public/committees/:slug: the published blocks and the committee's published projects, the same cards as the Projects page. 404 until the page is published.
pub(super) async fn page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let committee = committee_from_slug(&slug)?;
    let mut conn = state.db.acquire().await?;
    let row = one_page(&mut conn, committee).await?;
    let Some(published) = row.published.as_ref().map(|json| &json.0) else {
        return Err(AppError::not_found("committee page"));
    };
    let projects = published_cards(&mut *conn, Some(committee)).await?;
    Ok(public_json(PublicCommitteePageResponse {
        committee: CommitteeSummary::new(&row, published),
        blocks: cleaned_for_reading(published.clone()).blocks,
        projects,
        published_at: row.published_at,
    }))
}
