use std::{sync::Arc, time::Instant};

use axum::{Json, Router, extract::Path, response::IntoResponse, routing::get};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, RouteResponse};

const MCTIERS_REQS_KEY: &str = "api_rs_mctiers_reqs_total";
const MCTIERS_REQ_DURATION_KEY: &str = "api_rs_mctiers_req_duration_seconds";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllPlayerInfo {
    players: Vec<serde_json::Value>,
    unknown: Vec<Uuid>,
    fetch_unknown: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct MojangUUID {
    id: Uuid,
}

// === Routes ===

pub fn router() -> Router<Arc<AppState>> {
    let router = Router::new()
        .route("/all", get(get_all))
        .route("/tierlists", get(get_tierlists))
        .route("/profile/{uuid}", get(get_tier))
        .route("/search_profile/{name}", get(search_profile));

    Router::new().nest("/tiers", router)
}

pub async fn get_tier(Path(uuid): Path<Uuid>) -> RouteResponse<impl IntoResponse> {
    // uuid version 4 and ietf variant, used by UUID#randomUUID
    if uuid.get_version() != Some(uuid::Version::Random)
        || uuid.get_variant() != uuid::Variant::RFC4122
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let (profile, code) = fetch_tier(&uuid).await;

    let res = match profile {
        None => code.into_response(),
        Some(p) => Json(p).into_response(),
    };

    Ok(res)
}

pub async fn get_all() -> RouteResponse<Json<AllPlayerInfo>> {
    Ok(Json(AllPlayerInfo {
        players: Vec::new(),
        unknown: Vec::new(),
        fetch_unknown: true,
    }))
}

/// mctiers `search_profile` is not used here because their username cache can be outdated
pub async fn search_profile(Path(name): Path<String>) -> RouteResponse<impl IntoResponse> {
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{name}");

    let response = crate::CLIENT.get(url).send().await?;
    if response.status() != StatusCode::OK {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let response: MojangUUID = response.json().await?;

    get_tier(Path(response.id))
        .await
        .map(IntoResponse::into_response)
}

pub async fn get_tierlists() -> RouteResponse<impl IntoResponse> {
    // of course i can't do this because mctiers blocks all tiertagger requests
    // Ok(Redirect::to("https://mctiers.com/api/tierlists"))

    let json: serde_json::Value = crate::CLIENT
        .get("https://mctiers.com/api/tierlists")
        .send()
        .await?
        .json()
        .await?;

    Ok(Json(json))
}

// === Utility functions ===

async fn fetch_tier(uuid: &Uuid) -> (Option<serde_json::Value>, StatusCode) {
    let url = format!("https://mctiers.com/api/profile/{}", uuid.as_simple());

    let start = Instant::now();
    let response = crate::CLIENT
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);
    let delta_time = start.elapsed().as_secs_f64();

    let response = match response {
        Ok(res) => {
            let status = res.status().as_u16().to_string();
            let labels = [("path", String::from("profile")), ("status", status)];

            metrics::counter!(MCTIERS_REQS_KEY, &labels).increment(1);
            metrics::histogram!(MCTIERS_REQ_DURATION_KEY, &labels).record(delta_time);

            res
        }
        Err(e) => {
            if e.status() != Some(StatusCode::NOT_FOUND) {
                tracing::warn!("Failed to fetch profile `{uuid}`: {e}");
            }

            return (
                None,
                e.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            );
        }
    };

    let status = response.status();

    match response.json::<serde_json::Value>().await {
        Ok(p) => (Some(p), status),
        Err(e) => {
            tracing::warn!("Failed to parse profile `{uuid}`: {e}");
            (None, status)
        }
    }
}
