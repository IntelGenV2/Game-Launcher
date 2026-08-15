use crate::models::{DiscoveredGame, Store};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Product id -> display name from Playnite RiotGamesLibrary (ASchoe311).
fn known_name(product: &str) -> Option<&'static str> {
    match product {
        "league_of_legends" => Some("League of Legends"),
        "valorant" => Some("VALORANT"),
        "bacon" => Some("Legends of Runeterra"),
        "lion" => Some("2XKO"),
        _ => None,
    }
}

fn known_exe(product: &str) -> Option<&'static str> {
    match product {
        "league_of_legends" => Some("LeagueClient.exe"),
        "valorant" => Some("VALORANT.exe"),
        "bacon" => Some("LoR.exe"),
        "lion" => Some("Lion.exe"),
        _ => None,
    }
}

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let client = find_client();

    for (product, name, install) in installed_from_metadata() {
        if !seen.insert(product.clone()) {
            continue;
        }
        games.push(make_game(&product, &name, install, client.clone()));
    }

    // Playnite RiotGamesLibrary also checks HKCU uninstall keys when shortcuts exist.
    for (product, name, install) in installed_from_uninstall() {
        if !seen.insert(product.clone()) {
            continue;
        }
        games.push(make_game(&product, &name, install, client.clone()));
    }

    Ok(games)
}

pub fn launch(game: &crate::models::Game) -> Result<()> {
    let product = game
        .id
        .strip_prefix("riot:")
        .unwrap_or(game.launch_target.as_str());
    let client = find_client().unwrap_or_else(|| PathBuf::from(&game.launch_target));
    if !client.is_file() {
        bail!("Riot Client not found");
    }
    let dir = client.parent().unwrap_or_else(|| Path::new("."));
    Command::new("cmd")
        .current_dir(dir)
        .args([
            "/C",
            "start",
            "",
            &client.to_string_lossy(),
            &format!("--launch-product={product}"),
            "--launch-patchline=live",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("failed to start Riot Client")?;
    Ok(())
}

fn make_game(
    product: &str,
    name: &str,
    install: PathBuf,
    client: Option<PathBuf>,
) -> DiscoveredGame {
    DiscoveredGame {
        id: format!("riot:{product}"),
        name: name.to_string(),
        store: Store::Riot,
        launch_target: client
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| product.to_string()),
        install_path: Some(install.to_string_lossy().to_string()),
        steam_app_id: None,
        playtime_minutes: None,
    }
}

/// GameLib.NET RiotGamesFactory: `%ProgramData%\Riot Games\Metadata\*.live\*.live.product_settings.yaml`
/// Installed only when `product_install_full_path` is present (GOG Galaxy Riot issue #6).
fn installed_from_metadata() -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    let meta = PathBuf::from(r"C:\ProgramData\Riot Games\Metadata");
    if !meta.is_dir() {
        return out;
    }
    let Ok(entries) = fs::read_dir(&meta) else {
        return out;
    };
    let Ok(path_re) = Regex::new(r#"(?im)^\s*product_install_full_path:\s*"?([^"\r\n]+)"?"#) else {
        return out;
    };
    let Ok(shortcut_re) = Regex::new(r#"(?im)^\s*shortcut_name:\s*"?([^"\r\n]+)"?"#) else {
        return out;
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().to_string();
        let Some(product) = folder.strip_suffix(".live") else {
            continue;
        };
        if product.eq_ignore_ascii_case("riot client") {
            continue;
        }
        let yaml = dir.join(format!("{folder}.product_settings.yaml"));
        let Ok(text) = fs::read_to_string(&yaml) else {
            continue;
        };
        let Some(caps) = path_re.captures(&text) else {
            continue;
        };
        let install = PathBuf::from(caps.get(1).unwrap().as_str().trim().replace('/', "\\"));
        if !install.is_dir() {
            continue;
        }
        if let Some(exe) = known_exe(product) {
            if !install.join(exe).is_file() {
                continue;
            }
        }
        let name = shortcut_re
            .captures(&text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().trim_end_matches(".lnk").to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| known_name(product).map(|s| s.to_string()))
            .unwrap_or_else(|| humanize(product));
        out.push((product.to_string(), name, install));
    }
    out
}

/// Playnite RiotGamesLibrary: `HKCU\...\Uninstall\Riot Game {product}.live` + exe on disk.
fn installed_from_uninstall() -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Uninstall") else {
        return out;
    };
    for name in key.enum_keys().flatten() {
        let Some(rest) = name.strip_prefix("Riot Game ") else {
            continue;
        };
        let Some(product) = rest.strip_suffix(".live") else {
            continue;
        };
        let Ok(app) = key.open_subkey(&name) else {
            continue;
        };
        let loc: String = app
            .get_value::<String, _>("InstallLocation")
            .unwrap_or_default();
        if loc.is_empty() {
            continue;
        }
        let install = PathBuf::from(loc);
        if !install.is_dir() {
            continue;
        }
        if let Some(exe) = known_exe(product) {
            if !install.join(exe).is_file() {
                continue;
            }
        }
        let name = known_name(product)
            .map(|s| s.to_string())
            .unwrap_or_else(|| humanize(product));
        out.push((product.to_string(), name, install));
    }
    out
}

/// Playnite: HKCR `riotclient\shell\open\command`. GameLib.NET: uninstall Publisher = "Riot Games, Inc".
fn find_client() -> Option<PathBuf> {
    for hive in [HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        let path = if hive == HKEY_LOCAL_MACHINE {
            r"SOFTWARE\Classes\riotclient\shell\open\command"
        } else {
            r"riotclient\shell\open\command"
        };
        if let Ok(key) = root.open_subkey(path) {
            if let Ok(cmd) = key.get_value::<String, _>("") {
                if let Some(exe) = extract_quoted_exe(&cmd) {
                    if exe.ends_with("RiotClientServices.exe") && Path::new(&exe).is_file() {
                        return Some(PathBuf::from(exe));
                    }
                }
            }
        }
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(uninstall) = hkcu.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall")
    {
        for name in uninstall.enum_keys().flatten() {
            let Ok(app) = uninstall.open_subkey(&name) else {
                continue;
            };
            let publisher: String = app.get_value("Publisher").unwrap_or_default();
            if publisher != "Riot Games, Inc" {
                continue;
            }
            let uninstall_str: String = app.get_value("UninstallString").unwrap_or_default();
            if let Some(exe) = extract_quoted_exe(&uninstall_str) {
                if exe.ends_with("RiotClientServices.exe") && Path::new(&exe).is_file() {
                    return Some(PathBuf::from(exe));
                }
            }
        }
    }
    None
}

fn extract_quoted_exe(cmd: &str) -> Option<String> {
    let start = cmd.find('"')?;
    let rest = &cmd[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn humanize(product: &str) -> String {
    product
        .split('_')
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
