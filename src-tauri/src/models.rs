use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Store {
    Steam,
    Epic,
    Gog,
    Xbox,
    Battlenet,
    Ubisoft,
    Ea,
    Roblox,
    Manual,
}

impl Store {
    pub fn as_str(&self) -> &'static str {
        match self {
            Store::Steam => "steam",
            Store::Epic => "epic",
            Store::Gog => "gog",
            Store::Xbox => "xbox",
            Store::Battlenet => "battlenet",
            Store::Ubisoft => "ubisoft",
            Store::Ea => "ea",
            Store::Roblox => "roblox",
            Store::Manual => "manual",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "steam" => Some(Store::Steam),
            "epic" => Some(Store::Epic),
            "gog" => Some(Store::Gog),
            "xbox" => Some(Store::Xbox),
            "battlenet" => Some(Store::Battlenet),
            "ubisoft" => Some(Store::Ubisoft),
            "ea" => Some(Store::Ea),
            "roblox" => Some(Store::Roblox),
            "manual" => Some(Store::Manual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub store: Store,
    pub launch_target: String,
    pub install_path: Option<String>,
    pub cover_url: Option<String>,
    pub cover_path: Option<String>,
    pub favorite: bool,
    pub hidden: bool,
    pub missing: bool,
    pub playtime_minutes: i64,
    pub last_played_at: Option<String>,
    pub date_added: String,
    pub steam_app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredGame {
    pub id: String,
    pub name: String,
    pub store: Store,
    pub launch_target: String,
    pub install_path: Option<String>,
    pub steam_app_id: Option<String>,
    pub playtime_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub steam_grid_db_api_key: Option<String>,
    pub sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub total: usize,
    pub favorites: usize,
    pub missing: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySession {
    pub id: i64,
    pub game_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_minutes: i64,
    pub avg_fps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsSample {
    pub id: i64,
    pub game_id: String,
    pub recorded_at: String,
    pub fps: f64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlaytime {
    pub day: String,
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStats {
    pub game_id: String,
    pub total_playtime_minutes: i64,
    pub session_count: i64,
    pub avg_session_minutes: f64,
    pub last_played_at: Option<String>,
    pub first_played_at: Option<String>,
    pub avg_fps: Option<f64>,
    pub latest_fps: Option<f64>,
    pub daily_playtime: Vec<DailyPlaytime>,
    pub sessions: Vec<PlaySession>,
    pub fps_samples: Vec<FpsSample>,
}
