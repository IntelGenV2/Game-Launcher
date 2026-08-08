use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use winreg::enums::*;
use winreg::RegKey;

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    for root in [
        r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs",
        r"SOFTWARE\Ubisoft\Launcher\Installs",
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
            let install_dir: String = sub.get_value("InstallDir").unwrap_or_default();
            if install_dir.is_empty() {
                continue;
            }
            let name = std::path::Path::new(&install_dir)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("Ubisoft Game {game_id}"));

            games.push(DiscoveredGame {
                id: format!("ubisoft:{game_id}"),
                name,
                store: Store::Ubisoft,
                launch_target: format!("uplay://launch/{game_id}/0"),
                install_path: Some(install_dir),
                steam_app_id: None,
                playtime_minutes: None,
            });
        }
    }
    Ok(games)
}
