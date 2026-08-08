use crate::models::{DiscoveredGame, Store};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn create_manual_from_path(path: &str) -> Result<DiscoveredGame> {
    let p = PathBuf::from(path);
    if !p.exists() {
        bail!("Path does not exist");
    }
    let (name, launch_target, install_path) = if p.is_file() {
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Manual Game".into());
        let install = p.parent().map(|d| d.to_string_lossy().to_string());
        (name, p.to_string_lossy().to_string(), install)
    } else {
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Manual Game".into());
        let exe = find_exe_in_dir(&p);
        let launch = exe.unwrap_or_else(|| p.to_string_lossy().to_string());
        (name, launch, Some(p.to_string_lossy().to_string()))
    };

    Ok(DiscoveredGame {
        id: format!("manual:{}", Uuid::new_v4()),
        name,
        store: Store::Manual,
        launch_target,
        install_path,
        steam_app_id: None,
        playtime_minutes: None,
    })
}

fn find_exe_in_dir(dir: &Path) -> Option<String> {
    let mut exes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
            {
                exes.push(path);
            }
        }
    }
    exes.sort_by_key(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let penalty = if n.contains("unins") || n.contains("setup") || n.contains("crash") {
            1
        } else {
            0
        };
        (penalty, n)
    });
    exes.first().map(|p| p.to_string_lossy().to_string())
}
