//! Session cookies and rows. The cookie holds 32 random bytes; the database keeps only their SHA-256, so a database leak does not hand out sessions.

use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use cookie::time::Duration as CookieDuration;
use cookie::{Cookie, SameSite};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgExecutor};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppResult;
use crate::models::AuthMode;

/// A session ends after this long without a request.
pub const IDLE_TIMEOUT_HOURS: i64 = 12;
/// A session ends this long after sign-in, however active it is.
pub const MAX_AGE_DAYS: i64 = 7;
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

/// A session that was just created. `token` goes in the cookie and is never stored.
pub struct NewSession {
    pub id: Uuid,
    pub token: String,
    pub csrf_token: String,
}

/// Starts a session for a user and records that they signed in.
pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    mode: AuthMode,
) -> AppResult<NewSession> {
    purge_expired(&mut *conn).await?;
    let session = NewSession {
        id: Uuid::now_v7(),
        token: random_token(),
        csrf_token: random_token(),
    };
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, csrf_token, auth_mode, expires_at)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(days => $6))",
    )
    .bind(session.id)
    .bind(hash_token(&session.token))
    .bind(user_id)
    .bind(&session.csrf_token)
    .bind(mode)
    .bind(i32::try_from(MAX_AGE_DAYS).unwrap_or(7))
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

pub fn session_cookie(config: &Config, token: &str) -> String {
    build_cookie(
        config,
        cookie_name(config),
        token,
        CookieDuration::days(MAX_AGE_DAYS),
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
    fn secrets_compare_exactly() {
        assert!(secrets_match("abc", "abc"));
        assert!(!secrets_match("abc", "abd"));
        assert!(!secrets_match("abc", "abcd"));
        assert!(!secrets_match("", "abc"));
    }

    #[test]
    fn cookie_attributes_follow_the_scheme() {
        let mut config = testing::config();
        let plain = session_cookie(&config, "tok");
        assert!(plain.starts_with("sl_session=tok"));
        assert!(plain.contains("HttpOnly"));
        assert!(plain.contains("SameSite=Lax"));
        assert!(plain.contains("Path=/"));
        assert!(!plain.contains("Secure"));

        config.app_url = url::Url::parse("https://scottylabs.org").unwrap();
        let secure = session_cookie(&config, "tok");
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
