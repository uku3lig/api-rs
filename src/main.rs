mod model;
mod util;

use crate::model::{ModrinthProject, ShieldsBadge};
use crate::util::{AppError, RequestTracer};
use anyhow::Result;
use axum::extract::Path;
use axum::routing::get;
use axum::{Json, Router};
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Client, StatusCode};
use std::net::SocketAddr;
use std::str::FromStr;
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
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/downloads/:name", get(downloads))
        .route("/downloads/:name/shields", get(downloads_shields))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(RequestTracer));

    let socket_addr = SocketAddr::from_str("0.0.0.0:3000")?;
    tracing::info!("listening on {}", socket_addr);

    Ok(axum::Server::bind(&socket_addr)
        .serve(app.into_make_service())
        .await?)
}

type RouteResponse<T> = Result<T, AppError>;

async fn downloads(Path(name): Path<String>) -> RouteResponse<String> {
    let req = CLIENT
        .get(format!("https://api.modrinth.com/v2/user/{name}/projects"))
        .build()?;

    let res: Vec<ModrinthProject> = CLIENT
        .execute(req)
        .await?
        .error_for_status()?
        .json()
        .await?;

    let sum: usize = res.iter().map(|p| p.downloads).sum();

    Ok(format!("{sum}"))
}

async fn downloads_shields(Path(name): Path<String>) -> RouteResponse<Json<ShieldsBadge>> {
    let count: usize = downloads(Path(name)).await?.parse().unwrap();
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
