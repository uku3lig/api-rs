use std::collections::HashMap;

use anyhow::Context;
use redis::{AsyncCommands, Client, ConnectionLike};
use redis_macros::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tiers::PlayerInfo;

const PROFILE_KEY: &str = "tiers-v1-profile";

type Result<T> = std::result::Result<T, redis::RedisError>;

pub struct Storage {
    client: Client,
}

impl Storage {
    pub fn new_from_env() -> anyhow::Result<Self> {
        let url = std::env::var("REDIS_URL").context("REDIS_URL not set")?;
        let mut client = redis::Client::open(url)?;

        if client.check_connection() {
            Ok(Self { client })
        } else {
            anyhow::bail!("failed to connect to redis");
        }
    }

    // === PlayerInfo ===

    pub async fn has_player_info(&self, uuid: uuid::Uuid) -> Result<bool> {
        let key = format!("{PROFILE_KEY}:{uuid}");
        let mut con = self.client.get_multiplexed_async_connection().await?;

        con.exists(&key).await
    }

    pub async fn get_player_info(&self, uuid: uuid::Uuid) -> Result<Option<PlayerInfo>> {
        let key = format!("{PROFILE_KEY}:{uuid}");
        let mut con = self.client.get_multiplexed_async_connection().await?;

        let player: OptionalPlayerInfo = con.get(&key).await?;

        Ok(player.into())
    }

    pub async fn set_player_info(
        &self,
        uuid: uuid::Uuid,
        player: Option<PlayerInfo>,
    ) -> Result<()> {
        let key = format!("{PROFILE_KEY}:{uuid}");
        let mut con = self.client.get_multiplexed_async_connection().await?;

        let player: OptionalPlayerInfo = player.into();

        redis::pipe()
            .set(&key, player)
            .expire(&key, 60 * 60 * 12)
            .query_async(&mut con)
            .await
    }

    pub async fn get_all_players(&self) -> anyhow::Result<HashMap<Uuid, Option<PlayerInfo>>> {
        let mut con = self.client.get_multiplexed_async_connection().await?;

        let keys: Vec<String> = con.keys(format!("{PROFILE_KEY}:*").as_str()).await?;
        let mut players = HashMap::new();

        for key in keys {
            let uuid = &key[PROFILE_KEY.len() + 1..];

            if let Ok(uuid) = Uuid::parse_str(uuid) {
                let player: OptionalPlayerInfo = con.get(&key).await?;
                players.insert(uuid, player.into());
            } else {
                tracing::warn!("invalid key found: {key}");
            }
        }

        Ok(players)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRedisValue, ToRedisArgs)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OptionalPlayerInfo {
    Present(PlayerInfo),
    Unknown,
}

impl From<Option<PlayerInfo>> for OptionalPlayerInfo {
    fn from(player: Option<PlayerInfo>) -> Self {
        match player {
            Some(player) => Self::Present(player),
            None => Self::Unknown,
        }
    }
}

impl From<OptionalPlayerInfo> for Option<PlayerInfo> {
    fn from(val: OptionalPlayerInfo) -> Self {
        match val {
            OptionalPlayerInfo::Present(player) => Some(player),
            OptionalPlayerInfo::Unknown => None,
        }
    }
}
