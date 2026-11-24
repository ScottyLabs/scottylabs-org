//! What visitors see: GET /api/public/projects and GET /api/public/projects/:slug. Only published projects, read from their published version.

use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderValue;
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::types::Json as JsonColumn;
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use super::doc::{ProjectDoc, ProjectKind, ProjectStatus, check_slug};
use crate::error::{AppError, AppResult};
use crate::models::{Committee, ImageRef};
use crate::state::AppState;

const PUBLIC_CACHE: &str = "public, max-age=60, stale-while-revalidate=600";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/public/projects", get(list))
        .route("/public/projects/{slug}", get(detail))
}

/// A project on the Projects page and on committee pages. Mirrors `ProjectCard` in types.ts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCard {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub excerpt: String,
    pub committee: Committee,
    pub status: ProjectStatus,
    pub kind: ProjectKind,
    pub cover: Option<ImageRef>,
    pub site_url: Option<String>,
    pub repo_url: Option<String>,
    pub featured: bool,
}

impl ProjectCard {
    pub fn new(id: Uuid, slug: String, featured: bool, doc: &ProjectDoc) -> Self {
        Self {
            id,
            slug,
            name: doc.name.clone(),
            summary: doc.summary.clone(),
            excerpt: doc.excerpt(),
            committee: doc.committee,
            status: doc.status,
            kind: doc.kind,
            cover: doc.cover().cloned(),
            site_url: doc.site_url.clone(),
            repo_url: doc.repo_url.clone(),
            featured,
        }
    }
}

/// One project page. Mirrors `ProjectDetail` in types.ts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDetail {
    #[serde(flatten)]
    card: ProjectCard,
    description: String,
    built_with: Vec<Committee>,
    media: Vec<ImageRef>,
    video_url: Option<String>,
    published_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct PublicProjectsResponse {
    projects: Vec<ProjectCard>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicProjectResponse {
    project: Option<ProjectDetail>,
    redirect_to: Option<String>,
}

#[derive(FromRow)]
struct PublishedRow {
    id: Uuid,
    slug: String,
    featured: bool,
    published: JsonColumn<ProjectDoc>,
    published_at: Option<DateTime<Utc>>,
}

const PUBLISHED_COLUMNS: &str = "id, slug, featured, published, published_at";

/// Published projects, featured first and then by name, optionally only those owned by one committee. For the Projects page and the committees track's projects block.
pub async fn published_cards<'e>(
    db: impl PgExecutor<'e>,
    committee: Option<Committee>,
) -> AppResult<Vec<ProjectCard>> {
    let rows = sqlx::query_as::<_, PublishedRow>(&format!(
        "SELECT {PUBLISHED_COLUMNS} FROM projects
         WHERE state = 'published' AND published IS NOT NULL
           AND ($1::text IS NULL OR committee = $1)
         ORDER BY featured DESC, lower(published->>'name'), slug"
    ))
    .bind(committee)
    .fetch_all(db)
    .await?;
    Ok(rows
        .iter()
        .map(|row| ProjectCard::new(row.id, row.slug.clone(), row.featured, &row.published))
        .collect())
}

/// Public answers can be cached for a minute and read from any site, errors included.
fn public(result: AppResult<Response>) -> Response {
    let mut response = result.unwrap_or_else(IntoResponse::into_response);
    let success = response.status().is_success();
    let headers = response.headers_mut();
    if success {
        headers.insert(CACHE_CONTROL, HeaderValue::from_static(PUBLIC_CACHE));
    }
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    response
}

async fn list(State(state): State<AppState>, RawQuery(query): RawQuery) -> Response {
    public(list_projects(&state, query.as_deref()).await)
}

async fn list_projects(state: &AppState, query: Option<&str>) -> AppResult<Response> {
    let committee =
        match query_param(query, "committee") {
            None => None,
            Some(slug) => Some(Committee::from_slug(&slug).ok_or_else(|| {
                AppError::field("committee", "Choose one of the seven committees.")
            })?),
        };
    let projects = published_cards(&state.db, committee).await?;
    Ok(Json(PublicProjectsResponse { projects }).into_response())
}

async fn detail(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    public(project_page(&state, &slug).await)
}

async fn project_page(state: &AppState, raw_slug: &str) -> AppResult<Response> {
    let not_found = || AppError::NotFound("There is no project at that address.".into());
    let slug = check_slug(raw_slug).map_err(|_| not_found())?;
    if slug != raw_slug {
        return Err(not_found());
    }
    let row = sqlx::query_as::<_, PublishedRow>(&format!(
        "SELECT {PUBLISHED_COLUMNS} FROM projects
         WHERE slug = $1 AND state = 'published' AND published IS NOT NULL"
    ))
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?;
    if let Some(row) = row {
        let doc = &row.published;
        let body = PublicProjectResponse {
            project: Some(ProjectDetail {
                card: ProjectCard::new(row.id, row.slug.clone(), row.featured, doc),
                description: doc.description.clone(),
                built_with: doc.built_with.clone(),
                media: doc.media.clone(),
                video_url: doc.video_url.clone(),
                published_at: row.published_at.unwrap_or_else(Utc::now),
            }),
            redirect_to: None,
        };
        return Ok(Json(body).into_response());
    }
    let moved: Option<String> = sqlx::query_scalar(
        "SELECT p.slug FROM project_slug_redirects r JOIN projects p ON p.id = r.project_id
         WHERE r.old_slug = $1 AND p.state = 'published' AND p.published IS NOT NULL",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?;
    match moved {
        Some(new_slug) => Ok(Json(PublicProjectResponse {
            project: None,
            redirect_to: Some(format!("/projects/{new_slug}/")),
        })
        .into_response()),
        None => Err(not_found()),
    }
}

/// One query parameter, trimmed, or `None` when missing or empty.
pub fn query_param(raw: Option<&str>, name: &str) -> Option<String> {
    url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
