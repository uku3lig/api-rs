#![warn(clippy::pedantic)]

mod discord;
mod downloads;
mod tiers;
mod util;

use crate::util::AppError;
use anyhow::Result;
use axum::routing::get;
use axum::Router;
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Client, StatusCode};
use std::env;
use tokio::signal::unix::{signal, SignalKind};
use tower_http::trace::TraceLayer;

const VERSION: &str = env!("CARGO_PKG_VERSION");
static CLIENT: Lazy<Client> = Lazy::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        format!("uku3lig/api-rs/{VERSION}").parse().unwrap(),
    );

    Client::builder().default_headers(headers).build().unwrap()
});

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Failed to load .env file: {e}");
    }

    tracing_subscriber::fmt::init();

    let app = Router::new()
        .merge(downloads::router())
        .merge(tiers::router())
        .route("/generate_invite", get(discord::generate_invite))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(|_: &_, _: &_| {}));

    discord::init_bot().await?;

    let socket_addr = env::var("SOCKET_ADDR").unwrap_or("0.0.0.0:5000".into());
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal(SignalKind::terminate()).unwrap().recv().await;
        })
        .await?;

    tracing::info!("shutting down!");

    Ok(())
}

type RouteResponse<T> = Result<T, AppError>;
