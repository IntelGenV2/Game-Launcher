use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Playnite HumbleLibrary.GetInstalledGames:
/// `%APPDATA%\Humble App\config.json` → `game-collection-4`
/// where status is `downloaded` or `installed`.
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let Some(roaming) = dirs::config_dir() else {
        return Ok(games);
    };
    let config = roaming.join("Humble App").join("config.json");
    if !config.is_file() {
        return Ok(games);
    }
    let Ok(text) = fs::read_to_string(&config) else {
        return Ok(games);
    };
    let parsed: HumbleAppConfig = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Humble App config: {e:#}");
            return Ok(games);
        }
    };

    let mut seen = std::collections::HashSet::new();
    for entry in parsed.game_collection_4 {
        if entry.status != "downloaded" && entry.status != "installed" {
            continue;
        }
        let exe = resolve_exe(&entry);
        if exe.is_empty() || !Path::new(&exe).is_file() {
            continue;
        }
        let install = Path::new(&exe)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(&exe));
        if !install.is_dir() {
            continue;
        }
        let id = if entry.machine_name.is_empty() {
            continue;
        } else {
            entry.machine_name.clone()
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let name = if entry.game_name.is_empty() {
            id.clone()
        } else {
            entry.game_name
        };
        games.push(DiscoveredGame {
            id: format!("humble:{id}"),
            name,
            store: Store::Humble,
            launch_target: exe,
            install_path: Some(install.to_string_lossy().to_string()),
            steam_app_id: None,
            playtime_minutes: None,
        });
    }

    Ok(games)
}

fn resolve_exe(entry: &GameCollection4) -> String {
    // Playnite: filePath + executablePath, else older layout downloadFilePath/machineName/executablePath
    if !entry.file_path.is_empty() && !entry.executable_path.is_empty() {
        return PathBuf::from(&entry.file_path)
            .join(&entry.executable_path)
            .to_string_lossy()
            .replace('/', "\\");
    }
    if !entry.download_file_path.is_empty()
        && !entry.machine_name.is_empty()
        && !entry.executable_path.is_empty()
    {
        return PathBuf::from(&entry.download_file_path)
            .join(&entry.machine_name)
            .join(&entry.executable_path)
            .to_string_lossy()
            .replace('/', "\\");
    }
    String::new()
}

#[derive(Debug, Deserialize)]
struct HumbleAppConfig {
    #[serde(rename = "game-collection-4", default)]
    game_collection_4: Vec<GameCollection4>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GameCollection4 {
    #[serde(default)]
    machine_name: String,
    #[serde(default)]
    game_name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    file_path: String,
    #[serde(default)]
    executable_path: String,
    #[serde(default)]
    download_file_path: String,
}
