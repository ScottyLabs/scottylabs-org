//! Production answers only on its own host. Kennel also routes a production deployment's generated `*.scottylabs.net` host to the same service, where sign-in can never finish (the relay returns to `APP_URL`) and every page would be published twice. When `APP_URL` is a production host, a request that names another host is sent to the same path on `APP_URL` with a 308, if it reads; one that writes is refused.
//!
//! The health check is exempt, since kennel probes it on `127.0.0.1`. Requests without a host (HTTP/1.0, tests) pass.

use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, HOST, LOCATION};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::config::{Config, is_production_host};
use crate::error::AppError;
use crate::state::AppState;

const EXEMPT_PATHS: [&str; 1] = ["/api/health"];
/// Browsers and the CDN may keep the redirect for a day.
const REDIRECT_CACHE: &str = "public, max-age=86400";

pub async fn canonical_host(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    match redirect_for(&state.config, &request) {
        None => next.run(request).await,
        Some(Answer::Refuse) => AppError::Forbidden(format!(
            "Changes can only be made at {}. Open the portal there and try again.",
            state.config.origin()
        ))
        .into_response(),
        Some(Answer::Redirect(location)) => {
            let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
            let headers = response.headers_mut();
            headers.insert(LOCATION, location);
            headers.insert(CACHE_CONTROL, HeaderValue::from_static(REDIRECT_CACHE));
            response
        }
    }
}

enum Answer {
    Redirect(HeaderValue),
    Refuse,
}

fn redirect_for(config: &Config, request: &Request) -> Option<Answer> {
    let canonical = config.app_url.host_str()?;
    if !is_production_host(canonical) || EXEMPT_PATHS.contains(&request.uri().path()) {
        return None;
    }
    let host = request_host(request)?;
    if host.eq_ignore_ascii_case(canonical) {
        return None;
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return Some(Answer::Refuse);
    }
    let path_and_query = request
        .uri()
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let location = format!("{}{path_and_query}", config.origin());
    HeaderValue::from_str(&location).ok().map(Answer::Redirect)
}

/// The host the request names, without a port: the `Host` header, or the authority of an absolute request target.
fn request_host(request: &Request) -> Option<&str> {
    let raw = match request.headers().get(HOST) {
        Some(value) => value.to_str().ok()?,
        None => request.uri().authority()?.as_str(),
    };
    let host = if let Some(rest) = raw.strip_prefix('[') {
        // An IPv6 literal such as [::1]:8080.
        rest.split(']').next().unwrap_or_default()
    } else {
        raw.split(':').next().unwrap_or_default()
    };
    Some(host.trim_end_matches('.'))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use url::Url;

    use super::*;

    fn production() -> Config {
        Config {
            app_url: Url::parse("https://scottylabs.org").unwrap(),
            ..crate::testing::config()
        }
    }

    fn request(method: Method, uri: &str, host: Option<&str>) -> Request {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(host) = host {
            builder = builder.header(HOST, host);
        }
        builder.body(Body::empty()).unwrap()
    }

    fn location(answer: Option<Answer>) -> Option<String> {
        match answer {
            Some(Answer::Redirect(value)) => Some(value.to_str().unwrap().to_owned()),
            _ => None,
        }
    }

    #[test]
    fn other_hosts_move_to_the_production_host() {
        let config = production();
        let mirror = Some("scottylabs-org-site-main.scottylabs.net");
        assert_eq!(
            location(redirect_for(
                &config,
                &request(Method::GET, "/projects/nova/?from=x", mirror)
            ))
            .as_deref(),
            Some("https://scottylabs.org/projects/nova/?from=x")
        );
        assert_eq!(
            location(redirect_for(
                &config,
                &request(Method::HEAD, "/api/public/ticker", mirror)
            ))
            .as_deref(),
            Some("https://scottylabs.org/api/public/ticker")
        );
        // The redirect always stays on our origin.
        assert_eq!(
            location(redirect_for(
                &config,
                &request(Method::GET, "//evil.example/x", mirror)
            ))
            .as_deref(),
            Some("https://scottylabs.org//evil.example/x")
        );
        assert!(matches!(
            redirect_for(
                &config,
                &request(Method::POST, "/api/announcements", mirror)
            ),
            Some(Answer::Refuse)
        ));
        assert!(matches!(
            redirect_for(
                &config,
                &request(Method::DELETE, "/api/announcements/x", mirror)
            ),
            Some(Answer::Refuse)
        ));
    }

    #[test]
    fn the_production_host_health_checks_and_other_sites_pass() {
        let config = production();
        for host in ["scottylabs.org", "ScottyLabs.org:443", "scottylabs.org."] {
            assert!(
                redirect_for(&config, &request(Method::POST, "/api/me", Some(host))).is_none(),
                "{host}"
            );
        }
        assert!(
            redirect_for(
                &config,
                &request(Method::GET, "/api/health", Some("127.0.0.1:10234"))
            )
            .is_none()
        );
        assert!(redirect_for(&config, &request(Method::GET, "/", None)).is_none());
        assert_eq!(
            location(redirect_for(
                &config,
                &request(Method::GET, "/", Some("[::1]:10234"))
            ))
            .as_deref(),
            Some("https://scottylabs.org/")
        );

        // Previews and local development answer on whatever host they are reached by.
        let preview = Config {
            app_url: Url::parse("https://scottylabs-org-site-pr-3.scottylabs.net").unwrap(),
            ..crate::testing::config()
        };
        assert!(
            redirect_for(&preview, &request(Method::GET, "/", Some("127.0.0.1:8080"))).is_none()
        );
        assert!(
            redirect_for(
                &crate::testing::config(),
                &request(Method::GET, "/", Some("127.0.0.1:8080"))
            )
            .is_none()
        );
    }
}
