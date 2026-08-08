use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    for root in [
        r"SOFTWARE\WOW6432Node\GOG.com\Games",
        r"SOFTWARE\GOG.com\Games",
    ] {
        let key = match hklm.open_subkey(root) {
            Ok(k) => k,
            Err(_) => continue,
        };
        for game_id in key.enum_keys().filter_map(|k| k.ok()) {
            let sub = match key.open_subkey(&game_id) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let name: String = sub
                .get_value("gameName")
                .or_else(|_| sub.get_value("GAMENAME"))
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let path: String = sub
                .get_value("path")
                .or_else(|_| sub.get_value("PATH"))
                .unwrap_or_default();
            let exe: String = sub
                .get_value("exe")
                .or_else(|_| sub.get_value("EXE"))
                .unwrap_or_default();
            let launch_target = if !exe.is_empty() && PathBuf::from(&exe).exists() {
                exe
            } else {
                format!("goggalaxy://openGameView/{game_id}")
            };

            games.push(DiscoveredGame {
                id: format!("gog:{game_id}"),
                name,
                store: Store::Gog,
                launch_target,
                install_path: if path.is_empty() { None } else { Some(path) },
                steam_app_id: None,
                playtime_minutes: None,
            });
        }
    }
    Ok(games)
}
