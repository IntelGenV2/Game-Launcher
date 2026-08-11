use crate::covers;
use crate::db::{self, Database};
use crate::launch;
use crate::models::{AppSettings, Game, GameGroup, GameStats, LibraryStats};
use crate::scanners;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub db: Mutex<Database>,
}

fn user_err(e: impl std::fmt::Display) -> String {
    let full = e.to_string();
    // Prefer the root cause line; never ship multi-line anyhow chains to the UI.
    full.lines()
        .next()
        .unwrap_or("Something went wrong")
        .trim()
        .to_string()
}

fn with_db<T, F>(state: &State<AppState>, f: F) -> Result<T, String>
where
    F: FnOnce(&Database) -> anyhow::Result<T>,
{
    let db = state.db.lock().map_err(|e| e.to_string())?;
    f(&db).map_err(user_err)
}

#[tauri::command]
pub fn list_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    with_db(&state, |db| db.list_games(false))
}

#[tauri::command]
pub fn list_hidden_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    with_db(&state, |db| {
        let all = db.list_games(true)?;
        Ok(all.into_iter().filter(|g| g.hidden).collect())
    })
}

#[tauri::command]
pub fn rescan_library(state: State<AppState>) -> Result<Vec<Game>, String> {
    let discovered = scanners::scan_all().map_err(user_err)?;
    with_db(&state, |db| {
        db.upsert_discovered(&discovered)?;
        db.list_games(false)
    })
}

#[tauri::command]
pub fn launch_game(state: State<AppState>, id: String) -> Result<Game, String> {
    let game = with_db(&state, |db| {
        db.record_launch(&id)?;
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    launch::launch_game(&game).map_err(user_err)?;
    Ok(game)
}

#[tauri::command]
pub fn end_play_session(state: State<AppState>, id: String, minutes: i64) -> Result<Game, String> {
    with_db(&state, |db| db.end_session_and_add_playtime(&id, minutes))
}

#[tauri::command]
pub fn toggle_favorite(state: State<AppState>, id: String) -> Result<Game, String> {
    with_db(&state, |db| db.toggle_favorite(&id))
}

#[tauri::command]
pub fn set_hidden(state: State<AppState>, id: String, hidden: bool) -> Result<Game, String> {
    with_db(&state, |db| db.set_hidden(&id, hidden))
}

#[tauri::command]
pub fn open_install_folder(state: State<AppState>, id: String) -> Result<(), String> {
    let game = with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    let path = game
        .install_path
        .ok_or_else(|| "No install path".to_string())?;
    launch::open_folder(&path).map_err(user_err)
}

#[tauri::command]
pub fn add_manual_game(state: State<AppState>, path: String) -> Result<Game, String> {
    let discovered = scanners::create_manual_from_path(&path).map_err(user_err)?;
    with_db(&state, |db| {
        db.add_manual_game(
            &discovered.id,
            &discovered.name,
            &discovered.launch_target,
            discovered.install_path.as_deref(),
        )
    })
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    with_db(&state, |db| db.get_settings())
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: AppSettings) -> Result<(), String> {
    with_db(&state, |db| db.save_settings(&settings))
}

#[tauri::command]
pub fn library_stats(state: State<AppState>) -> Result<LibraryStats, String> {
    with_db(&state, |db| {
        let games = db.list_games(true)?;
        Ok(LibraryStats {
            total: games.iter().filter(|g| !g.hidden).count(),
            favorites: games.iter().filter(|g| g.favorite && !g.hidden).count(),
            missing: games.iter().filter(|g| g.missing && !g.hidden).count(),
        })
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoverUpdatedPayload {
    id: String,
    cover_path: String,
    steam_app_id: Option<String>,
    cover_url: Option<String>,
    genre: Option<String>,
}

/// Fetch missing covers / genre metadata in a background thread so the UI never freezes.
/// Emits `cover-updated` per game and `covers-done` when finished.
#[tauri::command]
pub fn fetch_covers(
    app: AppHandle,
    state: State<AppState>,
    ids: Option<Vec<String>>,
) -> Result<(), String> {
    let (games, api_key) = with_db(&state, |db| {
        let settings = db.get_settings()?;
        let games = db.list_games(true)?;
        Ok((games, settings.steam_grid_db_api_key))
    })?;

    let targets: Vec<Game> = match ids {
        Some(ids) => games.into_iter().filter(|g| ids.contains(&g.id)).collect(),
        None => games
            .into_iter()
            .filter(|g| {
                if g.hidden {
                    return false;
                }
                let need_cover = match &g.cover_path {
                    None => true,
                    Some(p) => !std::path::Path::new(p).exists(),
                };
                let need_genre = g
                    .genre
                    .as_ref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true);
                need_cover || need_genre
            })
            .take(40)
            .collect(),
    };

    if targets.is_empty() {
        let _ = app.emit("covers-done", ());
        return Ok(());
    }

    std::thread::spawn(move || {
        for game in targets {
            let fetched = match covers::ensure_cover(&game, api_key.as_deref()) {
                Ok(Some(f)) => f,
                _ => continue,
            };

            {
                let Some(state) = app.try_state::<AppState>() else {
                    break;
                };
                let Ok(db) = state.db.lock() else {
                    break;
                };
                // Don't store legacy CDN library_600x900 URLs — many modern titles 404.
                // The UI loads the local cover_path as a data URL instead.
                let _ = db.set_cover(&game.id, None, Some(&fetched.path));
                if let Some(steam_id) = &fetched.steam_app_id {
                    let _ = db.set_steam_app_id(&game.id, steam_id);
                }
                if let Some(genre) = &fetched.genre {
                    let _ = db.set_genre(&game.id, genre);
                }
            }

            let _ = app.emit(
                "cover-updated",
                CoverUpdatedPayload {
                    id: game.id.clone(),
                    cover_path: fetched.path,
                    steam_app_id: fetched.steam_app_id,
                    cover_url: None,
                    genre: fetched.genre,
                },
            );
        }
        let _ = app.emit("covers-done", ());
    });

    Ok(())
}

#[tauri::command]
pub fn set_game_path(state: State<AppState>, id: String, path: String) -> Result<Game, String> {
    with_db(&state, |db| db.set_game_path(&id, &path))
}

#[tauri::command]
pub fn set_game_name(state: State<AppState>, id: String, name: String) -> Result<Game, String> {
    with_db(&state, |db| db.set_game_name(&id, &name))
}

#[tauri::command]
pub fn set_custom_cover(state: State<AppState>, id: String, path: String) -> Result<Game, String> {
    let dest = covers::import_cover_file(&id, &path).map_err(user_err)?;
    with_db(&state, |db| {
        db.set_cover(&id, None, Some(&dest))?;
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })
}

#[tauri::command]
pub fn remove_game(state: State<AppState>, id: String) -> Result<(), String> {
    // Delete cached cover files for this game
    let game = with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    if let Some(path) = &game.cover_path {
        let _ = std::fs::remove_file(path);
    }
    let safe = id.replace([':', '/', '\\'], "_");
    if let Ok(entries) = std::fs::read_dir(db::covers_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&safe) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    with_db(&state, |db| db.remove_game(&id))
}

#[tauri::command]
pub fn get_cover_data_url(state: State<AppState>, id: String) -> Result<Option<String>, String> {
    let game = with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    if let Some(path) = &game.cover_path {
        return covers::cover_as_data_url(path)
            .map(Some)
            .map_err(user_err);
    }
    if let Some(url) = &game.cover_url {
        return Ok(Some(url.clone()));
    }
    Ok(None)
}

/// Batch-load local covers for the home grid (avoids N round-trips + cancel races).
#[tauri::command]
pub fn get_cover_data_urls(
    state: State<AppState>,
    ids: Option<Vec<String>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let games = with_db(&state, |db| db.list_games(true))?;
    let filter: Option<std::collections::HashSet<String>> = ids.map(|v| v.into_iter().collect());

    let mut out = std::collections::HashMap::new();
    for g in games {
        if let Some(ref want) = filter {
            if !want.contains(&g.id) {
                continue;
            }
        }
        let Some(path) = g.cover_path.as_deref() else {
            continue;
        };
        if let Ok(data) = covers::cover_as_data_url(path) {
            out.insert(g.id, data);
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn get_game_stats(state: State<AppState>, id: String) -> Result<GameStats, String> {
    with_db(&state, |db| db.game_stats(&id))
}

#[tauri::command]
pub fn app_data_path() -> Result<String, String> {
    Ok(db::default_db_path()
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default())
}

#[tauri::command]
pub fn list_groups(state: State<AppState>) -> Result<Vec<GameGroup>, String> {
    with_db(&state, |db| db.list_groups())
}

#[tauri::command]
pub fn create_group(
    state: State<AppState>,
    name: String,
    game_ids: Vec<String>,
) -> Result<GameGroup, String> {
    with_db(&state, |db| db.create_group(&name, &game_ids))
}

#[tauri::command]
pub fn rename_group(state: State<AppState>, id: String, name: String) -> Result<GameGroup, String> {
    with_db(&state, |db| db.rename_group(&id, &name))
}

#[tauri::command]
pub fn delete_group(state: State<AppState>, id: String) -> Result<(), String> {
    with_db(&state, |db| db.delete_group(&id))
}

#[tauri::command]
pub fn add_game_to_group(
    state: State<AppState>,
    group_id: String,
    game_id: String,
) -> Result<GameGroup, String> {
    with_db(&state, |db| db.add_game_to_group(&group_id, &game_id))
}

#[tauri::command]
pub fn remove_game_from_group(
    state: State<AppState>,
    group_id: String,
    game_id: String,
) -> Result<GameGroup, String> {
    with_db(&state, |db| db.remove_game_from_group(&group_id, &game_id))
}
