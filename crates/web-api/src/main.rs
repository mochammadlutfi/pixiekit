//! Pixiekit web API server entry point.
//!
//! Configuration via environment:
//! - `PORT` — listen port (default `8765`)
//! - `HOST` — bind address (default `0.0.0.0`)
//! - `CORS_ALLOWED_ORIGINS` — comma-separated list (default localhost dev pair)
//! - `RUST_LOG` — tracing-subscriber filter (default `info`)

use std::net::SocketAddr;

use anyhow::{Context, Result};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use pixiekit_web_api::build_router;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8765);

    let cors_origins = std::env::var("CORS_ALLOWED_ORIGINS").ok().map(|raw| {
        raw.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let app = build_router(Some(cors_origins.unwrap_or_default()));

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("invalid bind address {host}:{port}"))?;

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;

    info!(%addr, "pixiekit-web-api listening");

    axum::serve(listener, app)
        .await
        .context("axum::serve failed")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false))
        .init();
}
