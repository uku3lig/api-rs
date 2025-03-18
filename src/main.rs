#![warn(clippy::pedantic)]

mod cache;
mod config;
mod discord;
mod downloads;
mod metrics;
mod tiers;
mod twitter;
mod util;

use std::sync::{Arc, LazyLock};

use axum::{
    Router, middleware,
    routing::{get, post},
};
use reqwest::{
    StatusCode,
    header::{HeaderMap, USER_AGENT},
};
use tokio::signal::unix::{SignalKind, signal};
use tower_http::trace::TraceLayer;

use crate::{cache::Storage, config::EnvCfg, util::AppError};

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

struct AppState {
    config: EnvCfg,
    cache: cache::Storage,
    http: serenity::http::Http,
}

type RouteResponse<T> = Result<T, AppError>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Failed to load .env file: {e}");
    }

    tracing_subscriber::fmt::init();

    let config = envy::from_env::<EnvCfg>()?;
    let metrics_addr = config.metrics_socket_addr.clone();

    tokio::try_join!(
        start_main_app(config),
        metrics::start_metrics_app(metrics_addr)
    )?;

    tracing::info!("shutting down!");

    Ok(())
}

async fn start_main_app(config: EnvCfg) -> anyhow::Result<()> {
    let http = discord::init_bot(&config).await?;
    let cache = Storage::new(&config.redis_url).await?;

    let state = Arc::new(AppState {
        config,
        cache,
        http,
    });

    let app = Router::new()
        .merge(downloads::router())
        .merge(tiers::router())
        .route("/generate_invite", get(discord::generate_invite))
        .route("/twitter", post(twitter::webhook))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(|_: &_, _: &_| {}))
        .layer(middleware::from_fn(metrics::track))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&state.config.socket_addr).await?;
    tracing::info!("main app listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal(SignalKind::terminate()).unwrap().recv().await;
        })
        .await
        .map_err(anyhow::Error::from)
}
