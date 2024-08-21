use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::FromRedisValue;
use redis::{AsyncCommands, Client, ConnectionLike};
use redis_macros::ToRedisArgs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tiers::PlayerInfo;

const PROFILE_KEY: &str = "tiers-v1-profile";

#[derive(Debug)]
pub struct Storage {
    pool: Pool<RedisConnectionManager>,
}

impl Storage {
    pub async fn new_from_env() -> anyhow::Result<Self> {
        let url = std::env::var("REDIS_URL").context("REDIS_URL not set")?;
        let mut client = Client::open(url.clone())?;

        if client.check_connection() {
            let manager = RedisConnectionManager::new(url)?;
            let pool = Pool::builder().max_size(20).build(manager).await?;

            Ok(Self { pool })
        } else {
            anyhow::bail!("failed to connect to redis");
        }
    }

    // === PlayerInfo ===

    pub async fn has_player_info(&self, uuid: uuid::Uuid) -> Result<bool> {
        let key = format!("{PROFILE_KEY}:{uuid}");
        let mut con = self.pool.get().await?;

        con.exists(&key).await.map_err(anyhow::Error::from)
    }

    pub async fn get_player_info(&self, uuid: uuid::Uuid) -> Result<Option<PlayerInfo>> {
        let key = format!("{PROFILE_KEY}:{uuid}");
        let mut con = self.pool.get().await?;

        let player: OptionalPlayerInfo = con.get(&key).await?;

        Ok(player.into())
    }

    pub async fn set_player_info(
        &self,
        uuid: uuid::Uuid,
        player: Option<PlayerInfo>,
    ) -> Result<()> {
        let key = format!("{PROFILE_KEY}:{uuid}");
        let mut con = self.pool.get().await?;

        let player: OptionalPlayerInfo = player.into();

        redis::pipe()
            .set(&key, player)
            .expire(&key, 60 * 60 * 12)
            .query_async(&mut *con)
            .await
            .map_err(anyhow::Error::from)
    }

    pub async fn get_all_players(&self) -> anyhow::Result<HashMap<Uuid, Option<PlayerInfo>>> {
        let mut con = self.pool.get().await?;

        let keys: Vec<String> = con.keys(format!("{PROFILE_KEY}:*").as_str()).await?;

        let (uuids, keys) = keys
            .into_iter()
            .filter_map(|k| {
                let uuid = &k[PROFILE_KEY.len() + 1..];
                Uuid::parse_str(uuid).ok().map(|u| (u, k))
            })
            .collect::<(Vec<_>, Vec<_>)>();

        let values: Vec<OptionalPlayerInfo> = con.mget(&keys).await?;
        let values: Vec<Option<PlayerInfo>> = values.into_iter().map(Into::into).collect();

        Ok(uuids.into_iter().zip(values).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToRedisArgs)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OptionalPlayerInfo {
    Present(PlayerInfo),
    Unknown,
}

impl FromRedisValue for OptionalPlayerInfo {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        match *v {
            redis::Value::Nil => Ok(Self::Unknown),
            redis::Value::BulkString(ref bytes) => {
                if let Ok(s) = std::str::from_utf8(bytes) {
                    if let Ok(s) = serde_json::from_str(s) {
                        Ok(s)
                    } else {
                        redis_error(format!("Response type not deserializable with serde_json. (response was {v:?})"))
                    }
                } else {
                    redis_error(format!(
                        "Response was not valid UTF-8 string. (response was {v:?})"
                    ))
                }
            }
            _ => redis_error(format!(
                "Response type was not deserializable. (response was {v:?})"
            )),
        }
    }
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

fn redis_error(msg: String) -> redis::RedisResult<OptionalPlayerInfo> {
    Err(redis::RedisError::from((
        redis::ErrorKind::TypeError,
        "Response was of incompatible type",
        msg,
    )))
}
