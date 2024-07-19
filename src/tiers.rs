use std::time::Duration;
use std::{collections::HashMap, fmt::Display};

use axum::{extract::Path, response::IntoResponse, routing::get, Json, Router};
use moka::future::{Cache, CacheBuilder};
use once_cell::sync::Lazy;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::RouteResponse;

const MODES: [&str; 7] = ["axe", "neth_pot", "pot", "smp", "sword", "uhc", "vanilla"];

static PLAYER_CACHE: Lazy<Cache<Uuid, Option<PlayerInfo>>> = Lazy::new(|| {
    CacheBuilder::new(65535)
        .time_to_live(Duration::from_secs(60 * 60 * 72)) // 72 hours
        .time_to_idle(Duration::from_secs(60 * 60 * 6)) // 2 hours
        .build()
});

static MODE_CACHE: Lazy<Cache<String, HashMap<String, String>>> = Lazy::new(|| {
    CacheBuilder::new(MODES.len() as u64)
        .time_to_live(Duration::from_secs(60 * 60 * 24)) // 24 hours
        .build()
});

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerInfo {
    uuid: Uuid,
    name: String,
    rankings: HashMap<String, Ranking>,
    region: String,
    points: u32,
    overall: u32,
    badges: Vec<Badge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ranking {
    tier: u8,
    pos: u8,
    peak_tier: Option<u8>,
    peak_pos: Option<u8>,
    attained: i64,
    retired: bool,
}

impl Display for Ranking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let high = if self.pos == 0 { 'H' } else { 'L' };
        write!(f, "{}T{}", high, self.tier)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Badge {
    title: String,
    desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllPlayerInfo {
    players: Vec<PlayerInfo>,
    unknown: Vec<Uuid>,
}

// === Routes ===

pub fn router() -> Router {
    Router::new()
        .route("/all", get(get_all))
        .route("/profile/:uuid", get(get_tier))
        .route("/search_profile/:name", get(search_profile))
        .route("/:mode", get(get_mode))
}

pub async fn get_tier(Path(uuid): Path<String>) -> RouteResponse<impl IntoResponse> {
    let Ok(uuid) = Uuid::try_parse(&uuid) else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    // uuid version 4 and ietf variant, used by UUID#randomUUID
    if uuid.get_version() != Some(uuid::Version::Random)
        || uuid.get_variant() != uuid::Variant::RFC4122
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let profile = PLAYER_CACHE
        .entry(uuid)
        .or_insert_with(fetch_tier(&uuid))
        .await
        .into_value();

    let res = match profile {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(p) => Json(p).into_response(),
    };

    Ok(res)
}

pub async fn get_all() -> RouteResponse<Json<AllPlayerInfo>> {
    let mut players = Vec::new();
    let mut unknown = Vec::new();

    for (uuid, profile) in PLAYER_CACHE.iter() {
        match profile {
            Some(p) => players.push(p),
            None => unknown.push(*uuid),
        }
    }

    Ok(Json(AllPlayerInfo { players, unknown }))
}

pub async fn get_mode(Path(mode): Path<String>) -> RouteResponse<Json<HashMap<String, String>>> {
    let tiers = if MODES.contains(&mode.as_str()) {
        MODE_CACHE
            .entry_by_ref(&mode)
            .or_insert_with(async { get_mode_tiers(&mode) })
            .await
            .into_value()
    } else {
        HashMap::new()
    };

    Ok(Json(tiers))
}

/// (technically) no-op. forwards the request straight to mctiers
///
/// BEWARE !!!!!!!!!!!!!!!!!!! uuid is now returned WITH dashes !!!!
pub async fn search_profile(Path(name): Path<String>) -> RouteResponse<Json<PlayerInfo>> {
    let url = format!("https://mctiers.com/api/search_profile/{name}");

    let player: PlayerInfo = crate::CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    PLAYER_CACHE.insert(player.uuid, Some(player.clone())).await;

    Ok(Json(player))
}

// === Utility functions ===

async fn fetch_tier(uuid: &Uuid) -> Option<PlayerInfo> {
    let url = format!("https://mctiers.com/api/profile/{}", uuid.as_simple());

    let response = crate::CLIENT
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            if e.status() != Some(StatusCode::NOT_FOUND) {
                tracing::warn!("Failed to fetch profile `{uuid}`: {e}");
            }

            return None;
        }
    };

    match response.json().await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Failed to parse profile `{uuid}`: {e}");
            None
        }
    }
}

fn get_mode_tiers(mode: &str) -> HashMap<String, String> {
    PLAYER_CACHE
        .iter()
        .filter_map(|(_, p)| p)
        .filter_map(|p| p.rankings.get(mode).map(|r| (p.name, r.to_string())))
        .collect()
}
