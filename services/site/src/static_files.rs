//! Serves the SvelteKit build with the fallback rules, cache headers and security headers in docs/implementation/contract.md ("Static serving").
//!
//! Files in `STATIC_DIR` are served as they are. Paths with no file get `200.html`, the SPA shell:
//! with status 200 and `no-store` under `/portal/`; with the project's title, description and Open
//! Graph tags at `/projects/<slug>/` (404 when no published project has that slug); and with status
//! 404 everywhere else, so SvelteKit renders its error page.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{
    ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_SECURITY_POLICY, CONTENT_TYPE, ETAG,
    IF_NONE_MATCH, LOCATION, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
    X_FRAME_OPTIONS,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};

use crate::config::Config;
use crate::models::ImageRef;
use crate::state::AppState;

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const REVALIDATE: &str = "no-cache";
const ONE_HOUR: &str = "public, max-age=3600";
const NO_STORE: &str = "no-store";
const FALLBACK_PAGE: &str = "200.html";
const SITE_NAME: &str = "ScottyLabs";
const MAX_SLUG_LEN: usize = 100;

pub async fn serve(State(state): State<AppState>, request: Request) -> Response {
    let Some(root) = state.config.static_dir.as_deref() else {
        return plain(StatusCode::NOT_FOUND, "Not found");
    };
    let method = request.method().clone();
    if method != Method::GET && method != Method::HEAD {
        let mut response = plain(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed");
        response
            .headers_mut()
            .insert(ALLOW, HeaderValue::from_static("GET, HEAD"));
        return response;
    }
    let raw_path = request.uri().path();
    let query = request.uri().query();
    let Some(path) = clean_path(raw_path) else {
        return plain(StatusCode::BAD_REQUEST, "Bad request");
    };

    let response = match lookup(root, &path).await {
        Lookup::File(file) => serve_file(&file, &path, request.headers()).await,
        Lookup::AddSlash => redirect_with_slash(raw_path, query),
        Lookup::Missing => fallback(&state, root, &path, raw_path, query).await,
    };
    if method == Method::HEAD {
        let (parts, _) = response.into_parts();
        return Response::from_parts(parts, Body::empty());
    }
    response
}

fn plain(status: StatusCode, text: &'static str) -> Response {
    let mut response = (status, text).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(NO_STORE));
    response
}

/// Decodes percent-encoding once and refuses anything that could leave the build directory:
/// `..` or `.` segments, backslashes, NUL, a leading `//`, and invalid UTF-8.
pub fn clean_path(raw: &str) -> Option<String> {
    if !raw.starts_with('/') || raw.starts_with("//") {
        return None;
    }
    let decoded = percent_decode(raw)?;
    if decoded.contains(['\0', '\\']) {
        return None;
    }
    if decoded
        .split('/')
        .any(|segment| segment == ".." || segment == ".")
    {
        return None;
    }
    Some(decoded)
}

fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

enum Lookup {
    File(PathBuf),
    /// A directory with an index.html, asked for without the trailing slash.
    AddSlash,
    Missing,
}

async fn lookup(root: &Path, path: &str) -> Lookup {
    let relative = path.trim_start_matches('/');
    let candidate = if path.ends_with('/') {
        root.join(relative).join("index.html")
    } else {
        root.join(relative)
    };
    let Ok(metadata) = tokio::fs::metadata(&candidate).await else {
        return Lookup::Missing;
    };
    if metadata.is_dir() {
        let has_index = tokio::fs::metadata(candidate.join("index.html"))
            .await
            .is_ok_and(|m| m.is_file());
        return if has_index {
            Lookup::AddSlash
        } else {
            Lookup::Missing
        };
    }
    if !metadata.is_file() || !inside(root, &candidate).await {
        return Lookup::Missing;
    }
    Lookup::File(candidate)
}

/// Whether `candidate` resolves inside `root`, following symlinks.
async fn inside(root: &Path, candidate: &Path) -> bool {
    match (
        tokio::fs::canonicalize(root).await,
        tokio::fs::canonicalize(candidate).await,
    ) {
        (Ok(root), Ok(candidate)) => candidate.starts_with(root),
        _ => false,
    }
}

fn redirect_with_slash(raw_path: &str, query: Option<&str>) -> Response {
    let location = match query {
        Some(query) => format!("{raw_path}/?{query}"),
        None => format!("{raw_path}/"),
    };
    let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response.headers_mut().insert(LOCATION, value);
    }
    response
}

fn cache_policy(path: &str, file: &Path) -> &'static str {
    if path.starts_with("/_app/immutable/") {
        IMMUTABLE
    } else if file.extension().is_some_and(|ext| ext == "html") {
        REVALIDATE
    } else {
        ONE_HOUR
    }
}

fn content_type(file: &Path) -> String {
    let mime = mime_guess::from_path(file).first_or_octet_stream();
    let textual = mime.type_() == mime_guess::mime::TEXT
        || matches!(
            mime.subtype().as_str(),
            "javascript" | "json" | "xml" | "manifest+json"
        );
    if textual && mime.get_param("charset").is_none() {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

/// A strong validator from the content. File times are useless here: the Nix store sets them all to 1970.
fn etag(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

fn matches_etag(request: &HeaderMap, tag: &str) -> bool {
    request
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|candidate| candidate.trim().trim_start_matches("W/"))
        .any(|candidate| candidate == tag || candidate == "*")
}

async fn serve_file(file: &Path, path: &str, request: &HeaderMap) -> Response {
    let Ok(bytes) = tokio::fs::read(file).await else {
        return plain(StatusCode::NOT_FOUND, "Not found");
    };
    let cache = cache_policy(path, file);
    let tag = (cache != IMMUTABLE).then(|| etag(&bytes));
    let mut response = match &tag {
        Some(tag) if matches_etag(request, tag) => StatusCode::NOT_MODIFIED.into_response(),
        _ => {
            let length = bytes.len();
            let mut response = Body::from(bytes).into_response();
            let headers = response.headers_mut();
            if let Ok(value) = HeaderValue::from_str(&content_type(file)) {
                headers.insert(CONTENT_TYPE, value);
            }
            headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
            response
        }
    };
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Some(value) = tag.and_then(|tag| HeaderValue::from_str(&tag).ok()) {
        headers.insert(ETAG, value);
    }
    response
}

/// The slug of a project page path, `/projects/<slug>/` or `/projects/<slug>`.
fn project_slug(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/projects/")?;
    let slug = rest.strip_suffix('/').unwrap_or(rest);
    let valid = !slug.is_empty()
        && slug.len() <= MAX_SLUG_LEN
        && slug.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
    valid.then_some(slug)
}

fn is_portal(path: &str) -> bool {
    path == "/portal" || path.starts_with("/portal/")
}

async fn fallback(
    state: &AppState,
    root: &Path,
    path: &str,
    raw_path: &str,
    query: Option<&str>,
) -> Response {
    let Ok(shell) = tokio::fs::read_to_string(root.join(FALLBACK_PAGE)).await else {
        tracing::error!(dir = %root.display(), "the build has no {FALLBACK_PAGE}");
        return plain(StatusCode::NOT_FOUND, "Not found");
    };
    if is_portal(path) {
        return html(StatusCode::OK, NO_STORE, shell);
    }
    let Some(slug) = project_slug(path) else {
        return html(StatusCode::NOT_FOUND, REVALIDATE, shell);
    };
    if !path.ends_with('/') {
        return redirect_with_slash(raw_path, query);
    }
    match project_page(&state.db, slug).await {
        Ok(ProjectPage::Published(meta)) => html(
            StatusCode::OK,
            REVALIDATE,
            inject_meta(&shell, &meta, &state.config),
        ),
        Ok(ProjectPage::Moved(new_slug)) => {
            let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
            if let Ok(value) = HeaderValue::from_str(&format!("/projects/{new_slug}/")) {
                response.headers_mut().insert(LOCATION, value);
            }
            response
        }
        Ok(ProjectPage::Missing) => html(StatusCode::NOT_FOUND, REVALIDATE, shell),
        Err(error) => {
            tracing::error!(error = ?error, slug, "could not look up a project page");
            html(StatusCode::SERVICE_UNAVAILABLE, NO_STORE, shell)
        }
    }
}

fn html(status: StatusCode, cache: &'static str, body: String) -> Response {
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

/// What a project page's head says about it.
#[derive(Debug, FromRow)]
pub struct ProjectMeta {
    pub slug: String,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub cover_src: Option<String>,
    pub cover_alt: Option<String>,
}

enum ProjectPage {
    Published(ProjectMeta),
    /// The slug was renamed; the project lives at the new one.
    Moved(String),
    Missing,
}

async fn project_page(db: &PgPool, slug: &str) -> Result<ProjectPage, sqlx::Error> {
    let meta = sqlx::query_as::<_, ProjectMeta>(
        "SELECT slug, published->>'name' AS name, published->>'summary' AS summary,
                published->'media'->0->>'src' AS cover_src, published->'media'->0->>'alt' AS cover_alt
         FROM projects
         WHERE slug = $1 AND state = 'published' AND published IS NOT NULL",
    )
    .bind(slug)
    .fetch_optional(db)
    .await?;
    if let Some(meta) = meta {
        return Ok(ProjectPage::Published(meta));
    }
    let moved: Option<String> = sqlx::query_scalar(
        "SELECT p.slug FROM project_slug_redirects r JOIN projects p ON p.id = r.project_id
         WHERE r.old_slug = $1 AND p.state = 'published' AND p.published IS NOT NULL",
    )
    .bind(slug)
    .fetch_optional(db)
    .await?;
    Ok(moved.map_or(ProjectPage::Missing, ProjectPage::Moved))
}

/// Escapes text for HTML element content and quoted attribute values.
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn absolute(config: &Config, path: &str) -> String {
    format!("{}{path}", config.origin())
}

/// The tags placed in the shell's `<head>` for a project page. Every value is escaped.
fn meta_tags(meta: &ProjectMeta, config: &Config) -> String {
    let name = meta
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&meta.slug);
    let title = format!("{name} | {SITE_NAME}");
    let description = meta
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
        .map_or_else(|| format!("{name}, a {SITE_NAME} project."), str::to_owned);
    let url = absolute(config, &format!("/projects/{}/", meta.slug));
    let image = meta.cover_src.as_deref().and_then(|src| {
        let image = ImageRef {
            src: src.to_owned(),
            alt: meta.cover_alt.clone().unwrap_or_default(),
            width: None,
            height: None,
        };
        image.check(false).ok()?;
        let src = if image.media_id().is_some() {
            format!("{src}?w=1920")
        } else {
            src.to_owned()
        };
        Some((absolute(config, &src), image.alt))
    });

    let mut tags = vec![
        format!("<title>{}</title>", escape_html(&title)),
        format!(
            "<meta name=\"description\" content=\"{}\">",
            escape_html(&description)
        ),
        format!("<meta property=\"og:site_name\" content=\"{SITE_NAME}\">"),
        "<meta property=\"og:type\" content=\"website\">".to_owned(),
        format!(
            "<meta property=\"og:title\" content=\"{}\">",
            escape_html(&title)
        ),
        format!(
            "<meta property=\"og:description\" content=\"{}\">",
            escape_html(&description)
        ),
        format!(
            "<meta property=\"og:url\" content=\"{}\">",
            escape_html(&url)
        ),
        format!("<link rel=\"canonical\" href=\"{}\">", escape_html(&url)),
    ];
    match image {
        Some((src, alt)) => {
            tags.push(format!(
                "<meta property=\"og:image\" content=\"{}\">",
                escape_html(&src)
            ));
            if !alt.trim().is_empty() {
                tags.push(format!(
                    "<meta property=\"og:image:alt\" content=\"{}\">",
                    escape_html(alt.trim())
                ));
            }
            tags.push("<meta name=\"twitter:card\" content=\"summary_large_image\">".to_owned());
        }
        None => tags.push("<meta name=\"twitter:card\" content=\"summary\">".to_owned()),
    }
    tags.join("\n    ")
}

/// Puts the project's tags into the shell's head, replacing any title it already has.
pub fn inject_meta(shell: &str, meta: &ProjectMeta, config: &Config) -> String {
    let tags = meta_tags(meta, config);
    let without_title = match (shell.find("<title>"), shell.find("</title>")) {
        (Some(start), Some(end)) if end > start => {
            format!("{}{}", &shell[..start], &shell[end + "</title>".len()..])
        }
        _ => shell.to_owned(),
    };
    match without_title.find("</head>") {
        Some(at) => format!(
            "{}    {tags}\n  {}",
            &without_title[..at],
            &without_title[at..]
        ),
        None => without_title,
    }
}

/// Security headers on every response, without overriding ones a handler set. API responses that did not choose a cache policy are not cached.
pub async fn security_headers(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let is_api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    set_default(headers, X_CONTENT_TYPE_OPTIONS, "nosniff");
    set_default(headers, REFERRER_POLICY, "strict-origin-when-cross-origin");
    set_default(headers, X_FRAME_OPTIONS, "DENY");
    set_default(
        headers,
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=()",
    );
    if let Ok(value) = HeaderValue::from_str(&content_security_policy(&state.config)) {
        headers.entry(CONTENT_SECURITY_POLICY).or_insert(value);
    }
    if state.config.is_https() {
        set_default(headers, STRICT_TRANSPORT_SECURITY, "max-age=31536000");
    }
    if is_api {
        set_default(headers, CACHE_CONTROL, NO_STORE);
    }
    response
}

fn set_default(headers: &mut HeaderMap, name: HeaderName, value: &'static str) {
    headers
        .entry(name)
        .or_insert(HeaderValue::from_static(value));
}

/// Locks down framing, plugins, `<base>` and form targets. Scripts are not restricted yet, because
/// SvelteKit inlines its start-up script.
pub fn content_security_policy(config: &Config) -> String {
    let keycloak = config
        .oidc
        .as_ref()
        .and_then(|oidc| url::Url::parse(&oidc.keycloak_url).ok())
        .map(|url| url.origin().ascii_serialization())
        .filter(|origin| origin != "null");
    let form_action = match keycloak {
        Some(origin) => format!("'self' {origin}"),
        None => "'self'".to_owned(),
    };
    format!("frame-ancestors 'none'; base-uri 'self'; object-src 'none'; form-action {form_action}")
}

#[cfg(test)]
mod tests;
