use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Discover Roblox Player from LocalAppData Versions folder.
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let Some(local) = dirs::data_local_dir() else {
        return Ok(games);
    };
    let versions = local.join("Roblox").join("Versions");
    if !versions.is_dir() {
        return Ok(games);
    }

    let mut player: Option<PathBuf> = None;
    let mut newest: Option<std::time::SystemTime> = None;

    for entry in fs::read_dir(&versions)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        for exe_name in ["RobloxPlayerBeta.exe", "RobloxPlayer.exe"] {
            let exe = path.join(exe_name);
            if exe.exists() {
                let modified = fs::metadata(&exe)
                    .and_then(|m| m.modified())
                    .ok();
                if newest.is_none() || modified > newest {
                    newest = modified;
                    player = Some(exe);
                }
            }
        }
    }

    if let Some(exe) = player {
        games.push(DiscoveredGame {
            id: "roblox:player".into(),
            name: "Roblox".into(),
            store: Store::Roblox,
            launch_target: exe.to_string_lossy().to_string(),
            install_path: Some(versions.to_string_lossy().to_string()),
            steam_app_id: None,
            playtime_minutes: None,
        });
    }

    Ok(games)
}
