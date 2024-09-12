use anyhow::{anyhow, Result};
use redis::{Commands, ConnectionLike};
use redis_macros::FromRedisValue;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    cache::{expire_time, EXPIRATION_SECONDS, PROFILE_KEY_V2, UNKNOWN_SET_KEY},
    tiers::PlayerInfo,
};

const PROFILE_KEY_V1: &str = "tiers-v1-profile";

pub fn start_migration() -> Result<()> {
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not set");
    let mut client = redis::Client::open(redis_url)?;

    if client.check_connection() {
        tracing::info!("connected to redis!");
    } else {
        return Err(anyhow!("could not connect to redis"));
    }

    tiers_v1_to_v2(client.get_connection()?)?;

    Ok(())
}

#[derive(Debug, Clone, Deserialize, FromRedisValue)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OptionalPlayerInfo {
    Present(PlayerInfo),
    Unknown,
}

fn tiers_v1_to_v2(mut con: redis::Connection) -> Result<()> {
    let v1_keys: Vec<String> = con.keys(format!("{PROFILE_KEY_V1}:*"))?;
    tracing::info!("found {} v1 tier keys", v1_keys.len());

    if v1_keys.is_empty() {
        tracing::warn!("nothing to migrate in v1 tiers!");
        return Ok(());
    }

    let uuids: Vec<Uuid> = v1_keys
        .iter()
        .filter_map(|k| {
            let uuid = &k[PROFILE_KEY_V1.len() + 1..];
            Uuid::parse_str(uuid).ok()
        })
        .collect();

    let v1_players: Vec<OptionalPlayerInfo> = con.mget(v1_keys)?;

    let mut v1_known = vec![];
    let mut v1_unknown = vec![];

    for (uuid, opt) in uuids.into_iter().zip(v1_players) {
        match opt {
            OptionalPlayerInfo::Present(info) => v1_known.push(info),
            OptionalPlayerInfo::Unknown => v1_unknown.push(uuid),
        }
    }

    tracing::info!(
        "got {} known and {} unknown",
        v1_known.len(),
        v1_unknown.len()
    );

    // setting new values

    let mut pipe = redis::pipe();

    for player in v1_known {
        let key = format!("{PROFILE_KEY_V2}:{}", player.uuid);

        pipe.set(&key, player)
            .expire(&key, EXPIRATION_SECONDS.into());
    }

    for uuid in v1_unknown {
        pipe.zadd(UNKNOWN_SET_KEY, uuid.to_string(), expire_time());
    }

    pipe.exec(&mut con)?;

    Ok(())
}
