use crate::covers;
use crate::db::{self, Database};
use crate::fps;
use crate::launch;
use crate::models::{AppSettings, FpsSample, Game, GameStats, LibraryStats};
use crate::scanners;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub db: Mutex<Database>,
}

fn with_db<T, F>(state: &State<AppState>, f: F) -> Result<T, String>
where
    F: FnOnce(&Database) -> anyhow::Result<T>,
{
    let db = state.db.lock().map_err(|e| e.to_string())?;
    f(&db).map_err(|e| format!("{e:#}"))
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
    let discovered = scanners::scan_all().map_err(|e| format!("{e:#}"))?;
    with_db(&state, |db| {
        db.upsert_discovered(&discovered)?;
        db.list_games(false)
    })
}

#[tauri::command]
pub fn launch_game(app: AppHandle, state: State<AppState>, id: String) -> Result<Game, String> {
    let game = with_db(&state, |db| {
        db.record_launch(&id)?;
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    launch::launch_game(&game).map_err(|e| format!("{e:#}"))?;
    fps::start_monitoring(app, game.id.clone(), game.launch_target.clone());
    Ok(game)
}

#[tauri::command]
pub fn stop_fps_monitor() -> Result<(), String> {
    fps::stop_monitoring();
    Ok(())
}

#[tauri::command]
pub fn end_play_session(
    state: State<AppState>,
    id: String,
    minutes: i64,
    avg_fps: Option<f64>,
) -> Result<Game, String> {
    fps::stop_monitoring();
    with_db(&state, |db| db.end_session_and_add_playtime(&id, minutes, avg_fps))
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
    launch::open_folder(&path).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn add_manual_game(state: State<AppState>, path: String) -> Result<Game, String> {
    let discovered = scanners::create_manual_from_path(&path).map_err(|e| format!("{e:#}"))?;
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
}

/// Fetch missing covers in a background thread so the UI never freezes.
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
                // Only fetch when missing / broken — do NOT re-read every cover file
                // (that was freezing the UI for several seconds on startup).
                match &g.cover_path {
                    None => true,
                    Some(p) => !std::path::Path::new(p).exists(),
                }
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
            }

            let _ = app.emit(
                "cover-updated",
                CoverUpdatedPayload {
                    id: game.id.clone(),
                    cover_path: fetched.path,
                    steam_app_id: fetched.steam_app_id,
                    cover_url: None,
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
    let dest = covers::import_cover_file(&id, &path).map_err(|e| format!("{e:#}"))?;
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
            .map_err(|e| format!("{e:#}"));
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
    let filter: Option<std::collections::HashSet<String>> =
        ids.map(|v| v.into_iter().collect());

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
pub fn log_fps(
    state: State<AppState>,
    id: String,
    fps: f64,
    note: Option<String>,
) -> Result<FpsSample, String> {
    with_db(&state, |db| db.add_fps_sample(&id, fps, note.as_deref()))
}

#[tauri::command]
pub fn record_live_fps(state: State<AppState>, id: String, fps: f64) -> Result<(), String> {
    if !(0.0..=1000.0).contains(&fps) || fps == 0.0 {
        return Ok(());
    }
    with_db(&state, |db| {
        db.add_fps_sample(&id, fps, Some("live"))?;
        Ok(())
    })
}

#[tauri::command]
pub fn app_data_path() -> Result<String, String> {
    Ok(db::default_db_path()
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default())
}
