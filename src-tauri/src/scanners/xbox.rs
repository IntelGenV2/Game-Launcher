use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discover Xbox / PC Game Pass installs across every drive.
/// Supports both `XboxGames` and `Xbox Games` folder names (user has F:\Xbox Games).
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for root in xbox_roots() {
        if !root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let raw_name = entry.file_name().to_string_lossy().to_string();
            if raw_name.eq_ignore_ascii_case("GameSave")
                || raw_name.eq_ignore_ascii_case("Content")
                || raw_name.starts_with('.')
            {
                continue;
            }
            let name = humanize_folder_name(&raw_name);
            let id_key = raw_name.to_lowercase();
            if !seen.insert(id_key) {
                continue;
            }

            let exe = find_best_exe(&path, &raw_name);
            let launch_target = exe
                .clone()
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            games.push(DiscoveredGame {
                id: format!("xbox:{}", slug(&raw_name)),
                name,
                store: Store::Xbox,
                launch_target,
                install_path: Some(path.to_string_lossy().to_string()),
                steam_app_id: None,
                playtime_minutes: None,
            });
        }
    }
    Ok(games)
}

fn xbox_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    // All fixed/removable drive letters
    for letter in b'C'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if !Path::new(&drive).exists() {
            continue;
        }
        roots.push(PathBuf::from(format!("{}XboxGames", drive)));
        roots.push(PathBuf::from(format!("{}Xbox Games", drive)));
        roots.push(PathBuf::from(format!("{}Games\\Xbox", drive)));
    }
    if let Some(local) = dirs::data_local_dir() {
        roots.push(local.join("Microsoft").join("XboxGames"));
        roots.push(local.join("Microsoft").join("Xbox Games"));
    }
    roots
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn humanize_folder_name(name: &str) -> String {
    // Keep names that already have spaces
    if name.contains(' ') {
        return name.to_string();
    }
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_ascii_digit() && c.is_alphabetic())
                || (prev.is_alphabetic() && c.is_ascii_digit())
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(|n| n.is_lowercase()));
            if boundary && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_best_exe(game_dir: &Path, game_name: &str) -> Option<String> {
    let mut candidates: Vec<(i32, PathBuf)> = Vec::new();
    let search_dirs = [
        game_dir.to_path_buf(),
        game_dir.join("Content"),
        game_dir.join("Binaries"),
        game_dir.join("bin"),
    ];

    for dir in &search_dirs {
        if !dir.is_dir() {
            continue;
        }
        let depth = if dir == game_dir { 2 } else { 3 };
        for entry in WalkDir::new(dir).max_depth(depth).into_iter().filter_map(|e| e.ok()) {
            let path = entry.into_path();
            if !path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
            {
                continue;
            }
            let file = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let mut score = 0i32;
            let compact_game: String = game_name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_lowercase();
            if !compact_game.is_empty() && file.replace([' ', '_', '-'], "").contains(&compact_game)
            {
                score -= 50;
            }
            if file.contains("forza")
                || file.contains("minecraft")
                || file == compact_game
                || compact_game.contains(&file.replace([' ', '_', '-'], ""))
            {
                score -= 40;
            }
            if file.contains("gamelaunchhelper") {
                score += 5; // usable but prefer real game exe
            }
            if file.contains("unitycrash")
                || file.contains("crash")
                || file.contains("redist")
                || file.contains("unins")
                || file.contains("setup")
                || file.contains("vcredist")
                || file.contains("easervices")
            {
                score += 100;
            }
            // Prefer shallower paths
            score += path.components().count() as i32;
            candidates.push((score, path));
        }
    }

    candidates.sort_by_key(|(s, p)| (*s, p.to_string_lossy().to_string()));
    candidates
        .first()
        .map(|(_, p)| p.to_string_lossy().to_string())
}
