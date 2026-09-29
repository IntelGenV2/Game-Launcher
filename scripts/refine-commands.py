from pathlib import Path
p=Path('src-tauri/src/commands.rs')
s=p.read_text(encoding='utf-8')
s=s.replace('AtomicBool, AtomicU64, Ordering','AtomicBool, Ordering')
s=s.replace('    /// Bumped each time a hide-while-playing watch starts so older watchers stop.\n    pub hide_watch_gen: AtomicU64,','    pub sessions: Mutex<std::collections::HashMap<String, crate::models::LaunchStatus>>,\n    pub maintenance: AtomicBool,\n    pub operation: Mutex<()>,')
start=s.index('#[tauri::command]\npub fn launch_game(')
end=s.index('#[tauri::command]\npub fn toggle_favorite',start)
s=s[:start]+'''#[tauri::command]
pub fn launch_game(app: AppHandle, state: State<AppState>, id: String, source_id: Option<String>) -> Result<Game, String> {
    game_watch::start(&app, &state, &id, source_id.as_deref())
}

#[tauri::command]
pub fn launch_statuses(state: State<AppState>) -> Result<Vec<crate::models::LaunchStatus>, String> {
    Ok(state.sessions.lock().map_err(|e|e.to_string())?.values().cloned().collect())
}

#[tauri::command]
pub fn set_pinned(state: State<AppState>, id: String, pinned: bool) -> Result<Game, String> {
    with_db(&state, |db|db.set_pinned(&id,pinned))
}

#[tauri::command]
pub fn set_preferred_copy(state: State<AppState>, id: String, source_id: String) -> Result<Game, String> {
    with_db(&state, |db|db.set_preferred_copy(&id,&source_id))
}

'''+s[end:]
start=s.index('        "play" => {',s.index('pub fn launch_action'))
end=s.index('        "save_folder"',start)
s=s[:start]+'''        "play" => { return game_watch::start(&app, &state, &id, None); }
'''+s[end:]
s=s.replace('pub fn export_backup(dest: String) -> Result<String, String> {\n    backup::export_library_zip(&dest).map_err(user_err)\n}', '''pub fn export_backup(state: State<AppState>, dest: String) -> Result<String, String> {
    with_db(&state, |db|backup::export_library_zip(db,&dest))
}

#[tauri::command]
pub fn preview_backup(path: String) -> Result<backup::BackupPreview,String> {
    backup::preview(&path).map_err(user_err)
}

#[tauri::command]
pub fn restore_backup(state:State<AppState>,path:String) -> Result<String,String> {
    backup::stage_restore(&state,&path).map_err(user_err)
}

#[tauri::command]
pub fn open_backups() -> Result<(),String> {
    let path=db::app_data_dir().join("backups");
    std::fs::create_dir_all(&path).map_err(user_err)?;
    launch::open_folder(&path.to_string_lossy()).map_err(user_err)
}''')
s=s.replace('    with_db(&state, |db| db.merge_games(&keep_id, &source_ids))','    let _operation=state.operation.lock().map_err(user_err)?;\n    if game_watch::active(&state) { return Err("Finish running games before linking copies".into()); }\n    with_db(&state, |db| db.merge_games(&keep_id, &source_ids))')
start=s.index('#[tauri::command]\npub fn reset_all_art(')
end=s.index('#[tauri::command]\npub fn reset_app',start)
s=s[:start]+'''#[tauri::command]
pub fn reset_art(state: State<AppState>, custom: bool) -> Result<(), String> {
    if COVER_FETCH_RUNNING.load(Ordering::SeqCst) { return Err("Wait for artwork downloads to finish, then retry.".into()); }
    let paths = with_db(&state, |db| db.clear_art(custom))?;
    let root=db::covers_dir().canonicalize().map_err(user_err)?;
    for raw in paths {
        if let Ok(path)=std::path::Path::new(&raw).canonicalize() {
            if path.starts_with(&root) && path.is_file() { std::fs::remove_file(path).map_err(user_err)?; }
        }
    }
    covers::invalidate_cover_catalog();
    Ok(())
}

#[tauri::command]
pub fn reset_all_stats(state: State<AppState>) -> Result<(), String> {
    let _operation=state.operation.lock().map_err(user_err)?;
    if game_watch::active(&state) { return Err("Finish running games before resetting history".into()); }
    with_db(&state, |db| db.clear_all_stats())
}

'''+s[end:]
s=s.replace('pub fn remove_game(state: State<AppState>, id: String) -> Result<Game, String> {','pub fn remove_game(state: State<AppState>, id: String) -> Result<Game, String> {\n    let _operation=state.operation.lock().map_err(user_err)?;\n    if game_watch::active(&state) { return Err("Finish running games before removing library entries".into()); }')
p.write_text(s,encoding='utf-8')
p=Path('src-tauri/src/lib.rs')
s=p.read_text(encoding='utf-8')
s=s.replace('    db::apply_pending_reset();','    db::apply_pending_reset();\n    if let Err(error)=backup::apply_pending_restore() { eprintln!("Restore was not applied: {error}"); }')
s=s.replace('    let start_hidden =','    database.recover_sessions().expect("recover interrupted sessions");\n    let start_hidden =')
s=s.replace('            hide_watch_gen: std::sync::atomic::AtomicU64::new(0),','            sessions: Mutex::new(std::collections::HashMap::new()),\n            maintenance: std::sync::atomic::AtomicBool::new(false),\n            operation: Mutex::new(()),')
s=s.replace('            commands::end_play_session,','            commands::launch_statuses,\n            commands::set_pinned,\n            commands::set_preferred_copy,')
s=s.replace('            commands::export_backup,','            commands::export_backup,\n            commands::preview_backup,\n            commands::restore_backup,\n            commands::open_backups,')
s=s.replace('            commands::reset_all_art,','            commands::reset_art,')
s=s.replace('            let handle = app.handle().clone();','            let handle = app.handle().clone();\n            backup::start_automatic_backups(handle.clone());')
p.write_text(s,encoding='utf-8')
