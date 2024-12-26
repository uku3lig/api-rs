use anyhow::Result;
use axum::extract::Json;
use regex::Regex;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::Deserialize;
use serde_json::json;

use crate::RouteResponse;

#[derive(Deserialize)]
pub struct ProcessData {
    tweet_url: String,
    tweet_body: String,
    tweet_author: String,
    cobalt_url: String,
    cobalt_key: String,
    webhook_url: String,
    webhook_avatar: String,
}

// very, *very* minimal representation of a cobalt response, using only the stuff we need
#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum CobaltResponse {
    Error,
    Picker { picker: Vec<PickerObj> },
    Redirect,
    Tunnel,
}

#[derive(Debug, Deserialize)]
struct PickerObj {
    url: String,
}

pub async fn webhook(Json(data): Json<ProcessData>) -> RouteResponse<()> {
    tokio::spawn(async move {
        let mut message = format!("New tweet by {}: ", data.tweet_author);

        let cobalt = fetch_cobalt(&data.cobalt_url, &data.cobalt_key, &data.tweet_url).await;
        let cobalt = match cobalt {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Could not fetch cobalt data for {}: {e}", &data.tweet_url);
                return;
            }
        };

        message.push_str(&cobalt);
        message.push_str(&resolve_tco_urls(&data.tweet_body));

        let webhook_res = send_webhook(
            &data.webhook_url,
            &message,
            &data.tweet_author,
            &data.webhook_avatar,
        )
        .await;

        if let Err(e) = webhook_res {
            tracing::warn!("Could not send webhook message: {e}");
        }
    });

    Ok(())
}

/// fetch attachment urls, or replace link with fxtwitter if there are none
async fn fetch_cobalt(cobalt_url: &str, cobalt_key: &str, tweet_url: &str) -> Result<String> {
    let request = crate::CLIENT
        .post(cobalt_url)
        .json(&json!({ "url": tweet_url }))
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, format!("Api-Key {cobalt_key}"));

    let response = request
        .send()
        .await?
        .error_for_status()?
        .json::<CobaltResponse>()
        .await?;

    let out = match response {
        CobaltResponse::Picker { picker } => {
            let urls = picker
                .iter()
                .enumerate()
                .map(|(idx, p)| format!("[{}]({})", idx + 1, p.url))
                .collect::<Vec<_>>();

            format!("{tweet_url} (attachments: {})", urls.join(" "))
        }
        _ => tweet_url.replace("x.com", "fxtwitter.com"),
    };

    Ok(out)
}

fn resolve_tco_urls(tweet_body: &str) -> String {
    let tco_regex = Regex::new(r"https://t\.co/\S+").unwrap();
    let tco_urls = tco_regex
        .captures_iter(tweet_body)
        .map(|c| c.extract())
        .map(|(url, [])| url)
        .enumerate()
        .map(|(idx, url)| format!("[{}]({url})", idx + 1))
        .collect::<Vec<_>>();

    if tco_urls.is_empty() {
        String::new()
    } else {
        format!(" (urls: {})", tco_urls.join(" "))
    }
}

async fn send_webhook(
    webhook_url: &str,
    message: &str,
    webhook_name: &str,
    webhook_avatar: &str,
) -> Result<()> {
    let params = json!({
        "content": message,
        "username": webhook_name,
        "avatar_url": webhook_avatar
    });

    crate::CLIENT
        .post(webhook_url)
        .json(&params)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
