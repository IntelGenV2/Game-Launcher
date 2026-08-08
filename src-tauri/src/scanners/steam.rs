use crate::models::{DiscoveredGame, Store};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let steam_path = find_steam_path().context("Steam not found")?;
    let libraries = parse_library_folders(&steam_path)?;
    let playtimes = load_playtimes(&steam_path).unwrap_or_default();

    let mut games = Vec::new();
    for lib in libraries {
        let steamapps = lib.join("steamapps");
        if !steamapps.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&steamapps)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            if let Some(game) = parse_appmanifest(&path, &steamapps, &playtimes) {
                // Skip Steamworks Common Redistributables and similar tools
                if is_toolish(&game.name) {
                    continue;
                }
                games.push(game);
            }
        }
    }
    Ok(games)
}

fn is_toolish(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("steamworks common")
        || lower.contains("proton")
        || lower == "steam linux runtime"
        || lower.starts_with("steam linux runtime")
}

fn find_steam_path() -> Result<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            let p = PathBuf::from(path.replace('/', "\\"));
            if p.exists() {
                return Ok(p);
            }
        }
    }
    let candidates = [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"E:\Steam",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.join("steam.exe").exists() {
            return Ok(p);
        }
    }
    anyhow::bail!("Steam installation not found")
}

fn parse_library_folders(steam_path: &Path) -> Result<Vec<PathBuf>> {
    let mut libs = vec![steam_path.to_path_buf()];
    let vdf = steam_path.join("steamapps").join("libraryfolders.vdf");
    if !vdf.exists() {
        return Ok(libs);
    }
    let text = fs::read_to_string(&vdf)?;
    let re = Regex::new(r#""path"\s+"([^"]+)""#)?;
    for cap in re.captures_iter(&text) {
        let path = PathBuf::from(cap[1].replace("\\\\", "\\"));
        if path.exists() && !libs.iter().any(|l| l == &path) {
            libs.push(path);
        }
    }
    Ok(libs)
}

fn parse_appmanifest(
    path: &Path,
    steamapps: &Path,
    playtimes: &HashMap<String, i64>,
) -> Option<DiscoveredGame> {
    let text = fs::read_to_string(path).ok()?;
    let appid = extract_vdf_value(&text, "appid")?;
    let name = extract_vdf_value(&text, "name")?;
    let installdir = extract_vdf_value(&text, "installdir");
    let install_path = installdir.map(|d| steamapps.join("common").join(d));

    // Skip uninstalled / incomplete
    if let Some(state) = extract_vdf_value(&text, "StateFlags") {
        // 4 = fully installed typically; still include most non-zero
        if state == "0" {
            return None;
        }
    }

    let playtime = playtimes.get(&appid).copied();
    Some(DiscoveredGame {
        id: format!("steam:{appid}"),
        name,
        store: Store::Steam,
        launch_target: appid.clone(),
        install_path: install_path.map(|p| p.to_string_lossy().to_string()),
        steam_app_id: Some(appid),
        playtime_minutes: playtime,
    })
}

fn extract_vdf_value(text: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#""{}"\s+"([^"]*)""#, regex::escape(key))).ok()?;
    re.captures(text).map(|c| c[1].to_string())
}

/// Best-effort playtime from userdata/*/config/localconfig.vdf (minutes).
fn load_playtimes(steam_path: &Path) -> Result<HashMap<String, i64>> {
    let mut map = HashMap::new();
    let userdata = steam_path.join("userdata");
    if !userdata.is_dir() {
        return Ok(map);
    }
    let re = Regex::new(r#""(\d+)"\s*\{[^}]*?"Playtime"\s+"(\d+)""#)?;
    // Broader multiline-ish scan: "appid" { ... "Playtime" "N"
    let playtime_re = Regex::new(
        r#"(?s)"(\d+)"\s*\{(?:(?!"\d+"\s*\{).)*?"Playtime"\s+"(\d+)""#,
    )?;

    for entry in fs::read_dir(userdata)? {
        let entry = entry?;
        let config = entry.path().join("config").join("localconfig.vdf");
        if !config.exists() {
            continue;
        }
        let text = match fs::read_to_string(&config) {
            Ok(t) => t,
            Err(_) => continue,
        };
        // Prefer detailed regex; fallback simpler
        for cap in playtime_re.captures_iter(&text) {
            let appid = cap[1].to_string();
            let minutes: i64 = cap[2].parse().unwrap_or(0);
            map.entry(appid)
                .and_modify(|m| *m = (*m).max(minutes))
                .or_insert(minutes);
        }
        // Also try simple adjacent pairs if nothing found in this file
        if map.is_empty() {
            for cap in re.captures_iter(&text) {
                let appid = cap[1].to_string();
                let minutes: i64 = cap[2].parse().unwrap_or(0);
                map.insert(appid, minutes);
            }
        }
    }
    Ok(map)
}
