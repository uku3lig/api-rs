mod model;
mod util;

use crate::model::{ModrinthProject, ShieldsBadge};
use crate::util::{AppError, IntoAppError, ToHex};
use anyhow::{anyhow, Result};
use axum::extract::Path;
use axum::routing::get;
use axum::{Json, Router};
use google_sheets4::hyper::client::HttpConnector;
use google_sheets4::hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use google_sheets4::oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};
use google_sheets4::{hyper, Sheets};
use moka::future::Cache;
use once_cell::sync::Lazy;
use pollster::FutureExt;
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
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

static CACHE: Lazy<Cache<String, Value>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(900))
        .build()
});

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Failed to load .env file: {}", e);
    }

    tracing_subscriber::fmt::init();

    let downloads_route = Router::new()
        .route("/:name", get(downloads))
        .route("/:name/shields", get(downloads_shields));

    let tiers_route = Router::new()
        .route("/", get(list_modes))
        .route("/:mode", get(get_tiers));

    let app = Router::new()
        .nest("/downloads", downloads_route)
        .nest("/tiers", tiers_route)
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not Found") })
        .layer(TraceLayer::new_for_http().on_request(|_: &_, _: &_| {}));

    let socket_addr = env::var("SOCKET_ADDR").unwrap_or("0.0.0.0:5000".into());
    let socket_addr = SocketAddr::from_str(socket_addr.as_str())?;
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

static MODES: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    HashMap::from([
        ("sword", "🗡️ Sword"),
        ("vanilla", " 🌄 Vanilla"),
        ("pot", "🧪 Pot"),
        ("nethpot", "♟️ Netherite Pot"),
        ("uhc", "💓 UHC"),
        ("axe", "🪓 Axe"),
        ("smp", "🛡️ SMP"),
    ])
});

const COLUMNS: [&str; 5] = ["B", "D", "F", "H", "J"];

static HUB: Lazy<Sheets<HttpsConnector<HttpConnector>>> = Lazy::new(|| {
    tracing::info!("initializing Google Sheets API client");
    let reader = BufReader::new(File::open("service_account.json").unwrap());
    let secret: ServiceAccountKey = serde_json::from_reader(reader).unwrap();

    let auth = ServiceAccountAuthenticator::builder(secret)
        .build()
        .block_on()
        .unwrap();

    let client = hyper::client::Client::builder().build(
        HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build(),
    );

    Sheets::new(client, auth)
});

async fn list_modes() -> Json<Vec<&'static str>> {
    Json(MODES.keys().copied().collect())
}

async fn get_tiers(Path(mode): Path<String>) -> RouteResponse<Json<Value>> {
    if MODES.get(mode.as_str()).is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid mode; see /tiers for a list".into(),
        )
            .into_app_err());
    }

    if !CACHE.contains_key(&mode) {
        let data = get_spreadsheet_data(MODES.get(mode.as_str()).unwrap()).await?;
        let value = serde_json::to_value(data)?;

        CACHE.insert(mode.clone(), value).await;
    }

    let map = CACHE.get(&mode).ok_or(anyhow!("cache miss"))?;
    Ok(Json(map))
}

async fn get_spreadsheet_data(mode: &str) -> Result<HashMap<String, String>> {
    let mut query = HUB
        .spreadsheets()
        .get("175dbUBlzB3PJY0SV-5j0CcLiXIPxxx_R80OLq4nFC2c")
        .include_grid_data(true);

    for col in COLUMNS {
        query = query.add_ranges(&format!("{mode}!{col}2:{col}"));
    }

    let (res, spreadsheet) = query.doit().await?;

    if !res.status().is_success() {
        Err(anyhow!(
            "Could not get spreadsheet data: {} {:?}",
            res.status(),
            res.body(),
        ))?;
    }

    let sheet = spreadsheet
        .sheets
        .and_then(|s| s.get(0).cloned())
        .ok_or(anyhow!("Could not fetch sheets"))?;

    let columns = sheet
        .data
        .ok_or(anyhow!("Could not fetch grid data"))?
        .iter()
        .filter_map(|d| d.row_data.clone())
        .map(|v| {
            v.iter()
                .filter_map(|r| r.values.clone())
                .flatten()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut tiers = HashMap::new();
    for (i, cells) in columns.iter().enumerate() {
        for cell in cells {
            if let Some(value) = cell.formatted_value.as_ref() {
                let color = cell
                    .effective_format
                    .as_ref()
                    .and_then(|f| f.background_color.clone())
                    .unwrap_or_default()
                    .to_hex();

                match color {
                    0x3c78d8 => {
                        tiers.insert(value.clone(), format!("HT{}", i + 1));
                    }
                    0xa4c2f4 => {
                        tiers.insert(value.clone(), format!("LT{}", i + 1));
                    }
                    _ => {}
                };
            }
        }
    }

    Ok(tiers)
}
