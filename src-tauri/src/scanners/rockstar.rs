use crate::models::{DiscoveredGame, Store};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Playnite RockstarGames.Games catalog (title id, name).
fn known_games() -> &'static [(&'static str, &'static str)] {
    &[
        ("gta5", "Grand Theft Auto V"),
        ("gta5_gen9", "Grand Theft Auto V Enhanced"),
        ("rdr", "Red Dead Redemption"),
        ("rdr2", "Red Dead Redemption 2"),
        ("lanoire", "L.A. Noire"),
        ("mp3", "Max Payne 3"),
        ("lanoirevr", "L.A. Noire: The VR Case Files"),
        ("gtasa", "Grand Theft Auto: San Andreas"),
        ("gta3", "Grand Theft Auto III"),
        ("gtavc", "Grand Theft Auto: Vice City"),
        ("bully", "Bully: Scholarship Edition"),
        ("gta4", "Grand Theft Auto IV"),
        ("gta3unreal", "Grand Theft Auto III: The Definitive Edition"),
        ("gtavcunreal", "Grand Theft Auto: Vice City – The Definitive Edition"),
        ("gtasaunreal", "Grand Theft Auto: San Andreas – The Definitive Edition"),
    ]
}

fn name_for_title(id: &str) -> Option<&'static str> {
    known_games()
        .iter()
        .find(|(tid, _)| tid.eq_ignore_ascii_case(id))
        .map(|(_, n)| *n)
}

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let launcher = find_launcher();

    for (title_id, install) in installed_from_uninstall() {
        let Some(name) = name_for_title(&title_id) else {
            continue;
        };
        if !Path::new(&install).is_dir() {
            continue;
        }
        if !seen.insert(title_id.clone()) {
            continue;
        }
        games.push(make_game(&title_id, name, &install, launcher.as_ref()));
    }

    // GameLib.NET: HKLM\SOFTWARE\Rockstar Games\<Name>\InstallFolder
    for (key_name, install) in installed_from_registry() {
        if !Path::new(&install).is_dir() {
            continue;
        }
        let title_id = known_games()
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(&key_name))
            .map(|(id, _)| (*id).to_string())
            .unwrap_or_else(|| slug(&key_name));
        if !seen.insert(title_id.clone()) {
            continue;
        }
        let name = name_for_title(&title_id).unwrap_or(key_name.as_str());
        games.push(make_game(&title_id, name, &install, launcher.as_ref()));
    }

    Ok(games)
}

pub fn launch(game: &crate::models::Game) -> Result<()> {
    let launcher = find_launcher().or_else(|| {
        let p = PathBuf::from(&game.launch_target);
        p.is_file().then_some(p)
    });
    let Some(launcher) = launcher else {
        bail!("Rockstar Games Launcher not found");
    };
    let Some(install) = game.install_path.as_deref() else {
        bail!("Rockstar install directory missing");
    };
    if !Path::new(install).is_dir() {
        bail!("Rockstar install directory not found: {install}");
    }
    let dir = launcher.parent().unwrap_or_else(|| Path::new("."));
    Command::new("cmd")
        .current_dir(dir)
        .args([
            "/C",
            "start",
            "",
            &launcher.to_string_lossy(),
            "-launchTitleInFolder",
            install,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("failed to start Rockstar Games Launcher")?;
    Ok(())
}

fn make_game(
    title_id: &str,
    name: &str,
    install: &str,
    launcher: Option<&PathBuf>,
) -> DiscoveredGame {
    DiscoveredGame {
        id: format!("rockstar:{title_id}"),
        name: name.to_string(),
        store: Store::Rockstar,
        launch_target: launcher
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| title_id.to_string()),
        install_path: Some(install.to_string()),
        steam_app_id: None,
        playtime_minutes: None,
    }
}

/// Playnite: uninstall string matches `(?:Launcher|uninstall)\.exe.+uninstall=(.+)$`
fn installed_from_uninstall() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(re) = Regex::new(r"(?i)(?:Launcher|uninstall)\.exe.+uninstall=(.+)$") else {
        return out;
    };
    for_each_uninstall(|uninstall, loc| {
        let Some(caps) = re.captures(&uninstall) else {
            return;
        };
        let title_id = caps.get(1).unwrap().as_str().trim().to_string();
        if title_id.is_empty() || loc.is_empty() {
            return;
        }
        out.push((title_id, loc));
    });
    out
}

fn installed_from_registry() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for root in [r"SOFTWARE\WOW6432Node\Rockstar Games", r"SOFTWARE\Rockstar Games"] {
        let Ok(key) = hklm.open_subkey(root) else {
            continue;
        };
        for name in key.enum_keys().flatten() {
            if name.eq_ignore_ascii_case("Launcher")
                || name.eq_ignore_ascii_case("Rockstar Games Social Club")
            {
                continue;
            }
            let Ok(sub) = key.open_subkey(&name) else {
                continue;
            };
            let loc: String = sub.get_value("InstallFolder").unwrap_or_default();
            if loc.is_empty() {
                continue;
            }
            out.push((name, loc));
        }
    }
    out
}

fn find_launcher() -> Option<PathBuf> {
    let mut found = None;
    for_each_uninstall_named(|display, loc| {
        if display == "Rockstar Games Launcher" {
            let exe = PathBuf::from(&loc).join("Launcher.exe");
            if exe.is_file() {
                found = Some(exe);
            }
        }
    });
    found
}

fn for_each_uninstall(mut f: impl FnMut(String, String)) {
    let hives = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (hive, sub) in hives {
        let root = RegKey::predef(hive);
        let Ok(key) = root.open_subkey(sub) else {
            continue;
        };
        for name in key.enum_keys().flatten() {
            let Ok(app) = key.open_subkey(&name) else {
                continue;
            };
            let uninstall: String = app.get_value("UninstallString").unwrap_or_default();
            let loc: String = app
                .get_value::<String, _>("InstallLocation")
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            f(uninstall, loc);
        }
    }
}

fn for_each_uninstall_named(mut f: impl FnMut(String, String)) {
    let hives = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (hive, sub) in hives {
        let root = RegKey::predef(hive);
        let Ok(key) = root.open_subkey(sub) else {
            continue;
        };
        for name in key.enum_keys().flatten() {
            let Ok(app) = key.open_subkey(&name) else {
                continue;
            };
            let display: String = app.get_value("DisplayName").unwrap_or_default();
            let loc: String = app
                .get_value::<String, _>("InstallLocation")
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            f(display, loc);
        }
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
