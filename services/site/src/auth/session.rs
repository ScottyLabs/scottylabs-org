//! Session cookies and rows. The cookie holds 32 random bytes; the database keeps only their SHA-256, and the CSRF token is derived from the cookie rather than stored, so a database leak hands out neither sessions nor CSRF tokens.

use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use cookie::time::Duration as CookieDuration;
use cookie::{Cookie, SameSite};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection, PgExecutor};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppResult;
use crate::models::AuthMode;

/// A session ends after this long without a request.
pub const IDLE_TIMEOUT_HOURS: i64 = 12;
/// A session ends this long after sign-in, however active it is.
pub const MAX_AGE_DAYS: i64 = 7;
/// Dev sign-in sessions end sooner: they share one key on previews.
pub const DEV_MAX_AGE_HOURS: i64 = 8;
/// `last_seen_at` is written at most this often, so reads stay reads.
pub const TOUCH_INTERVAL_SECONDS: i64 = 60;

const SECURE_COOKIE: &str = "__Host-sl_session";
const PLAIN_COOKIE: &str = "sl_session";

/// The session cookie's name: `__Host-` prefixed when the site is served over https.
pub fn cookie_name(config: &Config) -> &'static str {
    if config.is_https() {
        SECURE_COOKIE
    } else {
        PLAIN_COOKIE
    }
}

/// 32 bytes from the thread-local CSPRNG, base64url without padding.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Compares two secrets without leaking where they differ or how long they are.
pub fn secrets_match(given: &str, expected: &str) -> bool {
    let given = Sha256::digest(given.as_bytes());
    let expected = Sha256::digest(expected.as_bytes());
    given.ct_eq(&expected).into()
}

/// The CSRF token for a session: base64url of SHA-256 over a fixed prefix and the session token. Only someone holding the cookie can compute it, and nothing usable is stored.
pub fn csrf_token_for(session_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"scottylabs-site csrf v1\0");
    hasher.update(session_token.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// What a dev session stores about the preview key it was made with, so that changing the key ends it.
pub fn dev_key_fingerprint(key: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"scottylabs-site dev key v1\0");
    hasher.update(key.as_bytes());
    hasher.finalize().to_vec()
}

/// How long a session of this kind lasts at most.
pub fn lifetime(mode: AuthMode) -> chrono::Duration {
    match mode {
        AuthMode::Oidc => chrono::Duration::days(MAX_AGE_DAYS),
        AuthMode::Dev => chrono::Duration::hours(DEV_MAX_AGE_HOURS),
    }
}

/// Compares two byte strings in constant time.
pub fn secrets_match_bytes(given: &[u8], expected: &[u8]) -> bool {
    given.ct_eq(expected).into()
}

/// A session that was just created. `token` goes in the cookie and is never stored.
pub struct NewSession {
    pub id: Uuid,
    pub token: String,
    /// What `X-CSRF-Token` must carry. Browsers read it from `GET /api/me`; tests take it from here.
    #[cfg_attr(not(test), allow(dead_code))]
    pub csrf_token: String,
    pub mode: AuthMode,
}

/// What a session keeps besides who it belongs to.
#[derive(Default)]
pub struct SessionExtras {
    /// Keycloak's refresh token, to check the session with Keycloak later. Server side only.
    pub refresh_token: Option<String>,
    /// The latest ID token, sent back to Keycloak as `id_token_hint` when signing out of it.
    pub id_token: Option<String>,
    pub dev_key_fingerprint: Option<Vec<u8>>,
}

/// Starts a session for a user and records that they signed in. The routes use [`create_with`].
#[cfg(test)]
pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    mode: AuthMode,
) -> AppResult<NewSession> {
    create_with(conn, user_id, mode, SessionExtras::default()).await
}

/// [`create`], keeping Keycloak's tokens or the dev key's fingerprint with the session. A Keycloak session counts as confirmed by Keycloak now.
pub async fn create_with(
    conn: &mut PgConnection,
    user_id: Uuid,
    mode: AuthMode,
    extras: SessionExtras,
) -> AppResult<NewSession> {
    purge_expired(&mut *conn).await?;
    let token = random_token();
    let session = NewSession {
        id: Uuid::now_v7(),
        csrf_token: csrf_token_for(&token),
        token,
        mode,
    };
    // The column is required by the schema. A random value no request can produce keeps a rollback to a release that still reads it working, while this release never accepts it.
    let unused_csrf = random_token();
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, csrf_token, auth_mode, expires_at,
                               refresh_token, id_token, validated_at, dev_key_fingerprint)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(secs => $6),
                 $7, $8, CASE WHEN $5 = 'oidc' THEN now() END, $9)",
    )
    .bind(session.id)
    .bind(hash_token(&session.token))
    .bind(user_id)
    .bind(&unused_csrf)
    .bind(mode)
    .bind(lifetime(mode).num_seconds() as f64)
    .bind(&extras.refresh_token)
    .bind(&extras.id_token)
    .bind(&extras.dev_key_fingerprint)
    .execute(&mut *conn)
    .await?;
    sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(session)
}

/// Whether `last_seen_at` is old enough to be worth writing again.
pub fn needs_touch(last_seen_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(last_seen_at).num_seconds() >= TOUCH_INTERVAL_SECONDS
}

/// Marks the session and its user as seen now.
pub async fn touch<'e>(db: impl PgExecutor<'e>, session_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "WITH seen AS (
             UPDATE sessions SET last_seen_at = now() WHERE id = $1 RETURNING user_id
         )
         UPDATE users SET last_seen_at = now() WHERE id IN (SELECT user_id FROM seen)",
    )
    .bind(session_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_by_token<'e>(db: impl PgExecutor<'e>, token: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(hash_token(token))
        .execute(db)
        .await?;
    Ok(())
}

/// A session that was just ended by its cookie.
#[derive(FromRow)]
pub struct EndedSession {
    pub user_id: Uuid,
    pub auth_mode: AuthMode,
    pub id_token: Option<String>,
    pub name: String,
    pub andrew_id: String,
}

/// Ends the session behind a cookie and says whose it was, if it existed.
pub async fn end_by_token<'e>(
    db: impl PgExecutor<'e>,
    token: &str,
) -> AppResult<Option<EndedSession>> {
    let ended = sqlx::query_as::<_, EndedSession>(
        "WITH gone AS (
             DELETE FROM sessions WHERE token_hash = $1 RETURNING user_id, auth_mode, id_token
         )
         SELECT g.user_id, g.auth_mode, g.id_token, u.name, u.andrew_id
         FROM gone g JOIN users u ON u.id = g.user_id",
    )
    .bind(hash_token(token))
    .fetch_optional(db)
    .await?;
    Ok(ended)
}

/// Ends every session of a person, in every browser. Returns how many there were.
pub async fn end_all_for_user<'e>(db: impl PgExecutor<'e>, user_id: Uuid) -> AppResult<u64> {
    let done = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(done.rows_affected())
}

/// Ends every session of a person who no longer has any role, so a copied cookie or a session that would get an admin role back from the Keycloak group stops working too. Returns how many ended.
pub async fn end_if_roleless<'e>(db: impl PgExecutor<'e>, user_id: Uuid) -> AppResult<u64> {
    let done = sqlx::query(
        "DELETE FROM sessions WHERE user_id = $1
         AND NOT EXISTS (SELECT 1 FROM role_assignments WHERE user_id = $1)",
    )
    .bind(user_id)
    .execute(db)
    .await?;
    if done.rows_affected() > 0 {
        tracing::info!(
            user = %user_id,
            ended = done.rows_affected(),
            "ended the sessions of someone left with no role"
        );
    }
    Ok(done.rows_affected())
}

/// Removes sessions and sign-in attempts that can no longer be used. Runs at each sign-in.
pub async fn purge_expired(conn: &mut PgConnection) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM sessions
         WHERE expires_at <= now() OR last_seen_at <= now() - make_interval(hours => $1)",
    )
    .bind(i32::try_from(IDLE_TIMEOUT_HOURS).unwrap_or(12))
    .execute(&mut *conn)
    .await?;
    sqlx::query("DELETE FROM auth_attempts WHERE created_at <= now() - interval '10 minutes'")
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Reads a cookie by name from every `Cookie` header on the request.
pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(Cookie::split_parse)
        .filter_map(Result::ok)
        .find(|cookie| cookie.name() == name && !cookie.value().is_empty())
        .map(|cookie| cookie.value().to_owned())
}

/// The session token sent by the browser, if any.
pub fn token_from_headers(headers: &HeaderMap, config: &Config) -> Option<String> {
    read_cookie(headers, cookie_name(config))
}

/// Builds a `Set-Cookie` value with the attributes every site cookie shares.
pub fn build_cookie(config: &Config, name: &str, value: &str, max_age: CookieDuration) -> String {
    Cookie::build((name.to_owned(), value.to_owned()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .secure(config.is_https())
        .max_age(max_age)
        .build()
        .to_string()
}

/// The cookie for a new session, expiring with it.
pub fn session_cookie(config: &Config, session: &NewSession) -> String {
    build_cookie(
        config,
        cookie_name(config),
        &session.token,
        CookieDuration::seconds(lifetime(session.mode).num_seconds()),
    )
}

pub fn clear_session_cookie(config: &Config) -> String {
    build_cookie(config, cookie_name(config), "", CookieDuration::ZERO)
}

/// Appends a `Set-Cookie` header. Values built here are always valid header values.
pub fn append_cookie(headers: &mut HeaderMap, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        headers.append(SET_COOKIE, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    #[test]
    fn tokens_are_long_random_and_url_safe() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_eq!(hash_token(&a).len(), 32);
    }

    #[test]
    fn csrf_tokens_come_from_the_session_token() {
        let a = random_token();
        let b = random_token();
        assert_eq!(csrf_token_for(&a), csrf_token_for(&a));
        assert_ne!(csrf_token_for(&a), csrf_token_for(&b));
        // Not the token itself, nor its stored hash.
        assert_ne!(csrf_token_for(&a), a);
        assert_ne!(csrf_token_for(&a), URL_SAFE_NO_PAD.encode(hash_token(&a)));
        assert_eq!(csrf_token_for(&a).len(), 43);
    }

    #[test]
    fn dev_key_fingerprints_differ_by_key() {
        assert_eq!(dev_key_fingerprint("abc"), dev_key_fingerprint("abc"));
        assert_ne!(dev_key_fingerprint("abc"), dev_key_fingerprint("abd"));
        assert_ne!(dev_key_fingerprint("abc"), Sha256::digest(b"abc").to_vec());
    }

    #[test]
    fn dev_sessions_are_shorter() {
        assert_eq!(lifetime(AuthMode::Oidc), chrono::Duration::days(7));
        assert_eq!(lifetime(AuthMode::Dev), chrono::Duration::hours(8));
    }

    #[test]
    fn secrets_compare_exactly() {
        assert!(secrets_match("abc", "abc"));
        assert!(!secrets_match("abc", "abd"));
        assert!(!secrets_match("abc", "abcd"));
        assert!(!secrets_match("", "abc"));
    }

    fn new_session(mode: AuthMode) -> NewSession {
        NewSession {
            id: Uuid::now_v7(),
            token: "tok".into(),
            csrf_token: csrf_token_for("tok"),
            mode,
        }
    }

    #[test]
    fn cookie_attributes_follow_the_scheme() {
        let mut config = testing::config();
        let plain = session_cookie(&config, &new_session(AuthMode::Oidc));
        assert!(plain.starts_with("sl_session=tok"));
        assert!(plain.contains("HttpOnly"));
        assert!(plain.contains("SameSite=Lax"));
        assert!(plain.contains("Path=/"));
        assert!(plain.contains("Max-Age=604800"));
        assert!(!plain.contains("Secure"));
        let dev = session_cookie(&config, &new_session(AuthMode::Dev));
        assert!(dev.contains("Max-Age=28800"));

        config.app_url = url::Url::parse("https://scottylabs.org").unwrap();
        let secure = session_cookie(&config, &new_session(AuthMode::Oidc));
        assert!(secure.starts_with("__Host-sl_session=tok"));
        assert!(secure.contains("Secure"));
        assert!(clear_session_cookie(&config).contains("Max-Age=0"));
    }

    #[test]
    fn reads_cookies_from_any_header() {
        let config = testing::config();
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_static("a=1; theme=dark"));
        headers.append(COOKIE, HeaderValue::from_static("sl_session=xyz"));
        assert_eq!(
            token_from_headers(&headers, &config).as_deref(),
            Some("xyz")
        );
        assert_eq!(read_cookie(&headers, "theme").as_deref(), Some("dark"));
        assert_eq!(read_cookie(&headers, "missing"), None);
    }

    #[test]
    fn touches_at_most_once_a_minute() {
        let now = Utc::now();
        assert!(!needs_touch(now - chrono::Duration::seconds(30), now));
        assert!(needs_touch(now - chrono::Duration::seconds(61), now));
    }
}
