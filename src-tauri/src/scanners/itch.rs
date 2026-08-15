use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Playnite ItchioLibrary reads butler caves; install folder comes from butler Cave.installInfo.
/// itch.io butler schema (itchio/butler database/models/cave.go + install_location.go):
/// install folder = custom_install_folder OR join(install_locations.path, install_folder_name)
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let Some(roaming) = dirs::config_dir() else {
        return Ok(games);
    };
    // dirs::config_dir on Windows is %APPDATA%; Playnite uses Itch.UserPath\db\butler.db
    let db = roaming.join("itch").join("db").join("butler.db");
    if !db.is_file() {
        return Ok(games);
    }

    let conn = match Connection::open_with_flags(
        &db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("itch.io butler.db: {e:#}");
            return Ok(games);
        }
    };

    let sql = r#"
        SELECT
            caves.id,
            caves.game_id,
            caves.install_folder_name,
            caves.custom_install_folder,
            caves.verdict,
            caves.seconds_run,
            games.title,
            games.classification,
            install_locations.path
        FROM caves
        LEFT JOIN games ON games.id = caves.game_id
        LEFT JOIN install_locations ON install_locations.id = caves.install_location_id
    "#;
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("itch.io query: {e:#}");
            return Ok(games);
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok(CaveRow {
            cave_id: row.get::<_, String>(0)?,
            game_id: row.get::<_, i64>(1).unwrap_or(0),
            folder_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            custom_folder: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            verdict: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            seconds_run: row.get::<_, i64>(5).unwrap_or(0),
            title: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            classification: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
            location_path: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
        })
    });
    let Ok(rows) = rows else {
        return Ok(games);
    };

    let mut seen = std::collections::HashSet::new();
    for row in rows.flatten() {
        if !row.classification.is_empty() && row.classification != "game" {
            continue;
        }
        let install = if !row.custom_folder.is_empty() {
            PathBuf::from(&row.custom_folder)
        } else if !row.location_path.is_empty() && !row.folder_name.is_empty() {
            PathBuf::from(&row.location_path).join(&row.folder_name)
        } else {
            continue;
        };
        if !install.is_dir() {
            continue;
        }
        let Some(launch) = resolve_launch(&install, &row.verdict) else {
            continue;
        };
        let id = if row.game_id != 0 {
            row.game_id.to_string()
        } else {
            row.cave_id.clone()
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let name = if row.title.is_empty() {
            install
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "itch.io game".into())
        } else {
            row.title
        };
        games.push(DiscoveredGame {
            id: format!("itch:{id}"),
            name,
            store: Store::Itch,
            launch_target: launch,
            install_path: Some(install.to_string_lossy().to_string()),
            steam_app_id: None,
            playtime_minutes: if row.seconds_run > 0 {
                Some(row.seconds_run / 60)
            } else {
                None
            },
        });
    }

    Ok(games)
}

struct CaveRow {
    cave_id: String,
    game_id: i64,
    folder_name: String,
    custom_folder: String,
    verdict: String,
    seconds_run: i64,
    title: String,
    classification: String,
    location_path: String,
}

/// Playnite TryGetGameActions: `.itch.toml` action named play.
/// Fallback: itch.io dash Verdict JSON (basePath + candidates[].path / flavor).
fn resolve_launch(install: &Path, verdict: &str) -> Option<String> {
    if let Some(p) = launch_from_toml(install) {
        return Some(p);
    }
    launch_from_verdict(install, verdict)
}

fn launch_from_toml(install: &Path) -> Option<String> {
    let walker = walkdir::WalkDir::new(install).max_depth(3);
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if !entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(".itch.toml")
        {
            continue;
        }
        let text = fs::read_to_string(entry.path()).ok()?;
        if let Some(path) = toml_play_path(&text) {
            if path.starts_with("http://") || path.starts_with("https://") {
                return None;
            }
            let full = if Path::new(&path).is_absolute() {
                PathBuf::from(path)
            } else {
                install.join(path)
            };
            if full.is_file() {
                return Some(full.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn toml_play_path(text: &str) -> Option<String> {
    let re = Regex::new(
        r#"(?is)\[\[actions\]\][^\[]*?name\s*=\s*"play"[^\[]*?path\s*=\s*"([^"]+)""#,
    )
    .ok()?;
    if let Some(c) = re.captures(text) {
        return Some(c.get(1)?.as_str().to_string());
    }
    let re2 = Regex::new(
        r#"(?is)\[\[actions\]\][^\[]*?path\s*=\s*"([^"]+)"[^\[]*?name\s*=\s*"play""#,
    )
    .ok()?;
    re2.captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn launch_from_verdict(install: &Path, verdict: &str) -> Option<String> {
    if verdict.is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(verdict).ok()?;
    let cands = v.get("candidates")?.as_array()?;
    let mut best: Option<(i64, PathBuf)> = None;
    for c in cands {
        let flavor = c.get("flavor").and_then(|x| x.as_str()).unwrap_or("");
        if flavor != "windows" {
            continue;
        }
        if c.pointer("/windowsInfo/uninstaller")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let rel = c.get("path")?.as_str()?;
        let full = install.join(rel.replace('/', "\\"));
        if !full.is_file() {
            continue;
        }
        let depth = c.get("depth").and_then(|x| x.as_i64()).unwrap_or(99);
        match &best {
            None => best = Some((depth, full)),
            Some((d, _)) if depth < *d => best = Some((depth, full)),
            _ => {}
        }
    }
    best.map(|(_, p)| p.to_string_lossy().to_string())
}
