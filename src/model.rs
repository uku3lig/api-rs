use serde::{Deserialize, Serialize};

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
