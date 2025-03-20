use std::sync::Arc;

use axum::{Json, extract::State};
use bb8_redis::redis::{self, AsyncCommands};
use serde_json::Value;

use crate::{AppState, CLIENT, RouteResponse, util::AppError};

const REDIS_KEY: &str = "now_listening";

pub async fn now_playing(State(state): State<Arc<AppState>>) -> RouteResponse<Json<Value>> {
    let mut conn = state.cache.pool.get().await?;

    if conn.exists(REDIS_KEY).await? {
        let cached: String = conn.get(REDIS_KEY).await?;
        Ok(Json(serde_json::from_str(&cached)?))
    } else {
        let url = format!(
            "https://ws.audioscrobbler.com/2.0/?method=user.getrecenttracks&api_key={}&user=ukute&limit=1&format=json",
            state.config.lastfm_key
        );

        let res = CLIENT.get(url).send().await?;
        if res.status() != 200 {
            return Err(AppError::StatusCode(res.status(), String::new()));
        }

        let json: Value = res.json().await?;

        redis::pipe()
            .set(REDIS_KEY, serde_json::to_string(&json)?)
            .expire(REDIS_KEY, 15)
            .query_async::<()>(&mut *conn)
            .await?;

        Ok(Json(json))
    }
}
