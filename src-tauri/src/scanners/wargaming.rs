use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use winreg::enums::*;
use winreg::RegKey;

/// Discover games managed by Wargaming.net Game Center (World of Tanks, etc.).
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for path in collect_candidate_roots() {
        if let Some(game) = try_parse_install(&path) {
            if seen.insert(game.id.clone()) {
                games.push(game);
            }
        }
    }

    Ok(games)
}

fn collect_candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut push = |p: PathBuf| {
        if p.is_dir() && !roots.iter().any(|x| x == &p) {
            roots.push(p);
        }
    };

    // WGC preferences often list install folders
    for p in paths_from_preferences() {
        push(p);
    }

    // Uninstall registry entries
    for p in paths_from_uninstall_registry() {
        push(p);
    }

    // Common default locations
    for base in [
        r"C:\Games",
        r"D:\Games",
        r"E:\Games",
        r"C:\Program Files (x86)",
        r"C:\Program Files",
        r"D:\Program Files (x86)",
        r"D:\Wargaming.net",
        r"C:\Wargaming.net",
    ] {
        let base = PathBuf::from(base);
        if !base.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.contains("world_of_tanks")
                    || name.contains("world of tanks")
                    || name.contains("world_of_warships")
                    || name.contains("world of warships")
                    || name.contains("world_of_warplanes")
                    || name.contains("world of warplanes")
                    || name.contains("wot_")
                    || name.starts_with("wot.")
                {
                    push(entry.path());
                }
            }
        }
    }

    // WGC ProgramData may mirror installed app metadata with path hints
    let program_data = PathBuf::from(r"C:\ProgramData\Wargaming.net\GameCenter");
    if program_data.is_dir() {
        for entry in WalkDir::new(&program_data)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name == "game_info.xml" {
                if let Some(parent) = path.parent() {
                    push(parent.to_path_buf());
                }
            }
            if name.ends_with(".xml") {
                if let Ok(text) = fs::read_to_string(path) {
                    for p in extract_paths_from_xml(&text) {
                        push(p);
                    }
                }
            }
        }
    }

    roots
}

fn paths_from_preferences() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let prefs = PathBuf::from(r"C:\ProgramData\Wargaming.net\GameCenter\preferences.xml");
    if let Ok(text) = fs::read_to_string(prefs) {
        out.extend(extract_paths_from_xml(&text));
    }
    out
}

fn extract_paths_from_xml(text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // Match Windows paths in XML attributes/text
    let Ok(re) = Regex::new(r#"[A-Za-z]:\\(?:[^<>:"|?*\r\n]+\\)*[^<>:"|?*\r\n]*"#) else {
        return out;
    };
    for m in re.find_iter(text) {
        let p = PathBuf::from(m.as_str());
        if p.is_dir() {
            out.push(p);
        } else if let Some(parent) = p.parent() {
            if parent.is_dir() {
                out.push(parent.to_path_buf());
            }
        }
    }
    out
}

fn paths_from_uninstall_registry() -> Vec<PathBuf> {
    let mut out = Vec::new();
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
            let lower = display.to_lowercase();
            if !(lower.contains("world of tanks")
                || lower.contains("world of warships")
                || lower.contains("world of warplanes")
                || lower.contains("wargaming"))
            {
                continue;
            }
            if lower.contains("game center") || lower.contains("wgc") {
                continue;
            }
            if let Ok(loc) = app.get_value::<String, _>("InstallLocation") {
                let p = PathBuf::from(loc.trim_matches('"'));
                if p.is_dir() {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn try_parse_install(dir: &Path) -> Option<DiscoveredGame> {
    let info = dir.join("game_info.xml");
    let (id, name) = if info.exists() {
        parse_game_info(&info).unwrap_or_else(|| fallback_identity(dir))
    } else if looks_like_wargaming_install(dir) {
        fallback_identity(dir)
    } else {
        return None;
    };

    let exe = find_game_exe(dir, &id, &name)?;
    Some(DiscoveredGame {
        id: format!("wargaming:{}", slug(&id)),
        name,
        store: Store::Wargaming,
        launch_target: exe.to_string_lossy().to_string(),
        install_path: Some(dir.to_string_lossy().to_string()),
        steam_app_id: None,
        playtime_minutes: None,
    })
}

fn parse_game_info(path: &Path) -> Option<(String, String)> {
    let text = fs::read_to_string(path).ok()?;
    let id = capture_xml_tag(&text, &["game_id", "id", "gameId"])
        .or_else(|| capture_xml_attr(&text, "game_id"))?;
    let name = capture_xml_tag(&text, &["name", "title", "game_name"])
        .map(|n| tidy_name(&n))
        .unwrap_or_else(|| display_name_from_id(&id));
    Some((id, name))
}

fn capture_xml_tag(text: &str, tags: &[&str]) -> Option<String> {
    for tag in tags {
        let pat = format!(r"(?is)<{tag}[^>]*>([^<]+)</{tag}>");
        if let Ok(re) = Regex::new(&pat) {
            if let Some(c) = re.captures(text) {
                let v = c.get(1)?.as_str().trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn capture_xml_attr(text: &str, attr: &str) -> Option<String> {
    let pat = format!(r#"(?i){attr}\s*=\s*"([^"]+)""#);
    let re = Regex::new(&pat).ok()?;
    let c = re.captures(text)?;
    Some(c.get(1)?.as_str().trim().to_string())
}

fn fallback_identity(dir: &Path) -> (String, String) {
    let folder = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Wargaming Game".into());
    let lower = folder.to_lowercase().replace(' ', "_");
    let id = if lower.contains("warship") {
        "wows.unknown"
    } else if lower.contains("warplane") {
        "wowp.unknown"
    } else {
        "wot.unknown"
    };
    (id.to_string(), tidy_name(&folder))
}

fn looks_like_wargaming_install(dir: &Path) -> bool {
    const MARKERS: &[&str] = &[
        "WorldOfTanks.exe",
        "WorldOfWarships.exe",
        "WorldOfWarplanes.exe",
        "wot.exe",
        "wows.exe",
        "wowp.exe",
        "game_info.xml",
        "wgc_api.exe",
    ];
    MARKERS.iter().any(|m| dir.join(m).exists())
}

fn find_game_exe(dir: &Path, id: &str, name: &str) -> Option<PathBuf> {
    let lower_id = id.to_lowercase();
    let lower_name = name.to_lowercase();
    let preferred: &[&str] = if lower_id.contains("wows") || lower_name.contains("warship") {
        &["WorldOfWarships.exe", "wows.exe"]
    } else if lower_id.contains("wowp") || lower_name.contains("warplane") {
        &["WorldOfWarplanes.exe", "wowp.exe"]
    } else {
        &["WorldOfTanks.exe", "wot.exe", "Tanki.exe"]
    };

    for name in preferred {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }

    // Shallow search for a plausible primary exe
    WalkDir::new(dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .find_map(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if !n.ends_with(".exe") {
                return None;
            }
            if n.contains("uninstall")
                || n.contains("crash")
                || n.contains("report")
                || n.contains("wgc_api")
                || n.contains("setup")
            {
                return None;
            }
            if n.contains("worldoftanks")
                || n.contains("worldofwarships")
                || n.contains("worldofwarplanes")
                || n == "wot.exe"
                || n == "wows.exe"
                || n == "wowp.exe"
            {
                return Some(e.path().to_path_buf());
            }
            None
        })
}

fn display_name_from_id(id: &str) -> String {
    let lower = id.to_lowercase();
    if lower.contains("wows") {
        "World of Warships".into()
    } else if lower.contains("wowp") {
        "World of Warplanes".into()
    } else if lower.contains("wot") {
        "World of Tanks".into()
    } else {
        tidy_name(id)
    }
}

fn tidy_name(raw: &str) -> String {
    raw.replace('_', " ")
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
