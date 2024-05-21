use std::env;

use anyhow::Context;
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::{extract::Query, response::IntoResponse};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serenity::all::{ChannelId, CreateInvite, Http};

use crate::{util::IntoAppError, RouteResponse};

const VERIF_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
static TURNSTILE_SECRET: OnceCell<String> = OnceCell::new();
static SERENITY_HTTP: OnceCell<Http> = OnceCell::new();
static CHANNEL_ID: OnceCell<ChannelId> = OnceCell::new();

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

pub async fn init_bot() -> anyhow::Result<()> {
    let turnstile_secret = env::var("TURNSTILE_SECRET").expect("TURNSTILE_SECRET not set");

    let channel_id = env::var("CHANNEL_ID").expect("CHANNEL_ID not set");
    let channel_id = ChannelId::new(channel_id.parse()?);

    let token = env::var("BOT_TOKEN").expect("BOT_TOKEN not set");
    let http = Http::new(token.as_str());

    let user = http.get_current_user().await?;

    TURNSTILE_SECRET.set(turnstile_secret).unwrap();
    CHANNEL_ID.set(channel_id).unwrap();
    SERENITY_HTTP.set(http).unwrap();

    tracing::info!("successfully logged in to discord bot {}!", user.name);

    Ok(())  
}

pub async fn generate_invite(
    Query(data): Query<TurnstileData>,
) -> RouteResponse<impl IntoResponse> {
    let secret = TURNSTILE_SECRET.get().context("turnstile secret not set")?;
    let http = SERENITY_HTTP.get().context("bot token not set")?;
    let channel_id = CHANNEL_ID.get().context("channel id not set")?;

    let body = [("secret", secret), ("response", &data.token)];
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

    let invite = channel_id
        .create_invite(http, CreateInvite::new().max_uses(1))
        .await?;

    let link = format!("https://discord.com/invite/{}", invite.code);

    Ok(Redirect::to(link.as_str()))
}
