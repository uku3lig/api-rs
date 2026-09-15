use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};

use crate::{AppState, RouteResponse, config::EnvCfg, util::IntoAppError};

const VERIF_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const DISCORD_API: &str = "https://discord.com/api/v10";

#[derive(Deserialize)]
pub struct TurnstileData {
    token: String,
}

#[derive(Deserialize)]
pub struct TurnstileResponse {
    success: bool,
    #[serde(rename = "error-codes")]
    error_codes: Vec<String>,
}

#[derive(Deserialize)]
struct DiscordUser {
    username: String,
    discriminator: String,
}

#[derive(Serialize)]
struct DiscordCreateInvite {
    max_age: usize,
    max_uses: usize,
}

#[derive(Deserialize)]
struct DiscordInvite {
    code: String,
}

pub async fn init_bot(config: &EnvCfg) -> anyhow::Result<()> {
    let user = crate::CLIENT
        .get(format!("{DISCORD_API}/users/@me"))
        .header("Authorization", format!("Bot {}", config.bot_token))
        .send()
        .await?
        .error_for_status()?
        .json::<DiscordUser>()
        .await?;

    tracing::info!(
        "successfully logged in to discord bot {}#{}!",
        user.username,
        user.discriminator
    );

    Ok(())
}

pub async fn generate_invite(
    Query(data): Query<TurnstileData>,
    State(state): State<Arc<AppState>>,
) -> RouteResponse<impl IntoResponse> {
    let body = [
        ("secret", &state.config.turnstile_secret),
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

    let invite_url = format!("{DISCORD_API}/channels/{}/invites", state.config.channel_id);
    let invite_body = DiscordCreateInvite {
        max_uses: 1,
        max_age: 3600,
    };

    let invite = crate::CLIENT
        .post(invite_url)
        .header("Authorization", format!("Bot {}", state.config.bot_token))
        .json(&invite_body)
        .send()
        .await?
        .error_for_status()?
        .json::<DiscordInvite>()
        .await?;

    let link = format!("https://discord.com/invite/{}", invite.code);

    Ok(Redirect::to(link.as_str()))
}
