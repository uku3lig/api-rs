use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthProject {
    pub slug: String,
    pub downloads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShieldsBadge {
    pub schema_version: u8,
    pub label: String,
    pub message: String,
    pub color: String,
    pub named_logo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierList {
    pub rankings: Vec<Vec<(String, u8)>>,
    pub players: HashMap<String, PlayerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub name: String,
    pub region: String,
    pub points: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredPlayerInfo {
    pub name: String,
    pub region: String,
    pub points: u64,
    pub tier: u8,
    pub high: bool,
}

impl TierList {
    pub fn get_formatted_rankings(&self) -> Vec<HashMap<String, bool>> {
        self.rankings
            .iter()
            .map(|v| {
                let mut map = HashMap::new();
                for (name, low) in v {
                    // low == 0 means high tier
                    map.insert(name.clone(), *low == 0);
                }
                map
            })
            .collect()
    }

    pub fn into_tiered_info(self) -> Vec<TieredPlayerInfo> {
        let rankings = self.get_formatted_rankings();
        let mut tiered = Vec::new();

        for (i, ranks) in rankings.iter().enumerate() {
            for (uuid, high) in ranks {
                if let Some(info) = self.players.get(uuid) {
                    tiered.push(TieredPlayerInfo {
                        name: info.name.clone(),
                        region: info.region.clone(),
                        points: info.points,
                        tier: (i + 1) as u8,
                        high: *high,
                    });
                }
            }
        }

        tiered
    }
}
