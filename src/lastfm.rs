use std::sync::Arc;

use axum::{body::Body, extract::State, http::Response, response::IntoResponse};

use crate::{AppState, CLIENT, RouteResponse};

pub async fn now_playing(State(state): State<Arc<AppState>>) -> RouteResponse<impl IntoResponse> {
    let url = format!(
        "https://ws.audioscrobbler.com/2.0/?method=user.getrecenttracks&api_key={}&user=ukute&limit=1&format=json",
        state.config.lastfm_key
    );

    let res = CLIENT.get(url).send().await?;

    let mut response_builder = Response::builder().status(res.status());
    *response_builder.headers_mut().unwrap() = res.headers().clone();
    let out_res = response_builder.body(Body::from_stream(res.bytes_stream()))?;

    Ok(out_res)
}
