use crate::models::{DiscoveredGame, PlayniteImportResult, Store};
use crate::scanners;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct PlayniteGame {
    #[allow(dead_code)]
    id: Option<String>,
    name: Option<String>,
    #[serde(alias = "InstallDirectory")]
    install_directory: Option<String>,
    #[serde(alias = "IsInstalled")]
    is_installed: Option<bool>,
    playtime: Option<u64>,
    hidden: Option<bool>,
    favorite: Option<bool>,
    #[serde(alias = "GameId")]
    game_id: Option<String>,
    source: Option<PlayniteNamed>,
    #[serde(default)]
    tags: Vec<PlayniteNamed>,
    #[serde(default)]
    genres: Vec<PlayniteNamed>,
    developers: Option<Vec<PlayniteNamed>>,
    publishers: Option<Vec<PlayniteNamed>>,
    #[serde(alias = "ReleaseYear")]
    release_year: Option<i64>,
    #[serde(alias = "ReleaseDate")]
    release_date: Option<serde_json::Value>,
    notes: Option<String>,
    description: Option<String>,
    #[serde(alias = "PluginId")]
    plugin_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PlayniteNamed {
    Name { name: Option<String> },
    Str(String),
}

impl PlayniteNamed {
    fn as_name(&self) -> Option<String> {
        match self {
            PlayniteNamed::Name { name } => name.clone().filter(|s| !s.trim().is_empty()),
            PlayniteNamed::Str(s) => {
                let t = s.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportedPlayniteGame {
    pub name: String,
    pub install_dir: Option<String>,
    pub exe_path: Option<String>,
    pub playtime_minutes: i64,
    pub hidden: bool,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_year: Option<i64>,
    pub notes: Option<String>,
    pub description: Option<String>,
    pub steam_app_id: Option<String>,
    #[allow(dead_code)]
    pub store: Store,
}

pub fn load_from_path(path: &str) -> Result<(Vec<ImportedPlayniteGame>, PlayniteImportResult)> {
    let p = PathBuf::from(path);
    if !p.exists() {
        anyhow::bail!("Playnite path not found");
    }
    let files = collect_library_files(&p)?;
    if files.is_empty() {
        anyhow::bail!("No Playnite library files found. Pick the Playnite data folder or a games.yaml / games.json export.");
    }

    let mut raw = Vec::new();
    for file in files {
        match parse_file(&file) {
            Ok(mut games) => raw.append(&mut games),
            Err(e) => eprintln!("Playnite parse {}: {e:#}", file.display()),
        }
    }

    let mut seen = HashSet::new();
    let mut imported = Vec::new();
    let mut skipped = 0usize;
    for g in raw {
        let Some(name) = g.name.clone().filter(|s| !s.trim().is_empty()) else {
            skipped += 1;
            continue;
        };
        let installed = g.is_installed.unwrap_or(true);
        let install_dir = g
            .install_directory
            .clone()
            .filter(|s| !s.trim().is_empty());
        if !installed && install_dir.is_none() {
            skipped += 1;
            continue;
        }
        let key = normalize_name(&name);
        if !seen.insert(key) {
            skipped += 1;
            continue;
        }

        let exe_path = install_dir.as_deref().and_then(find_exe);
        let steam_app_id = steam_id_from(&g);
        let store = store_from(&g);
        let year = g.release_year.or_else(|| year_from_value(g.release_date.as_ref()));
        let playtime_minutes = g.playtime.map(|s| (s / 60) as i64).unwrap_or(0);

        imported.push(ImportedPlayniteGame {
            name,
            install_dir,
            exe_path,
            playtime_minutes,
            hidden: g.hidden.unwrap_or(false),
            favorite: g.favorite.unwrap_or(false),
            tags: named_list(&g.tags),
            genres: named_list(&g.genres),
            developer: named_list(&g.developers.unwrap_or_default())
                .into_iter()
                .next(),
            publisher: named_list(&g.publishers.unwrap_or_default())
                .into_iter()
                .next(),
            release_year: year,
            notes: g.notes.filter(|s| !s.trim().is_empty()),
            description: g.description.filter(|s| !s.trim().is_empty()),
            steam_app_id,
            store,
        });
    }

    Ok((
        imported,
        PlayniteImportResult {
            updated: 0,
            added: 0,
            skipped,
        },
    ))
}

pub fn create_discovered(game: &ImportedPlayniteGame) -> Option<DiscoveredGame> {
    let path = game.exe_path.as_deref().or(game.install_dir.as_deref())?;
    scanners::create_manual_from_path(path).ok().map(|mut d| {
        d.name = game.name.clone();
        if let Some(id) = &game.steam_app_id {
            d.steam_app_id = Some(id.clone());
        }
        d.playtime_minutes = Some(game.playtime_minutes);
        d
    })
}

fn collect_library_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(files);
    }
    let candidates = [
        path.join("library").join("games.yaml"),
        path.join("library").join("games.json"),
        path.join("games.yaml"),
        path.join("games.json"),
        path.join("library.yaml"),
        path.join("backup").join("games.yaml"),
    ];
    for c in candidates {
        if c.exists() {
            files.push(c);
        }
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if name.contains("games")
                && (name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".json"))
            {
                if !files.iter().any(|f| f == &p) {
                    files.push(p);
                }
            }
        }
    }
    Ok(files)
}

fn parse_file(path: &Path) -> Result<Vec<PlayniteGame>> {
    let text = fs::read_to_string(path)?;
    let name = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name == "json" {
        parse_json(&text)
    } else {
        parse_yaml(&text).or_else(|_| parse_json(&text))
    }
}

fn parse_json(text: &str) -> Result<Vec<PlayniteGame>> {
    if let Ok(list) = serde_json::from_str::<Vec<PlayniteGame>>(text) {
        return Ok(list);
    }
    let v: serde_json::Value = serde_json::from_str(text)?;
    if let Some(arr) = v.as_array() {
        return Ok(arr
            .iter()
            .filter_map(|x| serde_json::from_value(x.clone()).ok())
            .collect());
    }
    for key in ["Games", "games", "Items", "items"] {
        if let Some(arr) = v.get(key).and_then(|x| x.as_array()) {
            return Ok(arr
                .iter()
                .filter_map(|x| serde_json::from_value(x.clone()).ok())
                .collect());
        }
    }
    anyhow::bail!("JSON is not a Playnite game list")
}

fn parse_yaml(text: &str) -> Result<Vec<PlayniteGame>> {
    if let Ok(list) = serde_yaml::from_str::<Vec<PlayniteGame>>(text) {
        return Ok(list);
    }
    let v: serde_yaml::Value = serde_yaml::from_str(text)?;
    let json = serde_json::to_value(v)?;
    parse_json(&json.to_string())
}

fn named_list(items: &[PlayniteNamed]) -> Vec<String> {
    items.iter().filter_map(|n| n.as_name()).collect()
}

fn year_from_value(v: Option<&serde_json::Value>) -> Option<i64> {
    match v {
        Some(serde_json::Value::String(s)) => s.get(0..4)?.parse().ok(),
        Some(serde_json::Value::Number(n)) => n.as_i64(),
        Some(obj) if obj.is_object() => obj
            .get("Year")
            .or_else(|| obj.get("year"))
            .and_then(|x| x.as_i64()),
        _ => None,
    }
}

fn steam_id_from(g: &PlayniteGame) -> Option<String> {
    let plugin = g.plugin_id.as_deref().unwrap_or("").to_lowercase();
    let source = match &g.source {
        Some(s) => s.as_name().unwrap_or_default().to_lowercase(),
        None => String::new(),
    };
    if plugin.contains("steam") || source.contains("steam") {
        return g
            .game_id
            .clone()
            .filter(|id| id.chars().all(|c| c.is_ascii_digit()));
    }
    None
}

fn store_from(g: &PlayniteGame) -> Store {
    let plugin = g.plugin_id.as_deref().unwrap_or("").to_lowercase();
    let source = match &g.source {
        Some(s) => s.as_name().unwrap_or_default().to_lowercase(),
        None => String::new(),
    };
    let blob = format!("{plugin} {source}");
    if blob.contains("steam") {
        Store::Steam
    } else if blob.contains("epic") {
        Store::Epic
    } else if blob.contains("gog") {
        Store::Gog
    } else if blob.contains("xbox") || blob.contains("microsoft") {
        Store::Xbox
    } else if blob.contains("battle") {
        Store::Battlenet
    } else if blob.contains("ubisoft") || blob.contains("uplay") {
        Store::Ubisoft
    } else if blob.contains("origin") || blob.contains("ea app") || blob.contains("ea desktop") {
        Store::Ea
    } else if blob.contains("amazon") {
        Store::Amazon
    } else if blob.contains("itch") {
        Store::Itch
    } else if blob.contains("humble") {
        Store::Humble
    } else if blob.contains("riot") {
        Store::Riot
    } else if blob.contains("rockstar") {
        Store::Rockstar
    } else {
        Store::Manual
    }
}

fn find_exe(dir: &str) -> Option<String> {
    let p = Path::new(dir);
    if p.is_file() {
        return Some(dir.to_string());
    }
    if !p.is_dir() {
        return None;
    }
    let mut exes: Vec<PathBuf> = fs::read_dir(p)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        })
        .collect();
    if exes.is_empty() {
        return Some(dir.to_string());
    }
    exes.sort();
    Some(exes[0].to_string_lossy().to_string())
}

pub fn normalize_name(name: &str) -> String {
    let mut s = name.to_lowercase();
    for strip in [
        "™",
        "®",
        "©",
        " (tm)",
        " standard edition",
        " deluxe edition",
        " ultimate edition",
        " gold edition",
        " game of the year edition",
        " goty edition",
        " goty",
        " windows edition",
        " for windows",
        " pc edition",
    ] {
        s = s.replace(strip, "");
    }
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}
