use crate::backup;
use crate::covers;
use crate::db::{self, Database};
use crate::game_watch;
use crate::launch;
use crate::metadata;
use crate::models::{
    AppSettings, CoverChoiceGroup, DuplicateGroup, Game, GameGroup, GameStats, LibraryOverview,
    LibraryStats, PlayniteImportResult, ScanStoreProgress,
};
use crate::playnite;
use crate::scanners;
use crate::system_info;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub db: Mutex<Database>,
    /// Bumped each time a hide-while-playing watch starts so older watchers stop.
    pub hide_watch_gen: AtomicU64,
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
pub fn get_game(state: State<AppState>, id: String) -> Result<Game, String> {
    with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })
}

#[tauri::command]
pub fn list_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    covers::warm_cover_catalog();
    with_db(&state, |db| {
        let mut games = db.list_games(false)?;
        covers::reattach_local_covers(&mut games, |id, path| {
            if let Some(p) = path {
                let _ = db.set_cover(id, None, Some(p), None);
            } else {
                let _ = db.clear_cover_path(id);
            }
        });
        Ok(games)
    })
}

#[tauri::command]
pub fn list_hidden_games(state: State<AppState>) -> Result<Vec<Game>, String> {
    covers::warm_cover_catalog();
    with_db(&state, |db| {
        let mut all = db.list_games(true)?;
        covers::reattach_local_covers(&mut all, |id, path| {
            if let Some(p) = path {
                let _ = db.set_cover(id, None, Some(p), None);
            } else {
                let _ = db.clear_cover_path(id);
            }
        });
        Ok(all.into_iter().filter(|g| g.hidden).collect())
    })
}

#[tauri::command]
pub fn rescan_library(app: AppHandle, state: State<AppState>) -> Result<Vec<Game>, String> {
    let discovered = scanners::scan_all(|ev: ScanStoreProgress| {
        let _ = app.emit("scan-progress", ev);
    })
    .map_err(user_err)?;
    let _ = app.emit(
        "scan-progress",
        ScanStoreProgress {
            store: "Library".into(),
            status: "saving".into(),
            count: discovered.len(),
            message: None,
        },
    );
    covers::warm_cover_catalog();
    with_db(&state, |db| {
        db.upsert_discovered(&discovered)?;
        let mut games = db.list_games(false)?;
        covers::reattach_local_covers(&mut games, |id, path| {
            if let Some(p) = path {
                let _ = db.set_cover(id, None, Some(p), None);
            } else {
                let _ = db.clear_cover_path(id);
            }
        });
        Ok(games)
    })
}

#[tauri::command]
pub fn launch_game(app: AppHandle, state: State<AppState>, id: String) -> Result<Game, String> {
    let game = with_db(&state, |db| {
        db.record_launch(&id)?;
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    launch::launch_game(&game).map_err(user_err)?;
    game_watch::maybe_hide_while_playing(&app, &state, &game);
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
    with_db(&state, |db| db.save_settings(&settings))?;
    let start = settings.start_with_windows.unwrap_or(false);
    // Always start visible; tray/background is no longer a separate setting.
    launch::set_start_with_windows(start, false).map_err(user_err)?;
    Ok(())
}

#[tauri::command]
pub fn library_stats(state: State<AppState>) -> Result<LibraryStats, String> {
    with_db(&state, |db| {
        let games = db.list_games(true)?;
        let visible: Vec<_> = games.iter().filter(|g| !g.hidden).collect();
        Ok(LibraryStats {
            total: visible.len(),
            favorites: visible.iter().filter(|g| g.favorite).count(),
            missing: visible.iter().filter(|g| g.missing).count(),
            total_playtime_minutes: visible.iter().map(|g| g.playtime_minutes).sum(),
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
    logo_path: Option<String>,
    cover_source: Option<String>,
}

static COVER_FETCH_RUNNING: AtomicBool = AtomicBool::new(false);

/// Fetch missing covers / genre metadata in a background thread so the UI never freezes.
/// Emits `cover-updated` per game and `covers-done` when finished.
#[tauri::command]
pub fn fetch_covers(
    app: AppHandle,
    state: State<AppState>,
    ids: Option<Vec<String>>,
) -> Result<(), String> {
    covers::warm_cover_catalog();
    let (games, api_key, attached) = with_db(&state, |db| {
        let settings = db.get_settings()?;
        let mut games = db.list_games(true)?;
        let attached = covers::reattach_local_covers(&mut games, |id, path| {
            if let Some(p) = path {
                let _ = db.set_cover(id, None, Some(p), None);
            } else {
                let _ = db.clear_cover_path(id);
            }
        });
        Ok((games, settings.steam_grid_db_api_key, attached))
    })?;

    for (id, path) in attached {
        let steam_id = games.iter().find(|g| g.id == id).and_then(|g| g.steam_app_id.clone());
        let _ = app.emit(
            "cover-updated",
            CoverUpdatedPayload {
                id,
                cover_path: path,
                steam_app_id: steam_id.clone(),
                cover_url: None,
                genre: None,
                logo_path: None,
                cover_source: None,
            },
        );
    }

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
                    Some(p) => !covers::is_portrait_cover_path(p),
                };
                let need_genre = g
                    .genre
                    .as_ref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true);
                need_cover || need_genre
            })
            .collect(),
    };

    if targets.is_empty() {
        let _ = app.emit("covers-done", ());
        return Ok(());
    }

    if COVER_FETCH_RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    std::thread::spawn(move || {
        struct FetchGuard;
        impl Drop for FetchGuard {
            fn drop(&mut self) {
                COVER_FETCH_RUNNING.store(false, Ordering::SeqCst);
            }
        }
        let _guard = FetchGuard;
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
                let has_file = !fetched.path.is_empty() && std::path::Path::new(&fetched.path).exists();
                if has_file {
                    let _ = db.set_cover(
                        &game.id,
                        None,
                        Some(&fetched.path),
                        fetched.source.as_deref(),
                    );
                } else if let Some(steam_id) = fetched.steam_app_id.as_ref().or(game.steam_app_id.as_ref())
                {
                    let cdn = format!(
                        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{steam_id}/library_600x900.jpg"
                    );
                    let _ = db.set_cover(&game.id, Some(&cdn), None, Some("Steam"));
                }
                if let Some(steam_id) = &fetched.steam_app_id {
                    let _ = db.set_steam_app_id(&game.id, steam_id);
                }
                if let Some(genre) = &fetched.genre {
                    let _ = db.set_genre(&game.id, genre);
                }
                drop(db);
                let steam_id = fetched.steam_app_id.clone().or(game.steam_app_id.clone());
                let cover_path = if has_file {
                    fetched.path
                } else {
                    game.cover_path.clone().unwrap_or_default()
                };
                let _ = app.emit(
                    "cover-updated",
                    CoverUpdatedPayload {
                        id: game.id.clone(),
                        cover_path,
                        steam_app_id: steam_id.clone(),
                        cover_url: None,
                        genre: fetched.genre,
                        logo_path: None,
                        cover_source: fetched.source,
                    },
                );
            }
        }
        COVER_FETCH_RUNNING.store(false, Ordering::SeqCst);
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
        db.set_cover(&id, None, Some(&dest), Some("Custom"))?;
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })
}

#[tauri::command]
pub fn list_cover_choice_groups(state: State<AppState>) -> Result<Vec<CoverChoiceGroup>, String> {
    covers::warm_cover_catalog();
    let games = with_db(&state, |db| db.list_games(false))?;
    Ok(covers::cover_choice_groups(&games))
}

#[tauri::command]
pub fn list_cover_choices(state: State<AppState>, id: String) -> Result<CoverChoiceGroup, String> {
    with_db(&state, |db| {
        let game = db
            .get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;
        let paths = covers::cover_alternates_for(&game);
        Ok(CoverChoiceGroup {
            game_id: game.id,
            name: game.name,
            current_path: game.cover_path,
            paths: paths
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
        })
    })
}

#[tauri::command]
pub fn choose_cover(state: State<AppState>, id: String, path: String) -> Result<Game, String> {
    with_db(&state, |db| {
        let game = db
            .get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;
        if !covers::path_is_cover_choice(&game, &path) {
            anyhow::bail!("That image is not a cover for this game");
        }
        if !covers::is_portrait_cover_path(&path) {
            anyhow::bail!("Cover must be portrait box art (taller than wide)");
        }
        if !std::path::Path::new(&path).is_file() {
            anyhow::bail!("Cover file not found");
        }
        db.set_cover(&id, None, Some(&path), Some("Local"))?;
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })
}

#[tauri::command]
pub fn remove_game(state: State<AppState>, id: String) -> Result<Game, String> {
    let game = with_db(&state, |db| {
        let game = db
            .get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;
        db.remove_game(&id)?;
        Ok(game)
    })?;
    Ok(game)
}

#[tauri::command]
pub fn restore_game(state: State<AppState>, game: Game) -> Result<Game, String> {
    with_db(&state, |db| db.insert_game_full(&game))
}

#[tauri::command]
pub fn finalize_remove(state: State<AppState>, id: String) -> Result<(), String> {
    let gone = with_db(&state, |db| {
        db.finalize_remove(&id)?;
        Ok(db.get_game(&id)?.is_none())
    })?;
    if gone {
        let safe = id.replace([':', '/', '\\'], "_");
        if let Ok(entries) = std::fs::read_dir(db::covers_dir()) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&safe) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_cover_data_url(_state: State<AppState>, _id: String) -> Result<Option<String>, String> {
    Ok(None)
}

/// Batch-load local covers for the home grid (avoids N round-trips + cancel races).
/// Intentionally empty: encoding every cover as base64 OOMs the WebView.
#[tauri::command]
pub fn get_cover_data_urls(
    _state: State<AppState>,
    _ids: Option<Vec<String>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(std::collections::HashMap::new())
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

#[tauri::command]
pub fn set_notes(state: State<AppState>, id: String, notes: String) -> Result<Game, String> {
    with_db(&state, |db| db.set_notes(&id, &notes))
}

#[tauri::command]
pub fn set_tags(state: State<AppState>, id: String, tags: Vec<String>) -> Result<Game, String> {
    with_db(&state, |db| db.set_tags(&id, &tags))
}

#[tauri::command]
pub fn set_launch_options(
    state: State<AppState>,
    id: String,
    launch_args: Option<String>,
    working_dir: Option<String>,
    run_as_admin: bool,
    save_folder: Option<String>,
) -> Result<Game, String> {
    with_db(&state, |db| {
        db.set_launch_options(
            &id,
            launch_args.as_deref(),
            working_dir.as_deref(),
            run_as_admin,
            save_folder.as_deref(),
        )
    })
}

#[tauri::command]
pub fn fetch_game_metadata(state: State<AppState>, id: String) -> Result<Game, String> {
    let (game, key) = with_db(&state, |db| {
        let g = db
            .get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;
        let key = db.get_settings()?.steam_grid_db_api_key;
        Ok((g, key))
    })?;
    let meta = metadata::fetch_for_game(&game, key.as_deref()).map_err(user_err)?;
    with_db(&state, |db| db.apply_metadata(&id, &meta))
}

#[tauri::command]
pub fn set_custom_logo(state: State<AppState>, id: String, _path: String) -> Result<Game, String> {
    with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })
}

#[tauri::command]
pub fn get_logo_data_url(_state: State<AppState>, _id: String) -> Result<Option<String>, String> {
    Ok(None)
}

#[tauri::command]
pub fn get_logo_data_urls(
    _state: State<AppState>,
    _ids: Option<Vec<String>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(std::collections::HashMap::new())
}

#[tauri::command]
pub fn launch_action(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    action: String,
) -> Result<Game, String> {
    let game = with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })?;
    match action.as_str() {
        "play" => {
            with_db(&state, |db| {
                db.record_launch(&id)?;
                Ok(())
            })?;
            launch::launch_game(&game).map_err(user_err)?;
            game_watch::maybe_hide_while_playing(&app, &state, &game);
        }
        "save_folder" => {
            let path = game
                .save_folder
                .as_deref()
                .ok_or_else(|| "No save folder set for this game".to_string())?;
            launch::open_folder(path).map_err(user_err)?;
        }
        _ => return Err("Unknown action".into()),
    }
    with_db(&state, |db| {
        db.get_game(&id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    })
}

#[tauri::command]
pub fn library_overview(state: State<AppState>) -> Result<LibraryOverview, String> {
    with_db(&state, |db| db.library_overview())
}

#[tauri::command]
pub fn suggest_duplicates(state: State<AppState>) -> Result<Vec<DuplicateGroup>, String> {
    with_db(&state, |db| db.suggest_duplicates())
}

#[tauri::command]
pub fn merge_games(
    state: State<AppState>,
    keep_id: String,
    source_ids: Vec<String>,
) -> Result<Game, String> {
    with_db(&state, |db| db.merge_games(&keep_id, &source_ids))
}

#[tauri::command]
pub fn export_backup(dest: String) -> Result<String, String> {
    backup::export_library_zip(&dest).map_err(user_err)
}

#[tauri::command]
pub fn import_playnite(
    state: State<AppState>,
    path: String,
) -> Result<PlayniteImportResult, String> {
    let (imported, mut result) = playnite::load_from_path(&path).map_err(user_err)?;
    with_db(&state, |db| {
        let existing = db.list_games(true)?;
        for item in imported {
            let key = playnite::normalize_name(&item.name);
            if let Some(found) = existing.iter().find(|g| playnite::normalize_name(&g.name) == key)
            {
                db.apply_playnite_fields(
                    &found.id,
                    item.playtime_minutes,
                    item.favorite,
                    item.hidden,
                    item.notes.as_deref(),
                    &item.tags,
                    item.developer.as_deref(),
                    item.publisher.as_deref(),
                    item.release_year,
                    item.description.as_deref(),
                    &item.genres,
                    item.steam_app_id.as_deref(),
                )?;
                result.updated += 1;
            } else if let Some(disc) = playnite::create_discovered(&item) {
                let added = db.add_manual_game(
                    &disc.id,
                    &item.name,
                    &disc.launch_target,
                    disc.install_path.as_deref(),
                )?;
                db.apply_playnite_fields(
                    &added.id,
                    item.playtime_minutes,
                    item.favorite,
                    item.hidden,
                    item.notes.as_deref(),
                    &item.tags,
                    item.developer.as_deref(),
                    item.publisher.as_deref(),
                    item.release_year,
                    item.description.as_deref(),
                    &item.genres,
                    item.steam_app_id.as_deref(),
                )?;
                result.added += 1;
            } else {
                result.skipped += 1;
            }
        }
        Ok(result)
    })
}

#[tauri::command]
pub fn default_playnite_path() -> Result<Option<String>, String> {
    let dir = dirs::data_dir()
        .map(|p| p.join("Playnite"))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string());
    Ok(dir)
}

#[tauri::command]
pub fn started_in_background() -> bool {
    std::env::args().any(|a| a == "--background")
}

#[tauri::command]
pub fn system_info() -> system_info::SystemInfo {
    system_info::collect()
}

#[tauri::command]
pub fn reset_all_art(state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| db.clear_all_covers())?;
    let dir = db::covers_dir();
    if dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
    covers::invalidate_cover_catalog();
    Ok(())
}

#[tauri::command]
pub fn reset_all_stats(state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| db.clear_all_stats())
}

#[tauri::command]
pub fn reset_app() -> Result<(), String> {
    let dir = db::app_data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(".reset"), b"1").map_err(|e| e.to_string())?;
    Ok(())
}
