//! The site through database outages: the static pages without a database, the API's 503s, the health check, and the last good copies of public answers.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::StatusCode;
use sqlx::PgPool;

use crate::config::Config;
use crate::db::{self, Phase};
use crate::state::AppState;
use crate::testing::{self, Req};

/// A socket directory that does not exist: the outage's "No such file or directory".
const NOWHERE: &str = "postgresql:///nowhere?host=/nonexistent/sl-site-test";

/// A tiny build with a home page and the SPA shell, removed when dropped.
struct Build(PathBuf);

impl Build {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("site-outage-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("index.html"),
            "<!doctype html><title>Home</title>",
        )
        .unwrap();
        std::fs::write(root.join("200.html"), "<!doctype html><title>Shell</title>").unwrap();
        Self(root)
    }
}

impl Drop for Build {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The router as `main` builds it, over a database that is not there.
async fn without_a_database(build: &Build) -> (Router, AppState) {
    let config = Config {
        static_dir: Some(build.0.clone()),
        ..testing::config()
    };
    let pool = db::request_pool(db::connect_options(NOWHERE).unwrap(), 2);
    let state = AppState::starting(pool, Arc::new(config)).await.unwrap();
    (super::router(state.clone()), state)
}

#[tokio::test]
async fn pages_are_served_and_the_api_answers_503_without_a_database() {
    let build = Build::new();
    let (app, state) = without_a_database(&build).await;

    let home = Req::get("/").send(&app).await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(home.text().contains("Home"));

    let health = Req::get("/api/health").send(&app).await;
    assert_eq!(health.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(health.text(), "starting");
    assert_eq!(health.header("retry-after"), Some("5"));

    for path in ["/api/public/ticker", "/api/me", "/api/public/projects"] {
        let res = Req::get(path).send(&app).await;
        assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(res.code(), "unavailable", "{path}");
        assert_eq!(res.header("retry-after"), Some("5"), "{path}");
        assert_eq!(res.header("cache-control"), Some("no-store"), "{path}");
    }

    state.database.set(Phase::Unavailable);
    let health = Req::get("/api/health").send(&app).await;
    assert_eq!(health.text(), "unavailable");

    // Ready once, then the database went away: requests fail fast with 503, not 500.
    state.database.set(Phase::Ready);
    let started = Instant::now();
    let ticker = Req::get("/api/public/ticker").send(&app).await;
    assert_eq!(ticker.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(ticker.code(), "unavailable");
    assert!(started.elapsed() < Duration::from_secs(4));
    assert_eq!(
        Req::get("/api/health").send(&app).await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    // A project page still gets its shell, so the app can show what it has.
    let project = Req::get("/projects/nova/").send(&app).await;
    assert_eq!(project.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(project.text().contains("Shell"));
}

#[sqlx::test(migrations = "./migrations")]
async fn public_answers_survive_a_database_outage(db: PgPool) {
    testing::ensure_committees(&db).await;
    crate::seed::run(&db, &testing::config()).await.unwrap();
    // A pool of its own, so closing it plays the outage without touching the test's.
    let pool = db::request_pool((*db.connect_options()).clone(), 2);
    let state = AppState::new(pool.clone(), Arc::new(testing::config()))
        .await
        .unwrap();
    let app = super::router(state);

    let paths = [
        "/api/public/ticker",
        "/api/public/committees",
        "/api/public/committees/tech",
        "/api/public/projects",
        "/api/public/projects?committee=tech",
        "/api/public/projects/cmu-maps",
    ];
    let mut fresh = Vec::new();
    for path in paths {
        let res = Req::get(path).send(&app).await;
        assert_eq!(res.status, StatusCode::OK, "{path}");
        assert_eq!(res.header("x-content-stale"), None, "{path}");
        assert_eq!(
            res.header("cache-control"),
            Some("public, max-age=60, stale-while-revalidate=600, stale-if-error=86400"),
            "{path}"
        );
        fresh.push(res.body);
    }

    pool.close().await;
    for (path, body) in paths.iter().zip(&fresh) {
        let res = Req::get(path).send(&app).await;
        assert_eq!(res.status, StatusCode::OK, "{path}");
        assert_eq!(res.header("x-content-stale"), Some("true"), "{path}");
        assert_eq!(res.header("cache-control"), Some("no-store"), "{path}");
        assert_eq!(
            res.header("access-control-allow-origin"),
            Some("*"),
            "{path}"
        );
        assert_eq!(&res.body, body, "{path}");
    }

    // Searches and answers never loaded have no copy, and say the database is unavailable.
    for path in [
        "/api/public/projects?q=maps",
        "/api/public/committees/design",
    ] {
        let res = Req::get(path).send(&app).await;
        assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(res.code(), "unavailable", "{path}");
    }
}
