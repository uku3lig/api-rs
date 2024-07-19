use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::RouteResponse;

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

pub fn router() -> Router {
    let router = Router::new()
        .route("/:name", get(downloads))
        .route("/:name/shields", get(downloads_shields));

    Router::new().nest("/downloads", router)
}

async fn downloads(Path(name): Path<String>) -> RouteResponse<String> {
    let request = crate::CLIENT
        .get(format!("https://api.modrinth.com/v2/user/{name}/projects"))
        .build()?;

    let response: Vec<ModrinthProject> = crate::CLIENT
        .execute(request)
        .await?
        .error_for_status()?
        .json()
        .await?;

    let sum: usize = response.iter().map(|p| p.downloads).sum();

    Ok(format!("{sum}"))
}

async fn downloads_shields(Path(name): Path<String>) -> RouteResponse<Json<ShieldsBadge>> {
    let count: u32 = downloads(Path(name)).await?.parse().unwrap();
    let formatted = format_number(count);

    let shield = ShieldsBadge {
        schema_version: 1,
        label: "downloads".into(),
        message: formatted,
        color: "brightgreen".into(),
        named_logo: "modrinth".into(),
    };

    Ok(Json(shield))
}

const UNITS: &[&str] = &["", "k", "M", "B", "T"];

fn format_number(n: u32) -> String {
    let mut n = f64::from(n);

    for unit in UNITS {
        if n < 1000.0 {
            return format!("{n:.1}{unit}");
        }

        n /= 1000.0;
    }

    let last = UNITS.last().unwrap();
    format!("{n:.1}{last}")
}
