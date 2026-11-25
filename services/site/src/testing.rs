//! Helpers for integration tests that drive the real router against a throwaway database.
//!
//! ```ignore
//! use crate::models::{Committee, Role};
//! use crate::testing::{self, Req, UserSpec};
//!
//! #[sqlx::test(migrations = "./migrations")]
//! async fn directors_can_publish(db: sqlx::PgPool) {
//!     let app = testing::app(db.clone()).await;                // committees are seeded
//!     let director = testing::user(&db, "evanfost", &[(Role::Director, Some(Committee::Events))]).await;
//!     let nova = testing::project(&db, Committee::Events, "nova").await;
//!     let lead = UserSpec::new("spchoksh").lead(Committee::Events, true).edits(nova).create(&db).await;
//!
//!     // Writes carry the session cookie and the CSRF token automatically.
//!     let res = Req::post("/api/announcements").user(&director).json(serde_json::json!({ "message": "Hi" })).send(&app).await;
//!     assert_eq!(res.status, axum::http::StatusCode::CREATED);
//!     assert_eq!(res.json["announcement"]["message"], "Hi");
//!
//!     // Or the short forms, returning (status, json):
//!     let (status, body) = testing::get(&app, "/api/me", Some(&lead)).await;
//! }
//! ```
//!
//! `Req::cookie(&user)` sends the cookie without the CSRF token, to test that check. Requests have no `Origin` header unless you add one, and no client address, so they share one rate-limit bucket.

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

use crate::auth::csrf::CSRF_HEADER;
use crate::auth::session;
use crate::config::{Config, DevLogin};
use crate::models::{AuthMode, Committee, Role};
use crate::state::AppState;

/// Local-development settings: http on localhost:5173, open dev login, demo content, no S3, no OIDC.
pub fn config() -> Config {
    Config {
        database_url: String::new(),
        app_url: Url::parse("http://localhost:5173").unwrap(),
        bind_addr: "127.0.0.1:0".into(),
        static_dir: None,
        oidc: None,
        s3: None,
        dev_login: DevLogin::Open,
        seed_demo: true,
    }
}

/// The full router with [`config`]. Also makes sure the seven committees exist.
pub async fn app(db: PgPool) -> Router {
    app_with(db, config()).await
}

pub async fn app_with(db: PgPool, config: Config) -> Router {
    ensure_committees(&db).await;
    let state = AppState::new(db, Arc::new(config)).await.unwrap();
    crate::routes::router(state)
}

/// Inserts the seven committees, which role assignments and projects refer to.
pub async fn ensure_committees(db: &PgPool) {
    for (order, (committee, color, tint)) in crate::seed::COMMITTEE_COLORS.iter().enumerate() {
        sqlx::query(
            "INSERT INTO committees (slug, name, sort_order, color, tint) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (slug) DO NOTHING",
        )
        .bind(committee)
        .bind(committee.name())
        .bind(i32::try_from(order).unwrap())
        .bind(*color)
        .bind(*tint)
        .execute(db)
        .await
        .unwrap();
    }
}

/// Seeds the dev-login personas (Thomas, Evan, Anish, Sara, New Member) with their roles.
pub async fn seed_personas(db: &PgPool) {
    ensure_committees(db).await;
    let mut conn = db.acquire().await.unwrap();
    crate::seed::people::run(&mut conn, &config())
        .await
        .unwrap();
}

/// A signed-in person: send `cookie` as the `Cookie` header and `csrf` as `X-CSRF-Token`.
#[derive(Clone, Debug)]
pub struct TestUser {
    pub id: Uuid,
    pub andrew_id: String,
    pub cookie: String,
    pub csrf: String,
}

/// A person with the given roles (leads can post to the ticker) and a session.
pub async fn user(db: &PgPool, andrew_id: &str, roles: &[(Role, Option<Committee>)]) -> TestUser {
    let mut spec = UserSpec::new(andrew_id);
    for (role, committee) in roles {
        spec = spec.role(*role, *committee, true);
    }
    spec.create(db).await
}

/// Builds a person with roles, lead projects and a session.
pub struct UserSpec {
    andrew_id: String,
    name: String,
    roles: Vec<(Role, Option<Committee>, bool)>,
    projects: Vec<Uuid>,
    appointed_by: Option<Uuid>,
}

impl UserSpec {
    pub fn new(andrew_id: &str) -> Self {
        Self {
            andrew_id: andrew_id.into(),
            name: format!("Test {andrew_id}"),
            roles: Vec::new(),
            projects: Vec::new(),
            appointed_by: None,
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.into();
        self
    }

    pub fn role(mut self, role: Role, committee: Option<Committee>, can_post_ticker: bool) -> Self {
        self.roles.push((role, committee, can_post_ticker));
        self
    }

    pub fn admin(self) -> Self {
        self.role(Role::Admin, None, true)
    }

    pub fn director(self, committee: Committee) -> Self {
        self.role(Role::Director, Some(committee), true)
    }

    pub fn lead(self, committee: Committee, can_post_ticker: bool) -> Self {
        self.role(Role::Lead, Some(committee), can_post_ticker)
    }

    /// Lists the person in `project_editors` for a project (see [`project`]).
    pub fn edits(mut self, project: Uuid) -> Self {
        self.projects.push(project);
        self
    }

    pub fn appointed_by(mut self, user_id: Uuid) -> Self {
        self.appointed_by = Some(user_id);
        self
    }

    pub async fn create(self, db: &PgPool) -> TestUser {
        ensure_committees(db).await;
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users (id, andrew_id, name) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(&self.andrew_id)
            .bind(&self.name)
            .execute(db)
            .await
            .unwrap();
        for (role, committee, can_post_ticker) in &self.roles {
            sqlx::query(
                "INSERT INTO role_assignments (id, user_id, role, committee, can_post_ticker, appointed_by, source)
                 VALUES ($1, $2, $3, $4, $5, $6, 'portal')",
            )
            .bind(Uuid::now_v7())
            .bind(id)
            .bind(role)
            .bind(committee)
            .bind(can_post_ticker)
            .bind(self.appointed_by)
            .execute(db)
            .await
            .unwrap();
        }
        for project in &self.projects {
            sqlx::query("INSERT INTO project_editors (project_id, user_id) VALUES ($1, $2)")
                .bind(project)
                .bind(id)
                .execute(db)
                .await
                .unwrap();
        }
        session_for(db, id).await
    }
}

/// A new dev session for an existing user.
pub async fn session_for(db: &PgPool, user_id: Uuid) -> TestUser {
    let mut conn = db.acquire().await.unwrap();
    let created = session::create(&mut conn, user_id, AuthMode::Dev)
        .await
        .unwrap();
    let andrew_id: String = sqlx::query_scalar("SELECT andrew_id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(db)
        .await
        .unwrap();
    TestUser {
        id: user_id,
        andrew_id,
        cookie: format!("sl_session={}", created.token),
        csrf: created.csrf_token,
    }
}

/// Signs in as a seeded persona by Andrew ID (call [`seed_personas`] first).
pub async fn persona(db: &PgPool, andrew_id: &str) -> TestUser {
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE andrew_id = $1")
        .bind(andrew_id)
        .fetch_one(db)
        .await
        .unwrap();
    session_for(db, user_id).await
}

/// Adds a role to an existing person.
pub async fn grant(db: &PgPool, user_id: Uuid, role: Role, committee: Option<Committee>) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role_assignments (id, user_id, role, committee, source) VALUES ($1, $2, $3, $4, 'portal')",
    )
    .bind(id)
    .bind(user_id)
    .bind(role)
    .bind(committee)
    .execute(db)
    .await
    .unwrap();
    id
}

/// A minimal `ProjectDoc` for a project owned by `committee`.
pub fn project_doc(committee: Committee, name: &str) -> Value {
    json!({
        "name": name,
        "summary": "",
        "description": "",
        "committee": committee.slug(),
        "builtWith": [],
        "status": "live",
        "kind": "app",
        "siteUrl": null,
        "repoUrl": null,
        "videoUrl": null,
        "media": []
    })
}

/// A draft project with only the required columns, named after its slug.
pub async fn project(db: &PgPool, committee: Committee, slug: &str) -> Uuid {
    ensure_committees(db).await;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, slug, committee, state, draft) VALUES ($1, $2, $3, 'draft', $4)",
    )
    .bind(id)
    .bind(slug)
    .bind(committee)
    .bind(project_doc(committee, slug))
    .execute(db)
    .await
    .unwrap();
    id
}

/// A published project. `fields` are merged into the published `ProjectDoc`, for example `json!({ "name": "Nova", "summary": "...", "media": [{ "src": "/seed/x.jpg", "alt": "..." }] })`.
pub async fn published_project(
    db: &PgPool,
    committee: Committee,
    slug: &str,
    fields: Value,
) -> Uuid {
    let id = project(db, committee, slug).await;
    let mut doc = project_doc(committee, slug);
    if let (Some(doc), Some(fields)) = (doc.as_object_mut(), fields.as_object()) {
        for (key, value) in fields {
            doc.insert(key.clone(), value.clone());
        }
    }
    sqlx::query(
        "UPDATE projects SET state = 'published', draft = $2, published = $2, published_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(&doc)
    .execute(db)
    .await
    .unwrap();
    id
}

/// A request to send through the router.
pub struct Req {
    method: Method,
    path: String,
    headers: Vec<(String, String)>,
    cookie: Option<String>,
    csrf: Option<String>,
    body: Option<(String, Vec<u8>)>,
}

impl Req {
    pub fn new(method: Method, path: &str) -> Self {
        Self {
            method,
            path: path.into(),
            headers: Vec::new(),
            cookie: None,
            csrf: None,
            body: None,
        }
    }

    pub fn get(path: &str) -> Self {
        Self::new(Method::GET, path)
    }

    pub fn head(path: &str) -> Self {
        Self::new(Method::HEAD, path)
    }

    pub fn post(path: &str) -> Self {
        Self::new(Method::POST, path)
    }

    pub fn put(path: &str) -> Self {
        Self::new(Method::PUT, path)
    }

    pub fn patch(path: &str) -> Self {
        Self::new(Method::PATCH, path)
    }

    pub fn delete(path: &str) -> Self {
        Self::new(Method::DELETE, path)
    }

    /// Signed in as `user`, with the CSRF token on writes.
    pub fn user(mut self, user: &TestUser) -> Self {
        self.cookie = Some(user.cookie.clone());
        self.csrf = Some(user.csrf.clone());
        self
    }

    /// The session cookie only, without the CSRF token.
    pub fn cookie(mut self, user: &TestUser) -> Self {
        self.cookie = Some(user.cookie.clone());
        self.csrf = None;
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn json(mut self, body: Value) -> Self {
        self.body = Some(("application/json".into(), body.to_string().into_bytes()));
        self
    }

    /// A raw body with a content type.
    pub fn body(mut self, content_type: &str, bytes: Vec<u8>) -> Self {
        self.body = Some((content_type.into(), bytes));
        self
    }

    /// `multipart/form-data` with `(name, filename, content type, bytes)` parts. Text fields have no filename.
    pub fn multipart(self, parts: &[(&str, Option<&str>, &str, &[u8])]) -> Self {
        const BOUNDARY: &str = "sl-test-boundary-7d1f";
        let mut body = Vec::new();
        for (name, filename, content_type, bytes) in parts {
            body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
            let disposition = match filename {
                Some(file) => format!("form-data; name=\"{name}\"; filename=\"{file}\""),
                None => format!("form-data; name=\"{name}\""),
            };
            body.extend_from_slice(
                format!(
                    "Content-Disposition: {disposition}\r\nContent-Type: {content_type}\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
        self.body(&format!("multipart/form-data; boundary={BOUNDARY}"), body)
    }

    pub async fn send(self, app: &Router) -> Res {
        let writes = !matches!(self.method, Method::GET | Method::HEAD);
        let mut builder = Request::builder().method(self.method).uri(&self.path);
        if let Some(cookie) = &self.cookie {
            builder = builder.header("cookie", cookie);
        }
        if writes && let Some(csrf) = &self.csrf {
            builder = builder.header(CSRF_HEADER, csrf);
        }
        for (name, value) in &self.headers {
            builder = builder.header(name, value);
        }
        let request = match self.body {
            Some((content_type, bytes)) => builder
                .header("content-type", content_type)
                .body(Body::from(bytes)),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
        Res {
            status,
            headers,
            body,
            json,
        }
    }
}

/// A response with its body read.
pub struct Res {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    /// The body as JSON, or `null` when it is not JSON.
    pub json: Value,
}

impl Res {
    /// `error.code` of an error response, or "".
    pub fn code(&self) -> &str {
        self.json["error"]["code"].as_str().unwrap_or_default()
    }

    pub fn message(&self) -> &str {
        self.json["error"]["message"].as_str().unwrap_or_default()
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// GET, returning the status and JSON body.
pub async fn get(app: &Router, path: &str, user: Option<&TestUser>) -> (StatusCode, Value) {
    finish(with_user(Req::get(path), user).send(app).await)
}

pub async fn post(
    app: &Router,
    path: &str,
    user: Option<&TestUser>,
    body: Value,
) -> (StatusCode, Value) {
    finish(with_user(Req::post(path), user).json(body).send(app).await)
}

pub async fn put(
    app: &Router,
    path: &str,
    user: Option<&TestUser>,
    body: Value,
) -> (StatusCode, Value) {
    finish(with_user(Req::put(path), user).json(body).send(app).await)
}

pub async fn patch(
    app: &Router,
    path: &str,
    user: Option<&TestUser>,
    body: Value,
) -> (StatusCode, Value) {
    finish(with_user(Req::patch(path), user).json(body).send(app).await)
}

pub async fn delete(app: &Router, path: &str, user: Option<&TestUser>) -> (StatusCode, Value) {
    finish(with_user(Req::delete(path), user).send(app).await)
}

fn with_user(req: Req, user: Option<&TestUser>) -> Req {
    match user {
        Some(user) => req.user(user),
        None => req,
    }
}

fn finish(res: Res) -> (StatusCode, Value) {
    (res.status, res.json)
}

/// The audit summaries recorded so far, oldest first.
pub async fn audit_summaries(db: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT summary FROM audit_log ORDER BY created_at, id")
        .fetch_all(db)
        .await
        .unwrap()
}

/// A migrated database that belongs to one test run, dropped when this goes out of scope. Use it with `#[tokio::test]` in place of `#[sqlx::test]` for a test that spends seconds without the database, such as one that encodes or decodes images.
///
/// `#[sqlx::test]` names a test's database after the test alone, and the local Postgres server is shared. When the same test runs in two processes at once (two worktrees, or a commit hook beside a manual run), the second drops and recreates the database the first is using, which succeeds whenever the first holds no connection for a second, as while it works on an image. A name of its own for every run rules that out.
///
/// A killed run never drops its database, and neither does a failed test in the dev build, whose Cranelift backend does not unwind, so a panic skips every drop. Each new `IsolatedDb` therefore first drops the test databases more than an hour old.
pub struct IsolatedDb {
    pub pool: PgPool,
    // Dropped after the pool; the drop ends any connection still open.
    database: TestDatabase,
}

impl IsolatedDb {
    pub async fn new() -> Self {
        let mut server = PgConnection::connect_with(&server_options()).await.unwrap();
        drop_stale_databases(&mut server).await;
        let database = TestDatabase::create(&mut server).await;
        server.close().await.unwrap();
        // Where panics unwind, a failure from here on, such as a failed migration, drops `database` and so the database; elsewhere the next run's clean-up does.
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(server_options().database(&database.0))
            .await
            .unwrap();
        crate::db::migrate(&pool).await.unwrap();
        Self { pool, database }
    }
}

const TEST_DATABASE_PREFIX: &str = "_sl_test_";

/// No test holds a database this long, so an older one was left by a run that was killed or failed.
const STALE_TEST_DATABASE: Duration = Duration::from_secs(60 * 60);

/// A database created for one test run, named for the time it was made. Dropping this drops it.
struct TestDatabase(String);

impl TestDatabase {
    async fn create(server: &mut PgConnection) -> Self {
        let name = test_database_name(Uuid::now_v7());
        server
            .execute(format!("CREATE DATABASE \"{name}\"").as_str())
            .await
            .unwrap();
        Self(name)
    }
}

impl Drop for TestDatabase {
    /// Drops the database from a thread of its own, since this may run inside the test's runtime.
    fn drop(&mut self) {
        let name = std::mem::take(&mut self.0);
        let dropped = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let mut server = PgConnection::connect_with(&server_options()).await?;
                server.execute(drop_database(&name).as_str()).await?;
                server.close().await
            })?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        })
        .join();
        if !matches!(dropped, Ok(Ok(()))) {
            eprintln!("could not drop a test database: {dropped:?}");
        }
    }
}

fn test_database_name(id: Uuid) -> String {
    format!("{TEST_DATABASE_PREFIX}{}", id.simple())
}

fn drop_database(name: &str) -> String {
    format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)")
}

/// When a test database was made, for names this module gives out and nothing else.
fn test_database_created(name: &str) -> Option<SystemTime> {
    let id = Uuid::try_parse(name.strip_prefix(TEST_DATABASE_PREFIX)?).ok()?;
    if test_database_name(id) != name {
        return None;
    }
    let (seconds, nanos) = id.get_timestamp()?.to_unix();
    Some(UNIX_EPOCH + Duration::new(seconds, nanos))
}

fn is_stale_test_database(name: &str, now: SystemTime) -> bool {
    test_database_created(name)
        .and_then(|created| now.duration_since(created).ok())
        .is_some_and(|age| age > STALE_TEST_DATABASE)
}

/// Drops the test databases that killed runs left behind. Best effort: a failure here never fails a test.
async fn drop_stale_databases(server: &mut PgConnection) {
    let names: Vec<String> =
        match sqlx::query_scalar("SELECT datname FROM pg_database WHERE starts_with(datname, $1)")
            .bind(TEST_DATABASE_PREFIX)
            .fetch_all(&mut *server)
            .await
        {
            Ok(names) => names,
            Err(error) => {
                eprintln!("could not list test databases: {error}");
                return;
            }
        };
    let now = SystemTime::now();
    for name in names
        .iter()
        .filter(|name| is_stale_test_database(name, now))
    {
        if let Err(error) = server.execute(drop_database(name).as_str()).await {
            eprintln!("could not drop the stale test database {name}: {error}");
        }
    }
}

fn server_options() -> PgConnectOptions {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnectOptions::from_str(&url).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test database name as a run would have made it `ago` before `now`.
    fn made_before(now: SystemTime, ago: Duration) -> String {
        let since = (now - ago).duration_since(UNIX_EPOCH).unwrap();
        let at = uuid::Timestamp::from_unix(uuid::NoContext, since.as_secs(), since.subsec_nanos());
        test_database_name(Uuid::new_v7(at))
    }

    async fn database_exists(server: &mut PgConnection, name: &str) -> bool {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(name)
            .fetch_one(server)
            .await
            .unwrap()
    }

    #[test]
    fn only_test_databases_left_for_an_hour_are_stale() {
        let now = SystemTime::now();
        let two_hours = Duration::from_secs(2 * 60 * 60);
        assert!(is_stale_test_database(&made_before(now, two_hours), now));
        assert!(!is_stale_test_database(
            &made_before(now, Duration::from_secs(60)),
            now
        ));

        // Names this module does not give out are never dropped.
        let old = made_before(now, two_hours);
        let hyphenated = format!(
            "{TEST_DATABASE_PREFIX}{}",
            Uuid::try_parse(old.strip_prefix(TEST_DATABASE_PREFIX).unwrap())
                .unwrap()
                .hyphenated()
        );
        let without_time = test_database_name(Uuid::new_v4());
        for name in [
            "sl_stage3",
            "_sl_test_",
            "_sl_test_old",
            &hyphenated,
            &without_time,
        ] {
            assert!(!is_stale_test_database(name, now), "{name}");
        }
    }

    #[tokio::test]
    async fn isolated_databases_go_when_dropped_and_clear_ones_killed_runs_left() {
        let mut server = PgConnection::connect_with(&server_options()).await.unwrap();
        let left = made_before(SystemTime::now(), Duration::from_secs(2 * 60 * 60));
        server
            .execute(format!("CREATE DATABASE \"{left}\"").as_str())
            .await
            .unwrap();

        let isolated = IsolatedDb::new().await;
        let name = isolated.database.0.clone();
        assert!(database_exists(&mut server, &name).await);
        assert!(!database_exists(&mut server, &left).await);

        drop(isolated);
        assert!(!database_exists(&mut server, &name).await);
        server.close().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn helpers_drive_the_real_router(db: PgPool) {
        seed_personas(&db).await;
        let app = app(db.clone()).await;
        let thomas = persona(&db, "tkanz").await;
        assert_eq!(thomas.andrew_id, "tkanz");

        let (status, me) = get(&app, "/api/me", Some(&thomas)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["user"]["andrewId"], "tkanz");
        assert_eq!(me["csrfToken"], thomas.csrf.as_str());

        let lead = json!({
            "andrewId": "helperlead", "name": "Helper Lead", "role": "lead",
            "committee": "tech", "canPostTicker": true, "projectIds": []
        });
        let (status, row) = post(&app, "/api/people", Some(&thomas), lead).await;
        assert_eq!(status, StatusCode::CREATED);
        let path = format!(
            "/api/people/assignments/{}",
            row["roles"][0]["id"].as_str().unwrap()
        );
        let (status, row) = patch(
            &app,
            &path,
            Some(&thomas),
            json!({ "canPostTicker": false }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(row["roles"][0]["canPostTicker"], false);
        let (status, _) = delete(&app, &path, Some(&thomas)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, body) = put(&app, "/api/no-such-endpoint", Some(&thomas), json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        let res = Req::put("/api/no-such-endpoint")
            .user(&thomas)
            .json(json!({}))
            .send(&app)
            .await;
        assert_eq!(res.code(), "not_found");

        // Without the cookie there is no viewer.
        let (status, _) = get(&app, "/api/me", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(audit_summaries(&db).await.len(), 3);
    }
}
