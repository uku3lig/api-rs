mod model;
mod util;

use crate::model::{ModrinthProject, ShieldsBadge, TierList};
use crate::util::{AppError, IntoAppError};
use anyhow::Result;
use axum::extract::Path;
use axum::routing::get;
use axum::{Json, Router};
use moka::future::Cache;
use once_cell::sync::{Lazy, OnceCell};
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Client, StatusCode};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};
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

static TIMER_KEY: OnceCell<String> = OnceCell::new();

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Failed to load .env file: {}", e);
    }

    tracing_subscriber::fmt::init();

    let downloads_route = Router::new()
        .route("/:name", get(downloads))
        .route("/:name/shields", get(downloads_shields));

    let tiers_route = Router::new().route("/:mode", get(get_tiers));

    let mut app = Router::new()
        .nest("/downloads", downloads_route)
        .nest("/tiers", tiers_route)
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(|_: &_, _: &_| {}));

    if let Ok(key) = env::var("TIMER_KEY") {
        TIMER_KEY.set(key).expect("could not set timer key");
        app = app.route("/timer/:key", get(check_timer))
    }

    let socket_addr = env::var("SOCKET_ADDR").unwrap_or("0.0.0.0:5000".into());
    let socket_addr = SocketAddr::from_str(socket_addr.as_str())?;
    tracing::info!("listening on {}", socket_addr);

    let server = axum::Server::bind(&socket_addr).serve(app.into_make_service());

    server
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Could not register ctrl+c handler");
        })
        .await?;

            tracing::info!("shutting down...");

    Ok(())
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

static TIERS_CACHE: Lazy<Cache<String, HashMap<String, String>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(900))
        .build()
});

async fn get_tiers(Path(mode): Path<String>) -> RouteResponse<Json<HashMap<String, String>>> {
    let info = match TIERS_CACHE.get(&mode).await {
        Some(v) => v,
        None => {
            let req = CLIENT
                .get(format!("https://mctiers.com/api/tier/{mode}?count=32767"))
                .build()?;

            let res: TierList = CLIENT
                .execute(req)
                .await?
                .error_for_status()?
                .json()
                .await?;

            let info = res.into_tiered_info();

            let mut map = HashMap::new();
            for i in info.iter() {
                let high = if i.high { "H" } else { "L" };
                map.insert(i.name.clone(), format!("{}T{}", high, i.tier));
            }

            TIERS_CACHE.insert(mode, map.clone()).await;

            map
        }
    };

    Ok(Json(info))
}

async fn check_timer(Path(key): Path<String>) -> RouteResponse<String> {
    let Some(correct_key) = TIMER_KEY.get() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "timer key is not set").into_app_err();
    };

    if *correct_key != key {
        return (StatusCode::BAD_REQUEST, "no").into_app_err();
    }

    let seconds = UNIX_EPOCH.elapsed()?.as_secs();
    std::fs::write("last_checked.txt", format!("{}", seconds))?;

    Ok("success".into())
}
