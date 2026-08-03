//! HTTP scrape server (`jentayu serve`) — I/O edge only.

mod config;
mod policy;

pub use config::{
    build_serve_config, build_serve_config_with_timeout, load_token, parse_bind, ServeConfig,
    TOKEN_ENV,
};
pub use policy::{
    auth_required, bind_allowed, effective_top, is_loopback, parse_bearer_authorization,
    snapshot_for_serve, tokens_equal, BindPolicyError, DEFAULT_BIND, DEFAULT_COLLECT_TIMEOUT,
    DEFAULT_TOP, HARD_CAP, MAX_TOKEN_BYTES,
};

use crate::collector::Collector;
use crate::export::render_prometheus;
use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{info, warn};

/// Shared state for scrapes.
#[derive(Clone)]
struct AppState {
    collector: Arc<Mutex<Collector>>,
    config: Arc<ServeConfig>,
}

/// Run the scrape server until Ctrl-C (or listener error).
pub async fn run(config: ServeConfig, collector: Collector) -> Result<()> {
    let bind = config.bind;
    let state = AppState {
        collector: Arc::new(Mutex::new(collector)),
        config: Arc::new(config),
    };

    let app = router(state);

    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind scrape listener on {bind}"))?;
    let local = listener
        .local_addr()
        .with_context(|| format!("failed to read local address for {bind}"))?;
    info!(%local, "listening for Prometheus scrapes on GET /metrics");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutdown signal received");
        })
        .await
        .context("serve error")?;
    Ok(())
}

/// Build the axum router (also used by integration tests).
fn router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

async fn metrics_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    // Auth before lock / collect (do not let unauthenticated load queue on collector).
    if let Some(expected) = state.config.token.as_deref() {
        let presented = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_bearer_authorization);
        let ok = presented.is_some_and(|t| tokens_equal(t, expected));
        if !ok {
            return (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Bearer")],
                "unauthorized\n",
            )
                .into_response();
        }
    }

    let collect_timeout = state.config.collect_timeout;
    let collect = timeout(collect_timeout, async {
        let mut guard = state.collector.lock().await;
        guard.snapshot().await
    })
    .await;

    let snap = match collect {
        Ok(s) => s,
        Err(_) => {
            warn!(
                timeout_ms = collect_timeout.as_millis() as u64,
                "collect budget exceeded; returning 503"
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "service unavailable: collect budget exceeded\n",
            )
                .into_response();
        }
    };

    let ranked = snapshot_for_serve(
        &snap,
        state.config.name_filter.as_deref(),
        state.config.sort_by,
        state.config.top,
    );
    let body = render_prometheus(&ranked);
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::{CollectMode, Collector, ContainerCollector};
    use crate::metrics::SortBy;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::net::SocketAddr;
    use std::time::Duration;
    use tower::ServiceExt;

    fn test_collector() -> Collector {
        Collector::with_containers(CollectMode::Processes, ContainerCollector::disabled())
    }

    fn cfg(token: Option<&str>, timeout_ms: u64) -> ServeConfig {
        ServeConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            allow_remote: false,
            token: token.map(str::to_string),
            name_filter: None,
            sort_by: SortBy::Cpu,
            top: Some(5),
            collect_timeout: Duration::from_millis(timeout_ms),
        }
    }

    fn state(config: ServeConfig) -> AppState {
        AppState {
            collector: Arc::new(Mutex::new(test_collector())),
            config: Arc::new(config),
        }
    }

    async fn call(app: Router, req: Request<Body>) -> (StatusCode, String) {
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    #[tokio::test]
    async fn metrics_ok_contains_jentayu() {
        let app = router(state(cfg(None, 5_000)));
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("jentayu_"),
            "body missing jentayu_ prefix: {}",
            &body[..body.len().min(200)]
        );
    }

    #[tokio::test]
    async fn metrics_unauthorized_without_token() {
        let app = router(state(cfg(Some("secret"), 5_000)));
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!body.contains("jentayu_host"));
    }

    #[tokio::test]
    async fn metrics_unauthorized_wrong_token() {
        let app = router(state(cfg(Some("secret"), 5_000)));
        let (status, _) = call(
            app,
            Request::builder()
                .uri("/metrics")
                .header(header::AUTHORIZATION, "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn metrics_ok_with_correct_bearer() {
        let app = router(state(cfg(Some("secret"), 5_000)));
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/metrics")
                .header(header::AUTHORIZATION, "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("jentayu_"));
    }

    #[tokio::test]
    async fn metrics_bearer_case_insensitive() {
        let app = router(state(cfg(Some("secret"), 5_000)));
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/metrics")
                .header(header::AUTHORIZATION, "bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("jentayu_"));
    }

    #[tokio::test]
    async fn unknown_path_not_found() {
        let app = router(state(cfg(None, 5_000)));
        let (status, _) = call(
            app,
            Request::builder()
                .uri("/healths")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn collect_timeout_returns_503_without_metrics() {
        let collector = Arc::new(Mutex::new(test_collector()));
        // Hold the lock longer than the collect budget so lock().await cannot finish.
        let hold = collector.clone();
        let blocker = tokio::spawn(async move {
            let _guard = hold.lock().await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        });
        // Let the blocker acquire the lock first.
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = AppState {
            collector: collector.clone(),
            config: Arc::new(cfg(None, 30)),
        };
        let app = router(state);
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            !body.contains("jentayu_"),
            "503 must not include metric series"
        );
        let _ = blocker.await;
    }

    #[tokio::test]
    async fn live_listener_smoke() {
        let config = cfg(None, 5_000);
        let state = state(config);
        let app = router(state);
        let listener = TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Minimal HTTP/1.1 GET via tokio TcpStream + write/read.
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("200"), "response: {text}");
        assert!(text.contains("jentayu_"), "response: {text}");
        server.abort();
    }
}
