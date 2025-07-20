use anyhow::Result;
use axum::extract::Json;
use regex::Regex;
use reqwest::{
    Client,
    header::{ACCEPT, AUTHORIZATION, LOCATION},
    redirect::Policy,
};
use serde::Deserialize;
use serde_json::json;

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
    Redirect { url: String },
    Tunnel { url: String },
}

#[derive(Debug, Deserialize)]
struct PickerObj {
    url: String,
}

struct Webhook {
    url: String,
    name: String,
    avatar_url: String,
}

impl Webhook {
    async fn send(&self, message: &str) {
        let params = json!({
            "content": message,
            "username": self.name,
            "avatar_url": self.avatar_url
        });

        let res = crate::CLIENT
            .post(&self.url)
            .json(&params)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);

        if let Err(e) = res {
            tracing::warn!("Could not send webhook message: {e}");
        }
    }
}

pub async fn webhook(Json(data): Json<ProcessData>) {
    tokio::spawn(async move {
        let mut message = format!("New tweet by {}: {}", data.tweet_author, data.tweet_url);

        let webhook = Webhook {
            url: data.webhook_url,
            name: data.tweet_author,
            avatar_url: data.webhook_avatar,
        };

        let tco_urls = match resolve_tco_urls(&data.tweet_body).await {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Could not fetch t.co urls for {}: {e}", &data.tweet_url);
                return;
            }
        };

        message.push_str(&tco_urls);
        webhook.send(&message).await;

        let attachments = fetch_cobalt(&data.cobalt_url, &data.cobalt_key, &data.tweet_url).await;
        match attachments {
            Ok(Some(attachments)) => webhook.send(&attachments).await,
            Ok(None) => {}
            Err(e) => tracing::warn!("Could not fetch cobalt data for {}: {e}", &data.tweet_url),
        }
    });
}

/// fetch attachment urls
async fn fetch_cobalt(
    cobalt_url: &str,
    cobalt_key: &str,
    tweet_url: &str,
) -> Result<Option<String>> {
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

    let urls = match response {
        CobaltResponse::Redirect { url } | CobaltResponse::Tunnel { url } => vec![url],
        CobaltResponse::Picker { picker } => picker.into_iter().map(|p| p.url).collect(),
        CobaltResponse::Error => return Ok(None),
    };

    let formatted = urls
        .into_iter()
        .enumerate()
        .map(|(idx, url)| format!("[{}]({url})", idx + 1))
        .collect::<Vec<_>>();

    Ok(Some(format!("attachments: {}", formatted.join(" "))))
}

async fn resolve_tco_urls(tweet_body: &str) -> Result<String> {
    let tco_regex = Regex::new(r"https://t\.co/\S+").unwrap();
    let tco_client = Client::builder().redirect(Policy::none()).build().unwrap();

    let tco_urls = tco_regex
        .captures_iter(tweet_body)
        .map(|c| c.extract())
        .map(|(url, [])| url);

    let mut results = Vec::new();
    for (idx, url) in tco_urls.enumerate() {
        let response = tco_client.get(url).send().await?;

        if let Some(r_url) = response.headers().get(LOCATION) {
            let value = r_url.to_str()?;
            results.push(format!("[{}]({value})", idx + 1));
        }
    }

    if results.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!(" (urls: {})", results.join(" ")))
    }
}
