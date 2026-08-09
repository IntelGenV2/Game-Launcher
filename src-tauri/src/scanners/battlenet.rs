use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Known Battle.net product codes -> display names
fn known_products() -> Vec<(&'static str, &'static str)> {
    vec![
        ("wow", "World of Warcraft"),
        ("wow_classic", "World of Warcraft Classic"),
        ("d3", "Diablo III"),
        ("osi", "Diablo II: Resurrected"),
        ("fenris", "Diablo IV"),
        ("hsb", "Hearthstone"),
        ("hero", "Heroes of the Storm"),
        ("prometheus", "Overwatch 2"),
        ("s2", "StarCraft II"),
        ("w3", "Warcraft III: Reforged"),
        ("destiny2", "Destiny 2"),
        ("anbs", "Call of Duty"),
        ("viper", "Call of Duty: Black Ops 6"),
        ("zeus", "Call of Duty: Modern Warfare III"),
        ("odin", "Call of Duty: Modern Warfare II"),
        ("aqua", "Call of Duty: Black Ops Cold War"),
        ("lazr", "Call of Duty: Vanguard"),
        ("rtro", "Arcade Collection"),
    ]
}

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Parse Battle.net.config for install paths
    if let Some(config) = battle_net_config() {
        if let Ok(text) = fs::read_to_string(&config) {
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                if let Some(games_obj) = json.pointer("/Games") {
                    if let Some(map) = games_obj.as_object() {
                        for (code, entry) in map {
                            if code == "battle_net" {
                                continue;
                            }
                            let path = entry
                                .get("Path")
                                .or_else(|| entry.get("InstallPath"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let name = known_products()
                                .iter()
                                .find(|(c, _)| c == &code.as_str())
                                .map(|(_, n)| n.to_string())
                                .unwrap_or_else(|| humanize(code));
                            if seen.insert(code.clone()) {
                                games.push(make_game(code, &name, path));
                            }
                        }
                    }
                }
            }
        }
    }

    // Also scan common install directories for product folders
    for dir in common_dirs() {
        if !dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().to_string();
            for (code, name) in known_products() {
                if folder.eq_ignore_ascii_case(name)
                    || folder.to_lowercase().contains(&code.replace('_', ""))
                {
                    if seen.insert(code.to_string()) {
                        games.push(make_game(
                            code,
                            name,
                            Some(path.to_string_lossy().to_string()),
                        ));
                    }
                }
            }
            // Detect .product.db sibling indicator
            if path.join(".product.db").exists()
                || dir.join(format!("{folder}/.product.db")).exists()
            {
                let code = folder.to_lowercase().replace(' ', "_");
                if seen.insert(code.clone()) {
                    let name = known_products()
                        .iter()
                        .find(|(c, _)| *c == code)
                        .map(|(_, n)| n.to_string())
                        .unwrap_or(folder);
                    games.push(make_game(
                        &code,
                        &name,
                        Some(path.to_string_lossy().to_string()),
                    ));
                }
            }
        }
    }

    Ok(games)
}

fn make_game(code: &str, name: &str, install_path: Option<String>) -> DiscoveredGame {
    DiscoveredGame {
        id: format!("battlenet:{code}"),
        name: name.to_string(),
        store: Store::Battlenet,
        launch_target: format!("battlenet://{code}/"),
        install_path,
        steam_app_id: None,
        playtime_minutes: None,
    }
}

fn humanize(code: &str) -> String {
    code.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn battle_net_config() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("Battle.net").join("Battle.net.config"))
}

fn common_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for base in [
        r"C:\Program Files (x86)\Battle.net",
        r"C:\Program Files\Battle.net",
        r"C:\Program Files (x86)\Games",
        r"C:\Games",
        r"D:\Games",
        r"D:\Battle.net",
    ] {
        dirs.push(PathBuf::from(base));
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("Games"));
    }
    // Parent of Battle.net.exe install locations from Agent
    let agent = Path::new(r"C:\ProgramData\Battle.net\Agent");
    if agent.exists() {
        dirs.push(agent.to_path_buf());
    }
    dirs
}
