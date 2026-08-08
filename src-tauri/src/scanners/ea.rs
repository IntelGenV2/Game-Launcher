use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discover only games registered with EA Desktop InstallData.
/// Custom / loose folders (e.g. old NFS Most Wanted 2005) are ignored unless EA lists them.
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let install_data = PathBuf::from(r"C:\ProgramData\EA Desktop\InstallData");
    if !install_data.is_dir() {
        return Ok(games);
    }

    for entry in fs::read_dir(&install_data)?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        // InstallData entry must look like a real EA package (has content hashes / maps)
        if !install_data_looks_valid(&entry.path()) {
            continue;
        }
        if let Some(game) = resolve_ea_game(&name, &mut seen) {
            games.push(game);
        }
    }

    Ok(games)
}

fn install_data_looks_valid(dir: &Path) -> bool {
    WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.ends_with(".eacrc")
                || n.contains("origin.sft")
                || n.ends_with(".dlc")
                || n == "installerdata.xml"
                || n.ends_with(".xml") && n.contains("installer")
        })
}

fn resolve_ea_game(
    name: &str,
    seen: &mut std::collections::HashSet<String>,
) -> Option<DiscoveredGame> {
    let key = name.to_lowercase();
    if !seen.insert(key) {
        return None;
    }
    let install = find_install_path(name)?;
    if !is_valid_ea_install(&install) {
        return None;
    }
    let exe = find_game_exe(&install)?;
    Some(DiscoveredGame {
        id: format!("ea:{}", slug(name)),
        name: name.to_string(),
        store: Store::Ea,
        launch_target: exe,
        install_path: Some(install.to_string_lossy().to_string()),
        steam_app_id: None,
        playtime_minutes: None,
    })
}

/// EA-managed installs typically include __Installer, Support, or EA/Origin runtime bits.
fn is_valid_ea_install(dir: &Path) -> bool {
    if dir.join("__Installer").is_dir() {
        return true;
    }
    if dir.join("Support").is_dir() {
        return true;
    }
    // EA anticheat / services
    for marker in [
        "EAAntiCheat.GameServiceLauncher.exe",
        "EASteamProxy.exe",
        "OriginIGC64.dll",
        "EOSSDK-Win64-Shipping.dll",
    ] {
        if dir.join(marker).exists() {
            return true;
        }
    }
    WalkDir::new(dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.contains("eaanticheat")
                || n.contains("eadesktop")
                || n.starts_with("origin")
                || n == "installationinfo.ini"
        })
}

fn find_install_path(name: &str) -> Option<PathBuf> {
    let name_l = name.to_lowercase();
    for root in ea_search_roots() {
        if !root.is_dir() {
            continue;
        }
        // Direct child match
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let n = entry.file_name().to_string_lossy().to_string();
                let n_l = n.to_lowercase();
                if n_l == name_l {
                    return Some(path);
                }
                // One level deeper only when the child name matches exactly (case-insensitive)
                if let Ok(subs) = fs::read_dir(&path) {
                    for child in subs.flatten() {
                        if !child.path().is_dir() {
                            continue;
                        }
                        let cn = child.file_name().to_string_lossy().to_string();
                        if cn.eq_ignore_ascii_case(name) {
                            return Some(child.path());
                        }
                    }
                }
            }
        }
    }
    None
}

fn ea_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    // Prefer EA-configured library roots from local user ini when present
    if let Some(local) = dirs::data_local_dir() {
        let desktop = local.join("Electronic Arts").join("EA Desktop");
        if let Ok(entries) = fs::read_dir(&desktop) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("user_") && name.ends_with(".ini") {
                    if let Ok(text) = fs::read_to_string(entry.path()) {
                        for line in text.lines() {
                            if let Some(rest) = line
                                .split_once("installpath=")
                                .or_else(|| line.split_once("InstallPath="))
                                .or_else(|| line.split_once("locale="))
                            {
                                let _ = rest;
                            }
                            // Common key patterns for download locations
                            let lower = line.to_lowercase();
                            if lower.contains("downloadinplacedir")
                                || lower.contains("content_path")
                                || lower.contains("install_dir")
                            {
                                if let Some((_, val)) = line.split_once('=') {
                                    let p = PathBuf::from(val.trim().trim_matches('"'));
                                    if p.is_dir() {
                                        roots.push(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for letter in b'C'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if !Path::new(&drive).exists() {
            continue;
        }
        for name in [
            "Program Files\\EA Games",
            "Program Files (x86)\\EA Games",
            "Program Files\\Electronic Arts",
            "Program Files (x86)\\Electronic Arts",
            "EA Games",
            "Origin Games",
            "Games",
        ] {
            roots.push(PathBuf::from(format!("{drive}{name}")));
        }
    }
    roots
}

fn find_game_exe(dir: &Path) -> Option<String> {
    let folder = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let compact: String = folder.chars().filter(|c| c.is_ascii_alphanumeric()).collect();

    let mut exes: Vec<(i32, PathBuf)> = WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        })
        .map(|p| {
            let n = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let compact_exe: String = n.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            let mut score = 0i32;
            if !compact.is_empty() && (compact_exe.contains(&compact) || compact.contains(&compact_exe))
            {
                score -= 40;
            }
            if n.contains("trial") {
                score += 40;
            }
            if n.contains("unins")
                || n.contains("setup")
                || n.contains("redist")
                || n.contains("crash")
                || n.contains("eadesktop")
                || n.contains("eaanticheat")
                || n.contains("origin")
            {
                score += 100;
            }
            score += p.components().count() as i32;
            (score, p)
        })
        .collect();
    if exes.is_empty() {
        return None;
    }
    exes.sort_by_key(|(s, p)| (*s, p.to_string_lossy().to_string()));
    Some(exes[0].1.to_string_lossy().to_string())
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
