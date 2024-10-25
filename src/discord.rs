use std::sync::{Arc, OnceLock};

use anyhow::Context;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::{extract::Query, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serenity::all::{CreateInvite, Http};

use crate::config::EnvCfg;
use crate::{util::IntoAppError, RouteResponse};

const VERIF_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
static SERENITY_HTTP: OnceLock<Http> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
pub struct TurnstileData {
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TurnstileResponse {
    success: bool,
    #[serde(rename = "error-codes")]
    error_codes: Vec<String>,
}

pub async fn init_bot(config: &EnvCfg) -> anyhow::Result<()> {
    let http = Http::new(&config.bot_token);

    let user = http.get_current_user().await?;
    SERENITY_HTTP.set(http).unwrap();

    tracing::info!("successfully logged in to discord bot {}!", user.name);

    Ok(())
}

pub async fn generate_invite(
    Query(data): Query<TurnstileData>,
    State(config): State<Arc<EnvCfg>>,
) -> RouteResponse<impl IntoResponse> {
    let http = SERENITY_HTTP.get().context("bot token not set")?;

    let body = [
        ("secret", &config.turnstile_secret),
        ("response", &data.token),
    ];
    let request = crate::CLIENT.post(VERIF_URL).form(&body).build()?;

    let response: TurnstileResponse = crate::CLIENT
        .execute(request)
        .await?
        .error_for_status()?
        .json()
        .await?;

    if !response.success {
        let message = format!("invalid token: {:?}", response.error_codes);
        return (StatusCode::BAD_REQUEST, message.as_str()).into_app_err();
    }

    let invite = config
        .channel_id
        .create_invite(http, CreateInvite::new().max_uses(1))
        .await?;

    let link = format!("https://discord.com/invite/{}", invite.code);

    Ok(Redirect::to(link.as_str()))
}
