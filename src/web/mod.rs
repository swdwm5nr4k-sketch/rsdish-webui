//! Embedded web UI for rsdish.
//!
//! `rsdish web` (or the `rsdish-webui` binary) serves a small HTTP API on
//! top of the very same library the CLI uses, plus a static frontend that is
//! compiled into the binary - no node, no build step, no separate assets to ship.

mod api;
mod assets;
mod dto;
pub mod logs;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post, put};
use tracing::info;

pub use state::AppState;

use crate::web::api::{
    browse, cabinet_init, cabinet_join, cabinet_preset, cabinet_put_config, cabinet_read,
    get_config, group_clean, group_exec, group_link, group_plan, group_sync, health, logs,
    overview, put_config,
};

/// How the web UI should be served.
#[derive(Debug, Clone)]
pub struct WebOptions {
    pub host: String,
    pub port: u16,
    pub open: bool,
}

impl Default for WebOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8787,
            open: false,
        }
    }
}

/// Build the router. Exposed for tests.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(assets::index))
        .route("/favicon.ico", get(assets::favicon))
        .route("/assets/app.js", get(assets::app_js))
        .route("/assets/style.css", get(assets::style_css))
        .route("/assets/logo.png", get(assets::logo))
        .route("/api/health", get(health))
        .route("/api/overview", get(overview))
        .route("/api/browse", get(browse))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/logs", get(logs))
        .route("/api/groups/{uuid}/plan", get(group_plan))
        .route("/api/groups/{uuid}/sync", post(group_sync))
        .route("/api/groups/{uuid}/link", post(group_link))
        .route("/api/groups/{uuid}/clean", post(group_clean))
        .route("/api/groups/{uuid}/exec", post(group_exec))
        .route("/api/cabinets", get(cabinet_read))
        .route("/api/cabinets/init", post(cabinet_init))
        .route("/api/cabinets/join", post(cabinet_join))
        .route("/api/cabinets/preset", post(cabinet_preset))
        .route("/api/cabinets/config", put(cabinet_put_config))
        .with_state(state)
}

/// Serve the web UI until Ctrl-C.
pub async fn serve(opts: WebOptions) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", opts.host, opts.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", opts.host, opts.port))?;

    let state = Arc::new(AppState::new());
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {}", addr))?;

    let local = listener.local_addr().context("failed to read local addr")?;
    let url = format!("http://{}", local);

    info!("rsdish webui listening on {}", url);
    println!("rsdish webui listening on {}", url);
    println!("press Ctrl-C to stop");

    if opts.open {
        open_browser(&url);
    }

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("web server failed")?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::error!("failed to listen for Ctrl-C: {}", e);
        return;
    }
    info!("shutting down rsdish webui");
}

/// Best effort "open this URL in the default browser".
pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };

    if let Err(e) = cmd.spawn() {
        tracing::warn!("failed to open browser for {}: {}", url, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn app() -> Router {
        router(Arc::new(AppState::new()))
    }

    #[tokio::test]
    async fn health_is_served() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn index_and_embedded_assets_are_served() {
        for uri in [
            "/",
            "/assets/app.js",
            "/assets/style.css",
            "/assets/logo.png",
        ] {
            let response = app()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{} should be served",
                uri
            );
        }
    }

    #[tokio::test]
    async fn unknown_route_is_a_404() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_group_is_a_404_with_a_json_error() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/groups/not-a-real-uuid/plan")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            text.contains("unknown group uuid"),
            "unexpected body: {}",
            text
        );
    }

    #[tokio::test]
    async fn empty_script_is_rejected() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/groups/whatever/exec")
                    .header("content-type", "application/json")
                    .body(Body::from("{\"script\":\"   \"}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
