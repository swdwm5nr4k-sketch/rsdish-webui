//! Static frontend, compiled into the binary.

use axum::http::header;
use axum::response::IntoResponse;

pub const INDEX_HTML: &str = include_str!("assets/index.html");
pub const APP_JS: &str = include_str!("assets/app.js");
pub const STYLE_CSS: &str = include_str!("assets/style.css");
pub const LOGO_PNG: &[u8] = include_bytes!("../../assets/parfait_gpt.png");

/// Assets are embedded, so "no-cache" is enough to always ship the current build.
const NO_CACHE: (header::HeaderName, &str) = (header::CACHE_CONTROL, "no-cache");

pub async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8"), NO_CACHE],
        INDEX_HTML,
    )
}

pub async fn app_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            NO_CACHE,
        ],
        APP_JS,
    )
}

pub async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8"), NO_CACHE],
        STYLE_CSS,
    )
}

pub async fn logo() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/png"), NO_CACHE], LOGO_PNG)
}

pub async fn favicon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/png"), NO_CACHE], LOGO_PNG)
}
