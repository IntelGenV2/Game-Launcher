use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let manifests = PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests");
    if !manifests.is_dir() {
        return Ok(games);
    }

    for entry in fs::read_dir(manifests)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let json: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let display_name = json
            .get("DisplayName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if display_name.is_empty() {
            continue;
        }
        // Skip launcher / engine entries
        let lower = display_name.to_lowercase();
        if lower.contains("unreal engine") || lower.contains("epic games launcher") {
            continue;
        }

        let app_name = json
            .get("AppName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let catalog_id = json
            .get("CatalogItemId")
            .and_then(|v| v.as_str())
            .unwrap_or(&app_name)
            .to_string();
        let install_location = json
            .get("InstallLocation")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let launch_exe = json
            .get("LaunchExecutable")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let launch_target = if !app_name.is_empty() {
            format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", app_name)
        } else if let (Some(loc), Some(exe)) = (&install_location, &launch_exe) {
            PathBuf::from(loc).join(exe).to_string_lossy().to_string()
        } else {
            continue;
        };

        let id = format!("epic:{}", if catalog_id.is_empty() { &app_name } else { &catalog_id });
        games.push(DiscoveredGame {
            id,
            name: display_name,
            store: Store::Epic,
            launch_target,
            install_path: install_location,
            steam_app_id: None,
            playtime_minutes: None,
        });
    }
    Ok(games)
}
