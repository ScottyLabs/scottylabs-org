//! Per-IP request limits, kept in memory: starting a sign-in 60 a minute (a campus network often shares one address), dev sign-in 20, uploads 30, other writes 120. Reads are not limited. A refused `GET /api/auth/login`, which is a page navigation, goes back to the sign-in page with `error=rate_limited` instead of a JSON error.
//!
//! Each (class, client) pair has a token bucket that refills evenly over a minute. Idle buckets are pruned once a minute and the table has a hard size cap, so memory stays bounded however many addresses show up.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{CACHE_CONTROL, LOCATION, RETRY_AFTER};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::auth::csrf::{full_path, is_write};
use crate::error::AppError;
use crate::state::AppState;

/// Buckets kept at most. Past this, after pruning, the table starts over rather than grow.
const MAX_BUCKETS: usize = 50_000;
const PRUNE_EVERY: Duration = Duration::from_secs(60);

/// Where a refused sign-in start goes.
const RATE_LIMITED_SIGN_IN: &str = "/portal/sign-in/?error=rate_limited";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// Starting a Keycloak sign-in.
    SignIn,
    /// Dev sign-in and its persona list, which take the preview key.
    DevLogin,
    Write,
    Upload,
}

impl Class {
    pub fn per_minute(self) -> u32 {
        match self {
            Self::SignIn => 60,
            Self::DevLogin => 20,
            Self::Write => 120,
            Self::Upload => 30,
        }
    }

    fn refill_per_second(self) -> f64 {
        f64::from(self.per_minute()) / 60.0
    }
}

/// Which limit a request counts against, if any. `path` is the full path including `/api`.
pub fn classify(method: &Method, path: &str) -> Option<Class> {
    match (method, path) {
        (&Method::GET, "/api/auth/login") => Some(Class::SignIn),
        (&Method::POST, "/api/auth/dev-login" | "/api/auth/dev-personas") => Some(Class::DevLogin),
        (&Method::POST, "/api/media") => Some(Class::Upload),
        _ if is_write(method) => Some(Class::Write),
        _ => None,
    }
}

struct Bucket {
    tokens: f64,
    updated: Instant,
}

struct Table {
    buckets: HashMap<(Class, IpAddr), Bucket>,
    pruned_at: Instant,
}

pub struct RateLimiter {
    table: Mutex<Table>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            table: Mutex::new(Table {
                buckets: HashMap::new(),
                pruned_at: Instant::now(),
            }),
        }
    }

    /// Takes one request from the client's bucket. On refusal, returns how long until the next one is allowed.
    pub fn check(&self, class: Class, client: IpAddr, now: Instant) -> Result<(), Duration> {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        if now.saturating_duration_since(table.pruned_at) >= PRUNE_EVERY
            || table.buckets.len() >= MAX_BUCKETS
        {
            prune(&mut table, now);
        }
        let capacity = f64::from(class.per_minute());
        let rate = class.refill_per_second();
        let bucket = table.buckets.entry((class, client)).or_insert(Bucket {
            tokens: capacity,
            updated: now,
        });
        let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * rate).min(capacity);
        bucket.updated = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err(Duration::from_secs_f64((1.0 - bucket.tokens) / rate))
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .buckets
            .len()
    }
}

/// Drops buckets that have refilled completely, since a fresh bucket behaves the same.
fn prune(table: &mut Table, now: Instant) {
    table.buckets.retain(|(class, _), bucket| {
        let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
        bucket.tokens + elapsed * class.refill_per_second() < f64::from(class.per_minute())
    });
    if table.buckets.len() >= MAX_BUCKETS {
        tracing::warn!(
            buckets = table.buckets.len(),
            "rate limit table is full; starting over"
        );
        table.buckets.clear();
    }
    table.pruned_at = now;
}

/// The address limits apply to. Kennel serves through Cloudflare, then a tunnel to the host, then Caddy on the same host, so a loopback peer is the proxy. Behind it the visitor is `CF-Connecting-IP`, which Cloudflare's edge sets and the tunnel and Caddy pass on; Caddy replaces `X-Forwarded-For` with the tunnel's loopback address, so its right-most entry is only the fallback.
///
/// Those headers count only when `trust_proxy` is set, which the service does when it listens on a loopback address only (`Config::binds_loopback`): then every connection comes from this host. Listening on every interface, a loopback peer is not known to be the proxy, so it is the client itself, like any other peer, whose headers are never trusted.
pub fn client_ip(peer: Option<IpAddr>, headers: &HeaderMap, trust_proxy: bool) -> IpAddr {
    let ip = match peer {
        Some(peer) if trust_proxy && peer.is_loopback() => connecting_ip(headers)
            .or_else(|| forwarded_for(headers))
            .unwrap_or(peer),
        Some(peer) => peer,
        None => IpAddr::from([0, 0, 0, 0]),
    };
    normalize(ip)
}

fn connecting_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("cf-connecting-ip")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    let last_header = headers.get_all("x-forwarded-for").iter().next_back()?;
    let last_entry = last_header.to_str().ok()?.rsplit(',').next()?.trim();
    last_entry.parse().ok()
}

/// One IPv6 holder usually controls a whole /64, so the /64 is the client.
fn normalize(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                let segments = v6.segments();
                IpAddr::V6(Ipv6Addr::new(
                    segments[0],
                    segments[1],
                    segments[2],
                    segments[3],
                    0,
                    0,
                    0,
                    0,
                ))
            }
        },
    }
}

pub async fn limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(class) = classify(request.method(), full_path(&request)) else {
        return next.run(request).await;
    };
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let client = client_ip(peer, request.headers(), state.config.binds_loopback());
    match state.limits.check(class, client, Instant::now()) {
        Ok(()) => next.run(request).await,
        Err(wait) => {
            let mut response = if class == Class::SignIn {
                sign_in_page_redirect()
            } else {
                AppError::RateLimited.into_response()
            };
            let seconds = wait.as_secs().max(1).to_string();
            if let Ok(value) = HeaderValue::from_str(&seconds) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
            response
        }
    }
}

/// People reach `GET /api/auth/login` by following a link, so a refusal sends them back to the sign-in page, which explains it.
fn sign_in_page_redirect() -> Response {
    let mut response = StatusCode::FOUND.into_response();
    let headers = response.headers_mut();
    headers.insert(LOCATION, HeaderValue::from_static(RATE_LIMITED_SIGN_IN));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifies_requests() {
        assert_eq!(
            classify(&Method::GET, "/api/auth/login"),
            Some(Class::SignIn)
        );
        assert_eq!(
            classify(&Method::POST, "/api/auth/dev-login"),
            Some(Class::DevLogin)
        );
        assert_eq!(
            classify(&Method::POST, "/api/auth/dev-personas"),
            Some(Class::DevLogin)
        );
        assert_eq!(classify(&Method::GET, "/api/auth/callback"), None);
        assert_eq!(classify(&Method::POST, "/api/media"), Some(Class::Upload));
        assert_eq!(
            classify(&Method::PATCH, "/api/people/assignments/x"),
            Some(Class::Write)
        );
        assert_eq!(
            classify(&Method::DELETE, "/api/announcements/x"),
            Some(Class::Write)
        );
        assert_eq!(classify(&Method::GET, "/api/me"), None);
        assert_eq!(classify(&Method::GET, "/api/public/ticker"), None);
    }

    #[test]
    fn buckets_empty_then_refill() {
        let limiter = RateLimiter::new();
        let client = ip("203.0.113.9");
        let start = Instant::now();
        for _ in 0..20 {
            assert!(limiter.check(Class::DevLogin, client, start).is_ok());
        }
        let wait = limiter.check(Class::DevLogin, client, start).unwrap_err();
        assert!(wait <= Duration::from_secs(3), "{wait:?}");

        // Other clients and other classes are separate.
        assert!(
            limiter
                .check(Class::DevLogin, ip("203.0.113.10"), start)
                .is_ok()
        );
        assert!(limiter.check(Class::Write, client, start).is_ok());
        assert!(limiter.check(Class::SignIn, client, start).is_ok());

        // One request comes back every three seconds.
        assert!(
            limiter
                .check(Class::DevLogin, client, start + Duration::from_secs(3))
                .is_ok()
        );
        assert!(
            limiter
                .check(Class::DevLogin, client, start + Duration::from_secs(3))
                .is_err()
        );
    }

    #[test]
    fn a_campus_network_can_start_sixty_sign_ins_a_minute() {
        let limiter = RateLimiter::new();
        let campus = ip("128.2.0.1");
        let start = Instant::now();
        for _ in 0..60 {
            assert!(limiter.check(Class::SignIn, campus, start).is_ok());
        }
        assert!(limiter.check(Class::SignIn, campus, start).is_err());
        // One more every second.
        assert!(
            limiter
                .check(Class::SignIn, campus, start + Duration::from_secs(1))
                .is_ok()
        );
    }

    #[test]
    fn idle_buckets_are_pruned() {
        let limiter = RateLimiter::new();
        let start = Instant::now();
        for n in 0..100u8 {
            limiter
                .check(Class::Write, IpAddr::from([198, 51, 100, n]), start)
                .unwrap();
        }
        assert_eq!(limiter.len(), 100);
        limiter
            .check(
                Class::Write,
                ip("198.51.100.200"),
                start + Duration::from_secs(61),
            )
            .unwrap();
        assert_eq!(limiter.len(), 1);
    }

    #[test]
    fn trusts_forwarded_for_only_behind_a_local_proxy() {
        let mut headers = HeaderMap::new();
        headers.append(
            "x-forwarded-for",
            HeaderValue::from_static("1.1.1.1, 203.0.113.7"),
        );
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &headers, true),
            ip("203.0.113.7")
        );
        assert_eq!(
            client_ip(Some(ip("::1")), &headers, true),
            ip("203.0.113.7")
        );
        // A remote peer cannot pick its own address.
        assert_eq!(
            client_ip(Some(ip("198.51.100.4")), &headers, true),
            ip("198.51.100.4")
        );
        // Without the header the proxy itself is the client.
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &HeaderMap::new(), true),
            ip("127.0.0.1")
        );
        let mut garbage = HeaderMap::new();
        garbage.append("x-forwarded-for", HeaderValue::from_static("nonsense"));
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &garbage, true),
            ip("127.0.0.1")
        );
    }

    #[test]
    fn cloudflare_names_the_visitor_behind_kennels_proxy() {
        // What arrives through Cloudflare, the tunnel and Caddy: Caddy rewrote X-Forwarded-For to the tunnel's address.
        let mut headers = HeaderMap::new();
        headers.append("x-forwarded-for", HeaderValue::from_static("127.0.0.1"));
        headers.append("cf-connecting-ip", HeaderValue::from_static("198.51.100.7"));
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &headers, true),
            ip("198.51.100.7")
        );
        let mut v6 = HeaderMap::new();
        v6.append(
            "cf-connecting-ip",
            HeaderValue::from_static("2001:db8:5:6:7::1"),
        );
        assert_eq!(client_ip(Some(ip("::1")), &v6, true), ip("2001:db8:5:6::"));
        // A remote peer cannot pick its own address this way either.
        assert_eq!(
            client_ip(Some(ip("203.0.113.9")), &headers, true),
            ip("203.0.113.9")
        );
        // A header that is not an address falls back to X-Forwarded-For.
        let mut garbage = HeaderMap::new();
        garbage.append("cf-connecting-ip", HeaderValue::from_static("nonsense"));
        garbage.append("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &garbage, true),
            ip("203.0.113.7")
        );
    }

    #[test]
    fn proxy_headers_count_only_when_the_service_listens_on_loopback() {
        let mut headers = HeaderMap::new();
        headers.append("cf-connecting-ip", HeaderValue::from_static("198.51.100.7"));
        headers.append("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &headers, true),
            ip("198.51.100.7")
        );
        // Listening on every interface: a loopback peer is just a client, whatever it sends.
        assert_eq!(
            client_ip(Some(ip("127.0.0.1")), &headers, false),
            ip("127.0.0.1")
        );
        assert_eq!(client_ip(Some(ip("::1")), &headers, false), ip("::"));
        assert_eq!(
            client_ip(Some(ip("198.51.100.4")), &headers, false),
            ip("198.51.100.4")
        );
    }

    #[test]
    fn ipv6_clients_are_grouped_by_prefix() {
        let headers = HeaderMap::new();
        assert_eq!(
            client_ip(Some(ip("2001:db8:1:2:aaaa::1")), &headers, true),
            ip("2001:db8:1:2::")
        );
        assert_eq!(
            client_ip(Some(ip("::ffff:203.0.113.5")), &headers, true),
            ip("203.0.113.5")
        );
    }
}
