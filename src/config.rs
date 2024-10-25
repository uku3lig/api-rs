use serde::Deserialize;
use serenity::all::ChannelId;

#[derive(Debug, Deserialize)]
pub struct EnvCfg {
    #[serde(default = "default_addr")]
    pub socket_addr: String,
    #[serde(default = "default_metrics_addr")]
    pub metrics_socket_addr: String,
    pub redis_url: String,
    pub turnstile_secret: String,
    pub channel_id: ChannelId,
    pub bot_token: String,
}

fn default_addr() -> String {
    "0.0.0.0:5000".into()
}

fn default_metrics_addr() -> String {
    "127.0.0.1:5001".into()
}
