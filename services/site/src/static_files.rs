//! Serves the SvelteKit build with the fallback rules, cache headers and security headers in docs/implementation/contract.md ("Static serving").
//!
//! Files in `STATIC_DIR` are served as they are. Paths with no file get `200.html`, the SPA shell:
//! with status 200 and `no-store` under `/portal/`; with the project's title, description and Open
//! Graph tags at `/projects/<slug>/` (404 when no published project has that slug); and with status
//! 404 everywhere else, so SvelteKit renders its error page.
//!
//! The project tags carry [`INJECTED_ATTRIBUTE`], so the app removes them once it writes the head itself (`src/lib/site/head.ts`): crawlers that do not run scripts read them, and a visitor never gets two descriptions or the first project's tags after moving to another page.
//!
//! Every HTML page gets a Content-Security-Policy that allows its own inline scripts by hash (see [`content_security_policy`]).

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
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
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
/// Marks the tags [`inject_meta`] adds. The web app looks for the same name.
const INJECTED_ATTRIBUTE: &str = "data-sl-injected";
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
        Lookup::File(file) => serve_file(&state.config, &file, &path, request.headers()).await,
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

async fn serve_file(config: &Config, file: &Path, path: &str, request: &HeaderMap) -> Response {
    let Ok(bytes) = tokio::fs::read(file).await else {
        return plain(StatusCode::NOT_FOUND, "Not found");
    };
    let cache = cache_policy(path, file);
    let tag = (cache != IMMUTABLE).then(|| etag(&bytes));
    // A 304 carries it too: browsers update the stored response's headers from it.
    let policy = is_html(file)
        .then(|| page_policy(config, Audience::of(path), &String::from_utf8_lossy(&bytes)))
        .flatten();
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
    if let Some(value) = policy {
        headers.insert(CONTENT_SECURITY_POLICY, value);
    }
    response
}

fn is_html(file: &Path) -> bool {
    file.extension().is_some_and(|ext| ext == "html")
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
    let config = &state.config;
    let audience = Audience::of(path);
    if audience == Audience::Portal {
        return html(config, audience, StatusCode::OK, NO_STORE, shell);
    }
    let Some(slug) = project_slug(path) else {
        return html(config, audience, StatusCode::NOT_FOUND, REVALIDATE, shell);
    };
    if !path.ends_with('/') {
        return redirect_with_slash(raw_path, query);
    }
    // Anonymous visitors share the public slots with the public API (see `AppState::public_slots`).
    let looked_up = match state.public_slot().await {
        Some(_slot) => project_page(&state.db, slug).await,
        None => Err(sqlx::Error::PoolTimedOut),
    };
    match looked_up {
        Ok(ProjectPage::Published(meta)) => html(
            config,
            audience,
            StatusCode::OK,
            REVALIDATE,
            inject_meta(&shell, &meta, config),
        ),
        Ok(ProjectPage::Moved(new_slug)) => {
            let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
            if let Ok(value) = HeaderValue::from_str(&format!("/projects/{new_slug}/")) {
                response.headers_mut().insert(LOCATION, value);
            }
            response
        }
        Ok(ProjectPage::Missing) => {
            html(config, audience, StatusCode::NOT_FOUND, REVALIDATE, shell)
        }
        Err(error) => {
            tracing::error!(error = ?error, slug, "could not look up a project page");
            html(
                config,
                audience,
                StatusCode::SERVICE_UNAVAILABLE,
                NO_STORE,
                shell,
            )
        }
    }
}

fn html(
    config: &Config,
    audience: Audience,
    status: StatusCode,
    cache: &'static str,
    body: String,
) -> Response {
    let policy = page_policy(config, audience, &body);
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Some(value) = policy {
        headers.insert(CONTENT_SECURITY_POLICY, value);
    }
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

fn meta_tag(attribute: &str, key: &str, content: &str) -> String {
    format!(
        "<meta {attribute}=\"{key}\" content=\"{}\" {INJECTED_ATTRIBUTE}>",
        escape_html(content)
    )
}

/// The tags placed in the shell's `<head>` for a project page, each marked as injected. Every value is escaped.
fn meta_tags(meta: &ProjectMeta, config: &Config) -> String {
    let name = meta
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&meta.slug);
    // The same title the project page sets once it loads.
    let title = format!("{name} - {SITE_NAME}");
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
        }
        .readable();
        image.check(false).ok()?;
        let src = if image.media_id().is_some() {
            format!("{src}?w=1920")
        } else {
            src.to_owned()
        };
        Some((absolute(config, &src), image.alt))
    });

    let mut tags = vec![
        format!(
            "<title {INJECTED_ATTRIBUTE}>{}</title>",
            escape_html(&title)
        ),
        meta_tag("name", "description", &description),
        meta_tag("property", "og:site_name", SITE_NAME),
        meta_tag("property", "og:type", "website"),
        meta_tag("property", "og:title", &title),
        meta_tag("property", "og:description", &description),
        meta_tag("property", "og:url", &url),
        format!(
            "<link rel=\"canonical\" href=\"{}\" {INJECTED_ATTRIBUTE}>",
            escape_html(&url)
        ),
    ];
    match image {
        Some((src, alt)) => {
            tags.push(meta_tag("property", "og:image", &src));
            if !alt.trim().is_empty() {
                tags.push(meta_tag("property", "og:image:alt", alt.trim()));
            }
            tags.push(meta_tag("name", "twitter:card", "summary_large_image"));
        }
        None => tags.push(meta_tag("name", "twitter:card", "summary")),
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
    let path = request.uri().path();
    let is_api = path.starts_with("/api/");
    let audience = Audience::of(path);
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
    // HTML pages set their own, with their inline scripts' hashes.
    if let Ok(value) = HeaderValue::from_str(&content_security_policy(&state.config, audience, &[]))
    {
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

/// Who a page is for. Public pages may send page views to PostHog. The portal never loads it (`src/lib/site/analytics.ts`), so its pages may not connect to the analytics host either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    Public,
    Portal,
}

impl Audience {
    pub fn of(path: &str) -> Self {
        if is_portal(path) {
            Self::Portal
        } else {
            Self::Public
        }
    }
}

/// Team and history photos (`src/lib/data/README.md`). Uploads are served from this origin under `/media/`.
const IMAGE_HOSTS: [&str; 1] = ["https://assets.scottylabs.org"];

/// The video players, which load only after a visitor presses Play (`src/lib/media/video.ts`), and the events calendar on the home page.
const FRAME_HOSTS: [&str; 3] = [
    "https://www.youtube-nocookie.com",
    "https://player.vimeo.com",
    "https://calendar.google.com",
];

/// The hash of `this.__e=event`, the only inline event handler allowed (with `'unsafe-hashes'`). Svelte writes it on prerendered images that have an `onload` or `onerror` handler, to keep the event for after hydration; the same pair is in `csp` in `vite.config.ts`.
const SVELTE_EVENT_REPLAY: &str = "sha256-7dQwUgLau1NFCCGjfn9FsYptB6ZtWxJin6VohGIu20I=";

/// The policy for an HTML page, allowing the inline scripts it carries.
fn page_policy(config: &Config, audience: Audience, html: &str) -> Option<HeaderValue> {
    let hashes = inline_script_hashes(html);
    HeaderValue::from_str(&content_security_policy(config, audience, &hashes)).ok()
}

/// The Content-Security-Policy. Scripts come only from this origin, plus the inline scripts whose `script_hashes` (`sha256-...`, from [`inline_script_hashes`]) are given: SvelteKit starts each page with one, and writes the same hashes into a `<meta>` policy of its own (`csp` in `vite.config.ts`). Browsers enforce both policies. No other inline code runs, except Svelte's one-statement event handler ([`SVELTE_EVENT_REPLAY`]).
///
/// Style attributes are allowed inline (`style-src-attr`): Svelte writes them into prerendered pages and into the markup it creates in the browser, for `style:` directives and the like, and a style attribute cannot load anything that the other directives refuse. Stylesheets and `<style>` elements must come from this origin.
///
/// Connections go to this origin, and from public pages also to the PostHog host when analytics are on. Frames, plugins, `<base>` and form targets are locked down as before, with the Keycloak origin as a form target when sign-in is configured.
pub fn content_security_policy(
    config: &Config,
    audience: Audience,
    script_hashes: &[String],
) -> String {
    let mut script_src = vec![
        "'self'".to_owned(),
        "'unsafe-hashes'".to_owned(),
        format!("'{SVELTE_EVENT_REPLAY}'"),
    ];
    script_src.extend(script_hashes.iter().map(|hash| format!("'{hash}'")));
    let mut connect_src = vec!["'self'"];
    if audience == Audience::Public
        && let Some(origin) = config.analytics_origin.as_deref()
    {
        connect_src.push(origin);
    }
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
    [
        "default-src 'self'".to_owned(),
        format!("script-src {}", script_src.join(" ")),
        "style-src 'self'".to_owned(),
        "style-src-attr 'unsafe-inline'".to_owned(),
        format!("img-src 'self' data: blob: {}", IMAGE_HOSTS.join(" ")),
        format!("connect-src {}", connect_src.join(" ")),
        format!("frame-src {}", FRAME_HOSTS.join(" ")),
        "frame-ancestors 'none'".to_owned(),
        "base-uri 'self'".to_owned(),
        format!("form-action {form_action}"),
        "object-src 'none'".to_owned(),
    ]
    .join("; ")
}

/// `sha256-<base64>` for each script element in `html` that runs inline code: no `src`, and no `type` other than JavaScript or a module. Data blocks such as SvelteKit's `<script type="application/json">` never run, so they need no hash.
pub fn inline_script_hashes(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut hashes = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find("<script") {
        let start = from + found;
        let after_name = start + "<script".len();
        // `<scripts>` or `<script-x>` is another element.
        if !lower[after_name..]
            .starts_with(|c: char| c == '>' || c == '/' || c.is_ascii_whitespace())
        {
            from = after_name;
            continue;
        }
        let Some(tag_end) = tag_end(&lower, after_name) else {
            break;
        };
        let content_start = tag_end + 1;
        let Some(length) = lower[content_start..].find("</script") else {
            break;
        };
        let content_end = content_start + length;
        if runs_inline(&lower[after_name..tag_end]) {
            let digest = Sha256::digest(&html.as_bytes()[content_start..content_end]);
            hashes.push(format!("sha256-{}", BASE64.encode(digest)));
        }
        from = content_end;
    }
    hashes
}

/// The index of the `>` that closes a start tag whose attributes begin at `from`, skipping quoted values.
fn tag_end(html: &str, from: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, byte) in html.as_bytes()[from..].iter().enumerate() {
        match (quote, byte) {
            (None, b'"' | b'\'') => quote = Some(*byte),
            (Some(open), _) if *byte == open => quote = None,
            (None, b'>') => return Some(from + offset),
            _ => {}
        }
    }
    None
}

/// Whether a script with these (lowercased) attributes runs the code inside it.
fn runs_inline(attributes: &str) -> bool {
    let attributes = parse_attributes(attributes);
    if attributes.iter().any(|(name, _)| name == "src") {
        return false;
    }
    match attributes.iter().find(|(name, _)| name == "type") {
        None => true,
        Some((_, value)) => matches!(
            value.trim(),
            "" | "module" | "text/javascript" | "application/javascript"
        ),
    }
}

/// Attribute names and values, enough to tell `src` and `type` apart from text inside other values.
fn parse_attributes(text: &str) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    let mut rest = text.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
    while !rest.is_empty() {
        let name_end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        let name = rest[..name_end].to_owned();
        rest = rest[name_end..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        let mut value = String::new();
        if let Some(after_equals) = rest.strip_prefix('=') {
            let after_equals = after_equals.trim_start_matches(|c: char| c.is_ascii_whitespace());
            let (found, remaining) = match after_equals.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let inner = &after_equals[1..];
                    let close = inner.find(quote).unwrap_or(inner.len());
                    (&inner[..close], inner.get(close + 1..).unwrap_or(""))
                }
                _ => {
                    let end = after_equals
                        .find(|c: char| c.is_ascii_whitespace())
                        .unwrap_or(after_equals.len());
                    after_equals.split_at(end)
                }
            };
            value = found.to_owned();
            rest = remaining;
        }
        if !name.is_empty() {
            attributes.push((name, value));
        }
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
    }
    attributes
}

#[cfg(test)]
mod tests;
