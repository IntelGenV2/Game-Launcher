use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

/// Known Battle.net product codes -> display names.
/// Codes are lowercase for matching; config keys may vary in case.
fn known_products() -> Vec<(&'static str, &'static str)> {
    vec![
        ("wow", "World of Warcraft"),
        ("wow_classic", "World of Warcraft Classic"),
        ("wowc", "World of Warcraft Classic"),
        ("d3", "Diablo III"),
        ("diablo3", "Diablo III"),
        ("osi", "Diablo II: Resurrected"),
        ("fen", "Diablo IV"),
        ("fenris", "Diablo IV"),
        ("anbs", "Diablo Immortal"),
        ("hsb", "Hearthstone"),
        ("hs_beta", "Hearthstone"),
        ("wtcg", "Hearthstone"),
        ("hero", "Heroes of the Storm"),
        ("heroes", "Heroes of the Storm"),
        ("pro", "Overwatch 2"),
        ("prometheus", "Overwatch 2"),
        ("s2", "StarCraft II"),
        ("s1", "StarCraft Remastered"),
        ("w3", "Warcraft III: Reforged"),
        ("w1r", "Warcraft: Remastered"),
        ("w2r", "Warcraft II: Remastered"),
        ("destiny2", "Destiny 2"),
        // Call of Duty — modern titles share AUKS / Call of Duty HQ
        ("auks", "Call of Duty"),
        ("pinta", "Call of Duty: Modern Warfare III"),
        ("viper", "Call of Duty: Black Ops 4"),
        ("vipr", "Call of Duty: Black Ops 4"),
        ("odin", "Call of Duty: Modern Warfare"),
        ("zeus", "Call of Duty: Black Ops Cold War"),
        ("fore", "Call of Duty: Vanguard"),
        ("lazr", "Call of Duty: Modern Warfare 2 Campaign Remastered"),
        ("lazarus", "Call of Duty: Modern Warfare 2 Campaign Remastered"),
        ("rtro", "Blizzard Arcade Collection"),
        ("wlby", "Crash Bandicoot 4"),
        ("aris", "Doom: The Dark Ages"),
        ("scor", "Sea of Thieves"),
        ("aqua", "Avowed"),
    ]
}

pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let push = |games: &mut Vec<DiscoveredGame>,
                seen: &mut std::collections::HashSet<String>,
                code: &str,
                name: &str,
                path: Option<String>| {
        let code = canonicalize_code(code);
        if !seen.insert(code.clone()) {
            return;
        }
        games.push(make_game(&code, name, path));
    };

    // 1) Uninstall registry — strongest install signal (Playnite approach)
    for (uid, path) in installed_from_uninstall_registry() {
        let code = product_code_from_uid(&uid);
        let name = product_name(&code);
        push(&mut games, &mut seen, &code, &name, Some(path));
    }

    // 2) Battle.net.config — only when Path exists on disk
    for config in battle_net_configs() {
        let Ok(text) = fs::read_to_string(&config) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(map) = json.pointer("/Games").and_then(|v| v.as_object()) else {
            continue;
        };
        for (code, entry) in map {
            let code_l = code.to_lowercase();
            if code_l == "battle_net" || code_l == "battlenet" {
                continue;
            }
            let Some(path) = entry
                .get("Path")
                .or_else(|| entry.get("InstallPath"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
            else {
                continue;
            };
            if path.is_empty() || !Path::new(&path).is_dir() {
                continue;
            }
            // Owned-but-uninstalled entries often linger with empty/stale paths;
            // require at least one install marker under the path.
            if !looks_installed(&PathBuf::from(&path), &code_l) {
                continue;
            }
            let name = product_name(&code_l);
            push(&mut games, &mut seen, &code_l, &name, Some(path));
        }
    }

    // 3) product.db path strings — catch installs without clean registry/config
    for (code, path) in installed_from_product_db() {
        if !Path::new(&path).is_dir() {
            continue;
        }
        let name = product_name(&code);
        push(&mut games, &mut seen, &code, &name, Some(path));
    }

    // 4) Call of Duty HQ — often lives outside Battle.net folder layout
    for path in find_call_of_duty_dirs() {
        push(
            &mut games,
            &mut seen,
            "auks",
            "Call of Duty",
            Some(path.to_string_lossy().to_string()),
        );
    }

    Ok(games)
}

fn canonicalize_code(code: &str) -> String {
    match code.to_lowercase().as_str() {
        "prometheus" => "pro".into(),
        "fenris" => "fen".into(),
        "diablo3" => "d3".into(),
        "hs_beta" | "hsb" => "wtcg".into(),
        "heroes" => "hero".into(),
        "wow_classic" => "wowc".into(),
        "vipr" => "viper".into(),
        "lazarus" => "lazr".into(),
        other => other.to_string(),
    }
}

fn product_code_from_uid(uid: &str) -> String {
    // Uninstall UIDs look like "prometheus", "fenris", "wow", "auks"
    canonicalize_code(uid)
}

fn product_name(code: &str) -> String {
    let canon = canonicalize_code(code);
    known_products()
        .iter()
        .find(|(c, _)| *c == canon || *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| humanize(&canon))
}

fn make_game(code: &str, name: &str, install_path: Option<String>) -> DiscoveredGame {
    let code = canonicalize_code(code);
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

fn battle_net_configs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = dirs::config_dir().map(|p| p.join("Battle.net").join("Battle.net.config")) {
        if p.exists() {
            out.push(p);
        }
    }
    if let Some(local) = dirs::data_local_dir() {
        let p = local.join("Battle.net").join("Battle.net.config");
        if p.exists() {
            out.push(p);
        }
    }
    out
}

fn installed_from_uninstall_registry() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(re) = Regex::new(r#"(?i)Battle\.net.*--uid=([^\s"]+)"#) else {
        return out;
    };
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
            let Some(caps) = re.captures(&uninstall) else {
                continue;
            };
            let uid = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            if uid.is_empty() {
                continue;
            }
            let display: String = app.get_value("DisplayName").unwrap_or_default();
            let lower = display.to_lowercase();
            if lower.ends_with(" test") || lower.ends_with(" beta") {
                continue;
            }
            let loc: String = app
                .get_value::<String, _>("InstallLocation")
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            if loc.is_empty() || !Path::new(&loc).is_dir() {
                continue;
            }
            out.push((uid, loc));
        }
    }
    out
}

fn installed_from_product_db() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let db = PathBuf::from(r"C:\ProgramData\Battle.net\Agent\product.db");
    let Ok(bytes) = fs::read(&db) else {
        return out;
    };
    // product.db is protobuf; pull printable path + nearby product id strings.
    let text: String = bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '\n'
            }
        })
        .collect();

    let Ok(path_re) = Regex::new(r"[A-Za-z]:\\[^\n]{3,200}") else {
        return out;
    };
    let known: Vec<&str> = known_products().into_iter().map(|(c, _)| c).collect();

    for m in path_re.find_iter(&text) {
        let path = m.as_str().trim_end_matches(['\\', '/', '.', ' ']).to_string();
        if !Path::new(&path).is_dir() {
            continue;
        }
        // Look at surrounding window for a product code
        let start = m.start().saturating_sub(120);
        let end = (m.end() + 80).min(text.len());
        let window = text[start..end].to_lowercase();
        for code in &known {
            // word-ish match to avoid "pro" hitting random noise
            if code.len() <= 3 {
                let pat = format!(r"(^|[^a-z0-9]){}([^a-z0-9]|$)", regex::escape(code));
                if Regex::new(&pat).ok().is_some_and(|re| re.is_match(&window)) {
                    if looks_installed(&PathBuf::from(&path), code) {
                        out.push(((*code).to_string(), path.clone()));
                    }
                    break;
                }
            } else if window.contains(code)
                && looks_installed(&PathBuf::from(&path), code)
            {
                out.push(((*code).to_string(), path.clone()));
                break;
            }
        }
    }
    out
}

fn looks_installed(dir: &Path, code: &str) -> bool {
    if !dir.is_dir() {
        return false;
    }
    // Blizzard installs almost always have .product.db in the game root
    if dir.join(".product.db").is_file() {
        return true;
    }
    const MARKERS: &[&str] = &[
        "Overwatch.exe",
        "Diablo IV.exe",
        "Diablo III.exe",
        "Diablo II Resurrected.exe",
        "DiabloImmortal.exe",
        "Wow.exe",
        "WowClassic.exe",
        "Hearthstone.exe",
        "Heroes of the Storm.exe",
        "StarCraft II.exe",
        "Warcraft III.exe",
        "cod.exe",
        "Cod.exe",
        "_retail_",
        "_classic_",
        "Product.db",
    ];
    if MARKERS.iter().any(|m| dir.join(m).exists()) {
        return true;
    }
    // CoD HQ nested layouts
    if code == "auks" || code == "pinta" {
        return dir.join("cod.exe").is_file()
            || dir.join("_retail_").join("cod.exe").is_file()
            || dir.join("Content").join("cod.exe").is_file();
    }
    false
}

fn find_call_of_duty_dirs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let candidates = [
        PathBuf::from(r"C:\Program Files (x86)\Call of Duty"),
        PathBuf::from(r"C:\Program Files\Call of Duty"),
        PathBuf::from(r"C:\Program Files (x86)\Battle.net\Call of Duty"),
        PathBuf::from(r"D:\Call of Duty"),
        PathBuf::from(r"D:\Games\Call of Duty"),
        PathBuf::from(r"E:\Call of Duty"),
        PathBuf::from(r"E:\Games\Call of Duty"),
    ];
    for dir in candidates {
        if looks_installed(&dir, "auks") {
            found.push(dir);
        }
    }
    // Also check common Battle.net install roots for a Call of Duty folder
    for root in [
        PathBuf::from(r"C:\Program Files (x86)\Battle.net"),
        PathBuf::from(r"C:\Program Files\Battle.net"),
        PathBuf::from(r"D:\Battle.net"),
        PathBuf::from(r"E:\Battle.net"),
    ] {
        if !root.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.contains("call of duty") && looks_installed(&path, "auks") {
                    found.push(path);
                }
            }
        }
    }
    found
}
