//! Pixiekit SaaS REST API — axum 0.8 server wrapping `pixiekit-core` tools.
//!
//! The library exposes [`build_router`] so integration tests (and future
//! embedded uses) can exercise the same router that the binary serves.

pub mod error;
pub mod routes;
pub mod upload;

use std::time::Duration;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

/// 100 MiB request body limit — videos can be large.
pub const REQUEST_BODY_LIMIT: usize = 100 * 1024 * 1024;

/// Build the axum router with all routes and middleware.
///
/// `cors_origins` controls the CORS allow-list:
/// - `None` → allow `Any` (use only when you know what you're doing)
/// - `Some(empty)` → fall back to localhost dev defaults
///   (`http://localhost:3000`, `http://localhost:5173`)
/// - `Some(list)` → exact origin allow-list
pub fn build_router(cors_origins: Option<Vec<String>>) -> Router {
    let cors = build_cors(cors_origins);

    Router::new()
        .merge(routes::health::router())
        .merge(routes::bg_remove::router())
        .merge(routes::video_to_sprite::router())
        .merge(routes::vectorize::router())
        .merge(routes::presets::router())
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT))
        .layer(TraceLayer::new_for_http())
}

fn build_cors(cors_origins: Option<Vec<String>>) -> CorsLayer {
    let base = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(Duration::from_secs(3600));

    match cors_origins {
        None => base.allow_origin(Any),
        Some(list) => {
            let origins: Vec<_> = if list.is_empty() {
                vec![
                    "http://localhost:3000".parse().unwrap(),
                    "http://localhost:5173".parse().unwrap(),
                ]
            } else {
                list.into_iter().filter_map(|s| s.parse().ok()).collect()
            };
            base.allow_origin(origins)
        }
    }
}
