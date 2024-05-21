#![warn(clippy::pedantic)]

mod discord;
mod model;
mod util;

use crate::model::{ModrinthProject, ShieldsBadge};
use crate::util::AppError;
use anyhow::Result;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::Client;
use std::env;
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

    let downloads_route = Router::new()
        .route("/:name", get(downloads))
        .route("/:name/shields", get(downloads_shields));

    let app = Router::new()
        .nest("/downloads", downloads_route)
        .route("/generate_invite", get(discord::generate_invite))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(|_: &_, _: &_| {}));

    discord::init_bot().await?;

    let socket_addr = env::var("SOCKET_ADDR").unwrap_or("0.0.0.0:5000".into());
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    // TODO graceful shutdown (not implemented in 0.7 yet, but not a big loss so idc i'll just wait)

    Ok(())
}

type RouteResponse<T> = Result<T, AppError>;

async fn downloads(Path(name): Path<String>) -> RouteResponse<String> {
    let request = CLIENT
        .get(format!("https://api.modrinth.com/v2/user/{name}/projects"))
        .build()?;

    let response: Vec<ModrinthProject> = CLIENT
        .execute(request)
        .await?
        .error_for_status()?
        .json()
        .await?;

    let sum: usize = response.iter().map(|p| p.downloads).sum();

    Ok(format!("{sum}"))
}

async fn downloads_shields(Path(name): Path<String>) -> RouteResponse<Json<ShieldsBadge>> {
    let count: u32 = downloads(Path(name)).await?.parse().unwrap();
    let formatted = util::format_number(count);

    let shield = ShieldsBadge {
        schema_version: 1,
        label: "downloads".into(),
        message: formatted,
        color: "brightgreen".into(),
        named_logo: "modrinth".into(),
    };

    Ok(Json(shield))
}
