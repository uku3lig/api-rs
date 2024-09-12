#![warn(clippy::pedantic)]

mod cache;
mod discord;
mod downloads;
mod metrics;
mod migrate;
mod tiers;
mod util;

use std::env;
use std::sync::{LazyLock, OnceLock};

use axum::routing::get;
use axum::{middleware, Router};
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::StatusCode;
use tokio::signal::unix::{signal, SignalKind};
use tower_http::trace::TraceLayer;

use crate::cache::Storage;
use crate::util::AppError;

const VERSION: &str = env!("CARGO_PKG_VERSION");

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        format!("uku3lig/api-rs/{VERSION}").parse().unwrap(),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap()
});

static CACHE: OnceLock<cache::Storage> = OnceLock::new();

type RouteResponse<T> = Result<T, AppError>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Failed to load .env file: {e}");
    }

    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    match args.get(1) {
        Some(arg) if arg == "migrate" => {
            migrate::start_migration()?;
            tracing::info!("migration complete!");
        }
        _ => {
            tokio::try_join!(start_main_app(), metrics::start_metrics_app())?;
            tracing::info!("shutting down!");
        }
    }

    Ok(())
}

async fn start_main_app() -> anyhow::Result<()> {
    let app = Router::new()
        .merge(downloads::router())
        .merge(tiers::router())
        .route("/generate_invite", get(discord::generate_invite))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(|_: &_, _: &_| {}))
        .layer(middleware::from_fn(metrics::track));

    discord::init_bot().await?;

    let storage = Storage::new_from_env().await?;
    CACHE.set(storage).unwrap();

    let socket_addr = env::var("SOCKET_ADDR").unwrap_or("0.0.0.0:5000".into());
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    tracing::info!("main app listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal(SignalKind::terminate()).unwrap().recv().await;
        })
        .await
        .map_err(anyhow::Error::from)
}

fn get_cache() -> &'static Storage {
    CACHE.get().unwrap()
}
