use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::{AsyncCommands, Client, ConnectionLike};

use crate::tiers::PlayerInfo;

pub const PROFILE_KEY_V2: &str = "tiers-v2-profile";
pub const UNKNOWN_SET_KEY: &str = "tiers-v2:unknown-set";
pub const EXPIRATION_SECONDS: u32 = 60 * 60 * 12;

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
        let key = format!("{PROFILE_KEY_V2}:{uuid}");
        let mut con = self.pool.get().await?;

        con.exists(&key).await.map_err(anyhow::Error::from)
    }

    pub async fn get_player_info(&self, uuid: uuid::Uuid) -> Result<Option<PlayerInfo>> {
        let key = format!("{PROFILE_KEY_V2}:{uuid}");

        let mut con = self.pool.get().await?;

        if con.exists(&key).await? {
            Ok(Some(con.get(&key).await?))
        } else {
            let score: Option<f64> = con.zscore(UNKNOWN_SET_KEY, &key).await?;
            if score.is_none() {
                con.zadd(UNKNOWN_SET_KEY, uuid.to_string(), expire_time())
                    .await?;
            }

            Ok(None)
        }
    }

    pub async fn set_player_info(
        &self,
        uuid: uuid::Uuid,
        opt_player: Option<PlayerInfo>,
    ) -> Result<()> {
        let key = format!("{PROFILE_KEY_V2}:{uuid}");

        let mut con = self.pool.get().await?;

        match opt_player {
            Some(player) => {
                redis::pipe()
                    .set(&key, player)
                    .expire(&key, EXPIRATION_SECONDS.into())
                    .query_async(&mut *con)
                    .await?;
            }
            None => {
                con.zadd(UNKNOWN_SET_KEY, uuid.to_string(), expire_time())
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn get_all_players(&self) -> anyhow::Result<(Vec<PlayerInfo>, Vec<String>)> {
        let mut con = self.pool.get().await?;

        let keys: Vec<String> = {
            let mut keys = Vec::new();
            let mut iter = con
                .scan_match(format!("{PROFILE_KEY_V2}:*").as_str())
                .await?;

            while let Some(e) = iter.next_item().await {
                keys.push(e);
            }

            keys
        };

        if keys.is_empty() {
            return Ok((vec![], vec![]));
        }

        let values: Vec<PlayerInfo> = con.mget(&keys).await?;

        // ZRANGE with BYSCORE is not implement in redis-rs yet
        // see redis-rs/redis-rs#586
        let unknown: Vec<String> = redis::cmd("ZRANGE")
            .arg(UNKNOWN_SET_KEY)
            .arg(now())
            .arg("+inf")
            .arg("BYSCORE")
            .query_async(&mut *con)
            .await?;

        // remove expired entries
        con.zrembyscore(UNKNOWN_SET_KEY, "-inf", now()).await?;

        Ok((values, unknown))
    }
}

pub fn expire_time() -> u64 {
    let end = SystemTime::now() + Duration::from_secs(EXPIRATION_SECONDS.into());

    match end.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(e) => {
            // this should never happen, but in case something really fucking bad happens i'd rather have it log than panic
            tracing::error!("time went backwards: {e}");
            0
        }
    }
}

fn now() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(e) => {
            // this should never happen, but in case something really fucking bad happens i'd rather have it log than panic
            tracing::error!("time went backwards: {e}");
            0
        }
    }
}
