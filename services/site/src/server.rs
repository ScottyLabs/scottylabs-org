//! Accepting connections and bounding how long anything may take: request headers, an idle keep-alive connection, a request body that stops arriving, a whole request, and shutting down.
//!
//! `axum::serve` gives hyper no timer, so hyper's header timeout never runs and nothing else is bounded. This serves the same router with hyper directly, over HTTP/1.1 only: kennel's Caddy, its health probe and Vite's proxy all speak it, and HTTP/2's protocol detection would wait for a silent connection's first bytes without a timeout.

use std::future::Future;
use std::pin::pin;
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use tokio::net::TcpListener;
use tower::Service;
use tower_http::timeout::TimeoutBody;

use crate::error::AppError;

/// The most a request may take to produce its response, except uploads. The body of a response, such as an image streamed to a slow phone, is not counted.
pub const REQUEST_LIMIT: Duration = Duration::from_secs(30);
/// Uploads read up to 5 MB from the visitor and decode the image, which may wait for a free slot.
pub const UPLOAD_LIMIT: Duration = Duration::from_secs(120);

/// How long connections may take and sit idle.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The most a client may take to send a request's headers. On HTTP/1 the timer also runs while a kept-alive connection waits for its next request, so it is the idle limit too.
    pub header_read: Duration,
    /// The longest pause allowed while a request body arrives.
    pub body_idle: Duration,
    /// After a shutdown signal, how long open requests may finish before the process exits anyway. Kennel waits 10 seconds for a unit to stop before it starts the next release.
    pub shutdown_grace: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            header_read: Duration::from_secs(30),
            body_idle: Duration::from_secs(30),
            shutdown_grace: Duration::from_secs(8),
        }
    }
}

/// A failed `accept`, such as running out of file descriptors, is retried after this pause instead of at once.
const ACCEPT_ERROR_PAUSE: Duration = Duration::from_millis(100);

/// Serves `app` until `shutdown` completes, then gives open requests [`Limits::shutdown_grace`] to finish.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    limits: Limits,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(limits.header_read);
    let graceful = GracefulShutdown::new();
    let mut shutdown = pin!(shutdown);

    loop {
        let (stream, remote) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                Err(error) => {
                    tracing::warn!(error = %error, "could not accept a connection");
                    tokio::time::sleep(ACCEPT_ERROR_PAUSE).await;
                    continue;
                }
            },
            () = &mut shutdown => break,
        };
        let _ = stream.set_nodelay(true);
        // Every request carries the peer's address, as `ConnectInfo`, and a body that gives up after `body_idle` without new bytes.
        let app = app.clone();
        let body_idle = limits.body_idle;
        let service = hyper::service::service_fn(move |request: hyper::Request<Incoming>| {
            let mut request = request.map(|body| Body::new(TimeoutBody::new(body_idle, body)));
            request.extensions_mut().insert(ConnectInfo(remote));
            let mut app = app.clone();
            async move { app.call(request).await }
        });
        let connection = graceful.watch(builder.serve_connection(TokioIo::new(stream), service));
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::debug!(error = %error, "a connection ended with an error");
            }
        });
    }

    // Stop accepting, let open requests finish, and close idle connections.
    drop(listener);
    let open = graceful.count();
    tracing::info!(open, "shutting down");
    tokio::select! {
        () = graceful.shutdown() => {}
        () = tokio::time::sleep(limits.shutdown_grace) => tracing::warn!(
            grace_seconds = limits.shutdown_grace.as_secs(),
            "requests were still open when the shutdown grace ran out; stopping anyway"
        ),
    }
    Ok(())
}

/// Binds `addr`, with its address in the error.
pub async fn bind(addr: &str) -> anyhow::Result<TcpListener> {
    TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))
}

/// Ends requests that take longer than [`REQUEST_LIMIT`] ([`UPLOAD_LIMIT`] for uploads) with 503, so a stalled dependency cannot hold connections and workers without end.
pub async fn limit_duration(request: Request, next: Next) -> Response {
    let limit = if request.method() == Method::POST && request.uri().path() == "/api/media" {
        UPLOAD_LIMIT
    } else {
        REQUEST_LIMIT
    };
    within(limit, next.run(request)).await
}

async fn within(limit: Duration, response: impl Future<Output = Response>) -> Response {
    match tokio::time::timeout(limit, response).await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!(
                limit_seconds = limit.as_secs(),
                "a request took too long and was stopped"
            );
            AppError::Unavailable("That took too long. Try again in a moment.".into())
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Instant;

    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;

    fn quick() -> Limits {
        Limits {
            header_read: Duration::from_millis(300),
            body_idle: Duration::from_millis(300),
            shutdown_grace: Duration::from_millis(500),
        }
    }

    fn app() -> Router {
        Router::new()
            .route(
                "/peer",
                get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.ip().to_string() }),
            )
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    "late"
                }),
            )
            .route(
                "/echo",
                post(|body: axum::body::Bytes| async move { body.len().to_string() }),
            )
    }

    /// Starts the server on a free port. Sending on the returned channel shuts it down; the task ends when serving does.
    async fn start(
        limits: Limits,
    ) -> (
        SocketAddr,
        tokio::sync::oneshot::Sender<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            serve(listener, app(), limits, async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
        });
        (addr, stop, task)
    }

    async fn read_all(stream: &mut TcpStream) -> String {
        let mut text = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut text)).await;
        String::from_utf8_lossy(&text).into_owned()
    }

    #[tokio::test]
    async fn requests_carry_the_peer_address() {
        let (addr, _stop, _task) = start(quick()).await;
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /peer HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let answer = read_all(&mut stream).await;
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.ends_with("127.0.0.1"), "{answer}");
    }

    #[tokio::test]
    async fn slow_headers_and_idle_connections_are_closed() {
        let (addr, _stop, _task) = start(quick()).await;
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(b"GET /peer HTTP/1.1\r\nHo").await.unwrap();
        let started = Instant::now();
        let answer = read_all(&mut stream).await;
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!answer.contains("200"), "{answer}");

        // A connection that never sends anything is closed too.
        let mut idle = TcpStream::connect(addr).await.unwrap();
        let started = Instant::now();
        let _ = read_all(&mut idle).await;
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn a_body_that_stops_arriving_is_given_up() {
        let (addr, _stop, _task) = start(quick()).await;
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 100\r\n\r\nhalf")
            .await
            .unwrap();
        let started = Instant::now();
        let answer = read_all(&mut stream).await;
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!answer.starts_with("HTTP/1.1 200"), "{answer}");
    }

    #[tokio::test]
    async fn shutting_down_does_not_wait_for_stuck_requests() {
        let (addr, stop, task) = start(quick()).await;
        let mut stuck = TcpStream::connect(addr).await.unwrap();
        stuck
            .write_all(b"GET /slow HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = Instant::now();
        stop.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the server stops within its grace")
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        // New connections are refused once it stopped.
        assert!(TcpStream::connect(addr).await.is_err());
    }

    #[tokio::test]
    async fn requests_over_their_limit_answer_503() {
        let started = Instant::now();
        let response = within(Duration::from_millis(100), async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            StatusCode::OK.into_response()
        })
        .await;
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["retry-after"], "5");
        let quick = within(Duration::from_secs(5), async {
            StatusCode::OK.into_response()
        })
        .await;
        assert_eq!(quick.status(), StatusCode::OK);
    }
}
