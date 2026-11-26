use std::path::PathBuf;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::*;
use crate::models::Committee;
use crate::testing::{self, Req};

const SHELL: &str = "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\" />\n  </head>\n  <body><div id=\"app\"></div></body>\n</html>\n";

/// A small SvelteKit-like build in a fresh directory, removed when dropped.
struct Build(PathBuf);

impl Build {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("site-static-{}", uuid::Uuid::now_v7()));
        let write = |path: &str, body: &str| {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, body).unwrap();
        };
        write("200.html", SHELL);
        write("index.html", "<!doctype html><title>Home</title>");
        write("team/index.html", "<!doctype html><title>Team</title>");
        write(
            "_app/immutable/entry/start.abc123.js",
            "export const x = 1;",
        );
        write("robots.txt", "User-agent: *");
        write("seed/projects/nova.jpg", "not really a jpeg");
        std::fs::create_dir_all(root.join("empty-dir")).unwrap();
        Self(root)
    }
}

impl Drop for Build {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn app_for(db: &PgPool, build: &Build) -> axum::Router {
    let mut config = testing::config();
    config.static_dir = Some(build.0.clone());
    testing::app_with(db.clone(), config).await
}

#[test]
fn paths_are_decoded_once_and_checked() {
    assert_eq!(clean_path("/").as_deref(), Some("/"));
    assert_eq!(clean_path("/team/").as_deref(), Some("/team/"));
    assert_eq!(clean_path("/a%20b").as_deref(), Some("/a b"));
    // Decoded once: a double-encoded dot-dot stays literal text, which matches no file.
    assert_eq!(clean_path("/%252e%252e/x").as_deref(), Some("/%2e%2e/x"));
    for bad in [
        "/../Cargo.toml",
        "/%2e%2e/Cargo.toml",
        "/team/%2E%2E/%2e%2e/etc/passwd",
        "/./index.html",
        "/a%00b",
        "/a%5c..%5cb",
        "/a\\b",
        "//evil.example/",
        "/bad%zz",
        "/%ff%fe",
        "relative",
    ] {
        assert_eq!(clean_path(bad), None, "{bad}");
    }
}

#[test]
fn project_slugs_are_strict() {
    assert_eq!(project_slug("/projects/nova/"), Some("nova"));
    assert_eq!(project_slug("/projects/cmu-courses"), Some("cmu-courses"));
    for bad in [
        "/projects/",
        "/projects/Nova/",
        "/projects/a--b/",
        "/projects/-a/",
        "/projects/a/b/",
        "/project/nova/",
    ] {
        assert_eq!(project_slug(bad), None, "{bad}");
    }
}

#[test]
fn injected_meta_is_escaped() {
    let config = testing::config();
    let meta = ProjectMeta {
        slug: "nova".into(),
        name: Some("<script>alert(\"x\")</script> & co".into()),
        summary: Some("It's \"quoted\" <b>bold</b>".into()),
        cover_src: Some("/seed/projects/nova.jpg".into()),
        cover_alt: Some("A \"stage\" & lights".into()),
    };
    let html = inject_meta(SHELL, &meta, &config);
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<b>"));
    assert!(html.contains(
        "<title data-sl-injected>&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; co - ScottyLabs</title>"
    ));
    assert!(html.contains("<meta name=\"description\" content=\"It&#39;s &quot;quoted&quot; &lt;b&gt;bold&lt;/b&gt;\" data-sl-injected>"));
    assert!(html.contains(
        "<meta property=\"og:image\" content=\"http://localhost:5173/seed/projects/nova.jpg\" data-sl-injected>"
    ));
    assert!(html.contains(
        "<meta property=\"og:image:alt\" content=\"A &quot;stage&quot; &amp; lights\" data-sl-injected>"
    ));
    assert!(html.contains(
        "<meta property=\"og:url\" content=\"http://localhost:5173/projects/nova/\" data-sl-injected>"
    ));
    assert!(html.contains(
        "<link rel=\"canonical\" href=\"http://localhost:5173/projects/nova/\" data-sl-injected>"
    ));
    // The tags land inside the head, once.
    let head_end = html.find("</head>").unwrap();
    assert!(html.find("<title").unwrap() < head_end);
    assert_eq!(html.matches("<title").count(), 1);
}

/// The app removes what the service added by this attribute, so every tag it adds must carry it.
#[test]
fn every_injected_tag_is_marked() {
    let config = testing::config();
    let meta = ProjectMeta {
        slug: "nova".into(),
        name: Some("Nova".into()),
        summary: Some("A hackathon for first-timers".into()),
        cover_src: Some("/seed/projects/nova.jpg".into()),
        cover_alt: Some("Students presenting".into()),
    };
    let html = inject_meta(SHELL, &meta, &config);
    let added: Vec<&str> = html
        .lines()
        .map(str::trim)
        .filter(|line| !SHELL.lines().any(|shell_line| shell_line.trim() == *line))
        .collect();
    assert_eq!(added.len(), 11, "{added:?}");
    for tag in added {
        assert!(tag.contains(" data-sl-injected"), "{tag}");
    }
    assert!(!SHELL.contains("data-sl-injected"));
}

#[test]
fn covers_from_elsewhere_are_not_advertised() {
    let config = testing::config();
    let meta = ProjectMeta {
        slug: "nova".into(),
        name: Some("Nova".into()),
        summary: None,
        cover_src: Some("javascript:alert(1)".into()),
        cover_alt: None,
    };
    let html = inject_meta("<head><title>Old</title></head>", &meta, &config);
    assert!(!html.contains("og:image"));
    assert!(!html.contains("Old"));
    assert!(html.contains("content=\"Nova, a ScottyLabs project.\""));
    assert!(html.contains("<meta name=\"twitter:card\" content=\"summary\" data-sl-injected>"));

    let uploaded = ProjectMeta {
        cover_src: Some("/media/0190e0a4-7c1b-7d2e-9f00-000000000001".into()),
        ..meta
    };
    let html = inject_meta(SHELL, &uploaded, &config);
    assert!(html.contains("/media/0190e0a4-7c1b-7d2e-9f00-000000000001?w=1920"));
}

#[sqlx::test(migrations = "./migrations")]
async fn serves_files_with_cache_rules(db: PgPool) {
    let build = Build::new();
    let app = app_for(&db, &build).await;

    let home = Req::get("/").send(&app).await;
    assert_eq!(home.status, StatusCode::OK);
    assert_eq!(home.text(), "<!doctype html><title>Home</title>");
    assert_eq!(
        home.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(home.header("cache-control"), Some("no-cache"));
    let etag = home.header("etag").unwrap().to_owned();
    assert!(home.header("last-modified").is_none());

    let again = Req::get("/")
        .header("if-none-match", &etag)
        .send(&app)
        .await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    assert!(again.body.is_empty());

    let script = Req::get("/_app/immutable/entry/start.abc123.js")
        .send(&app)
        .await;
    assert_eq!(script.status, StatusCode::OK);
    assert_eq!(
        script.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );
    assert!(
        script
            .header("content-type")
            .unwrap()
            .contains("javascript")
    );

    let robots = Req::get("/robots.txt").send(&app).await;
    assert_eq!(robots.header("cache-control"), Some("public, max-age=3600"));
    assert_eq!(
        robots.header("content-type"),
        Some("text/plain; charset=utf-8")
    );

    let head = Req::head("/robots.txt").send(&app).await;
    assert_eq!(head.status, StatusCode::OK);
    assert!(head.body.is_empty());
    assert_eq!(head.header("content-length"), Some("13"));

    let post = Req::post("/").send(&app).await;
    assert_eq!(post.status, StatusCode::METHOD_NOT_ALLOWED);
}

#[sqlx::test(migrations = "./migrations")]
async fn directories_redirect_to_the_slash_form(db: PgPool) {
    let build = Build::new();
    let app = app_for(&db, &build).await;
    let res = Req::get("/team").send(&app).await;
    assert_eq!(res.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(res.header("location"), Some("/team/"));
    let res = Req::get("/team?ref=nav").send(&app).await;
    assert_eq!(res.header("location"), Some("/team/?ref=nav"));
    let res = Req::get("/team/").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.text().contains("Team"));
    // A directory without an index is not a page.
    let res = Req::get("/empty-dir").send(&app).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn traversal_never_leaves_the_build(db: PgPool) {
    let build = Build::new();
    let outside = build.0.with_extension("secret");
    std::fs::write(&outside, "top secret").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, build.0.join("link.txt")).unwrap();
    let app = app_for(&db, &build).await;
    let name = outside.file_name().unwrap().to_str().unwrap().to_owned();

    for path in [
        format!("/../{name}"),
        format!("/%2e%2e/{name}"),
        format!("/team/%2e%2e/%2e%2e/{name}"),
        "/%2e%2e%2f%2e%2e%2fetc%2fpasswd".to_owned(),
        "/a%00.html".to_owned(),
        "/..%5c..%5cetc%5cpasswd".to_owned(),
    ] {
        let res = Req::get(&path).send(&app).await;
        assert!(!res.text().contains("top secret"), "{path}");
        assert!(
            matches!(res.status, StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND),
            "{path}: {}",
            res.status
        );
    }
    #[cfg(unix)]
    {
        let res = Req::get("/link.txt").send(&app).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND);
        assert!(!res.text().contains("top secret"));
    }
    let _ = std::fs::remove_file(outside);
}

#[sqlx::test(migrations = "./migrations")]
async fn fallback_status_follows_the_path(db: PgPool) {
    let build = Build::new();
    let app = app_for(&db, &build).await;

    for path in [
        "/portal",
        "/portal/",
        "/portal/ticker/",
        "/portal/projects/0190e0a4/",
    ] {
        let res = Req::get(path).send(&app).await;
        assert_eq!(res.status, StatusCode::OK, "{path}");
        assert_eq!(res.header("cache-control"), Some("no-store"), "{path}");
        assert_eq!(res.text(), SHELL);
    }
    let res = Req::get("/no-such-page/").send(&app).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.header("cache-control"), Some("no-cache"));
    assert_eq!(res.header("content-type"), Some("text/html; charset=utf-8"));
    assert_eq!(res.text(), SHELL);
}

#[sqlx::test(migrations = "./migrations")]
async fn project_pages_get_their_meta(db: PgPool) {
    let build = Build::new();
    let app = app_for(&db, &build).await;
    let nova = testing::published_project(
        &db,
        Committee::Events,
        "nova",
        json!({
            "name": "Nova",
            "summary": "A hackathon for first-timers",
            "media": [{ "src": "/seed/projects/nova.jpg", "alt": "Students presenting" }]
        }),
    )
    .await;
    testing::project(&db, Committee::Tech, "secret-draft").await;
    sqlx::query(
        "INSERT INTO project_slug_redirects (old_slug, project_id) VALUES ('old-nova', $1)",
    )
    .bind(nova)
    .execute(&db)
    .await
    .unwrap();

    let res = Req::get("/projects/nova/").send(&app).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.header("cache-control"), Some("no-cache"));
    let html = res.text();
    assert!(html.contains("<title data-sl-injected>Nova - ScottyLabs</title>"));
    assert!(html.contains(
        "<meta name=\"description\" content=\"A hackathon for first-timers\" data-sl-injected>"
    ));
    assert!(html.contains("og:image\" content=\"http://localhost:5173/seed/projects/nova.jpg\""));
    assert!(html.contains("<div id=\"app\"></div>"));

    let res = Req::get("/projects/nova").send(&app).await;
    assert_eq!(res.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(res.header("location"), Some("/projects/nova/"));

    let res = Req::get("/projects/old-nova/").send(&app).await;
    assert_eq!(res.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(res.header("location"), Some("/projects/nova/"));

    for missing in ["/projects/secret-draft/", "/projects/unknown/"] {
        let res = Req::get(missing).send(&app).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{missing}");
        assert!(!res.text().contains("og:title"), "{missing}");
    }

    // Archived projects disappear too.
    sqlx::query("UPDATE projects SET state = 'archived' WHERE id = $1")
        .bind(nova)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        Req::get("/projects/nova/").send(&app).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn without_a_build_only_the_api_answers(db: PgPool) {
    let app = testing::app(db).await;
    assert_eq!(Req::get("/").send(&app).await.status, StatusCode::NOT_FOUND);
    assert_eq!(
        Req::get("/portal/").send(&app).await.status,
        StatusCode::NOT_FOUND
    );
    let health = Req::get("/api/health").send(&app).await;
    assert_eq!(health.status, StatusCode::OK);
    assert_eq!(health.text(), "ok");
    let missing = Req::get("/api/nope").send(&app).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.code(), "not_found");
    assert_eq!(missing.header("cache-control"), Some("no-store"));
}

#[sqlx::test(migrations = "./migrations")]
async fn every_response_carries_security_headers(db: PgPool) {
    let build = Build::new();
    let app = app_for(&db, &build).await;
    for path in ["/", "/portal/", "/api/health", "/api/me", "/media/x"] {
        let res = Req::get(path).send(&app).await;
        assert_eq!(
            res.header("x-content-type-options"),
            Some("nosniff"),
            "{path}"
        );
        assert_eq!(res.header("x-frame-options"), Some("DENY"), "{path}");
        assert_eq!(
            res.header("referrer-policy"),
            Some("strict-origin-when-cross-origin"),
            "{path}"
        );
        assert_eq!(
            res.header("permissions-policy"),
            Some("camera=(), microphone=(), geolocation=()"),
            "{path}"
        );
        let csp = res.header("content-security-policy").unwrap();
        for directive in [
            "frame-ancestors 'none'",
            "base-uri 'self'",
            "object-src 'none'",
            "form-action 'self'",
        ] {
            assert!(csp.contains(directive), "{path}: {csp}");
        }
        assert!(!csp.contains("script-src"));
        assert!(res.header("strict-transport-security").is_none(), "{path}");
    }

    let mut https = testing::config();
    https.app_url = url::Url::parse("https://scottylabs.org").unwrap();
    https.static_dir = Some(build.0.clone());
    let app = testing::app_with(db, https).await;
    let res = Req::get("/").send(&app).await;
    assert_eq!(
        res.header("strict-transport-security"),
        Some("max-age=31536000")
    );
}

#[test]
fn keycloak_may_receive_sign_in_forms() {
    let mut config = testing::config();
    assert_eq!(
        content_security_policy(&config),
        "frame-ancestors 'none'; base-uri 'self'; object-src 'none'; form-action 'self'"
    );
    config.oidc = Some(crate::config::OidcSettings {
        keycloak_url: "https://idp.scottylabs.org".into(),
        keycloak_realm: "scottylabs".into(),
        client_id: "c".into(),
        client_secret: "s".into(),
        oauth_relay_url: "https://oauth.scottylabs.org/oauth2/callback".into(),
        project_group: "/g".into(),
        project_admin_group: "/g/admins".into(),
        andrew_id_claim: None,
    });
    assert!(
        content_security_policy(&config).ends_with("form-action 'self' https://idp.scottylabs.org")
    );
}
