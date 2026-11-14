use crate::{commands::AppState, launch, models::{Game, LaunchStatus}};
use std::{path::{Path, PathBuf}, thread, time::{Duration, Instant}};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter, Manager};

pub fn active(state: &AppState) -> bool {
    state.sessions.lock().map(|sessions| sessions.values().any(|s| s.state == "launching" || s.state == "running")).unwrap_or(true)
}

fn publish(app: &AppHandle, id: &str, source: &str, phase: &str, message: Option<String>) {
    let status = LaunchStatus { game_id:id.into(), source_id:source.into(), state:phase.into(), message };
    if let Ok(mut sessions) = app.state::<AppState>().sessions.lock() { sessions.insert(id.into(), status.clone()); }
    let _ = app.emit("launch-status", status);
}

fn targets(game: &Game) -> (Vec<PathBuf>, Option<PathBuf>) {
    let exact = Path::new(&game.launch_target).is_file().then(|| PathBuf::from(&game.launch_target));
    let roots = game.install_path.as_ref().map(PathBuf::from).filter(|p| p.is_dir()).into_iter().collect();
    (roots, exact)
}

fn is_helper(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    ["steam.exe", "epicgameslauncher.exe", "galaxyclient.exe", "ubisoftconnect.exe", "eadesktop.exe", "battle.net.exe", "launcher.exe"].contains(&name.as_str())
        || ["crashreport", "crashpad", "unins", "updater", "setup", "easyanticheat", "beservice"].iter().any(|p| name.starts_with(p))
}

fn matches(path: &Path, roots: &[PathBuf], exact: Option<&Path>) -> bool {
    if is_helper(path) { return false; }
    let candidate = path.to_string_lossy().to_lowercase();
    if exact.is_some_and(|p| p.to_string_lossy().to_lowercase() == candidate) { return true; }
    roots.iter().any(|root| {
        let root = root.to_string_lossy().trim_end_matches(['\\','/']).to_lowercase();
        candidate.strip_prefix(&root).is_some_and(|tail| tail.starts_with('\\') || tail.starts_with('/'))
    })
}

fn running(sys: &mut System, roots: &[PathBuf], exact: Option<&Path>) -> bool {
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
    sys.processes().values().any(|p| p.exe().is_some_and(|exe| matches(exe, roots, exact)))
}

pub fn start(app: &AppHandle, state: &AppState, id: &str, source_id: Option<&str>) -> Result<Game, String> {
    let _operation = state.operation.lock().map_err(|e| e.to_string())?;
    if state.maintenance.load(std::sync::atomic::Ordering::SeqCst) { return Err("A library restore is ready. Restart the launcher first.".into()); }
    let (display, game) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let display = db.get_game(id).map_err(|e| e.to_string())?.ok_or("Game not found")?;
        let preferred = source_id.or(display.preferred_copy_id.as_deref());
        let copy = if source_id.is_some() {
            display.copies.iter().find(|c| Some(c.id.as_str()) == source_id && !c.missing)
        } else {
            display.copies.iter().find(|c| Some(c.id.as_str()) == preferred && !c.missing)
                .or_else(|| display.copies.iter().find(|c| c.id == id && !c.missing))
                .or_else(|| display.copies.iter().find(|c| !c.missing))
        }.ok_or("Installation unavailable. Locate the game or rescan your library.")?;
        let game = db.get_game(&copy.id).map_err(|e| e.to_string())?.ok_or("Game copy not found")?;
        (display, game)
    };
    {
        let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        if sessions.get(id).is_some_and(|s| s.state == "launching" || s.state == "running") {
            return Err("This game is already launching or running.".into());
        }
        sessions.insert(id.into(), LaunchStatus { game_id:id.into(), source_id:game.id.clone(), state:"launching".into(), message:None });
    }
    publish(app, id, &game.id, "launching", None);
    let (roots, exact) = targets(&game);
    let mut sys = System::new();
    if !running(&mut sys, &roots, exact.as_deref()) {
        if let Err(error) = launch::launch_game(&game) {
            publish(app, id, &game.id, "failed", Some(error.to_string()));
            return Err(error.to_string());
        }
    }
    let app = app.clone();
    let id = id.to_string();
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(120);
        while !running(&mut sys, &roots, exact.as_deref()) {
            if Instant::now() >= deadline {
                publish(&app, &id, &game.id, "failed", Some("No game process was detected after two minutes. Check the game path or owning client, then retry.".into()));
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
        let state = app.state::<AppState>();
        let started = state.db.lock().map_err(|e| e.to_string()).and_then(|db| db.record_launch(&id, &game.id).map_err(|e| e.to_string()));
        if let Err(error) = started { publish(&app, &id, &game.id, "failed", Some(error)); return; }
        publish(&app, &id, &game.id, "running", None);
        let hide = state.db.lock().ok().and_then(|db| db.get_settings().ok()).and_then(|s| s.hide_on_game_launch).unwrap_or(false);
        if hide { if let Some(win) = app.get_webview_window("main") { let _ = win.hide(); } }
        let mut absent = 0;
        loop {
            thread::sleep(Duration::from_secs(3));
            if running(&mut sys, &roots, exact.as_deref()) {
                absent = 0;
                if let Ok(db) = state.db.lock() { let _ = db.heartbeat_session(&id); }
            } else {
                absent += 1;
                if absent >= 2 { break; }
            }
        }
        let result = state.db.lock().map_err(|e| e.to_string()).and_then(|db| db.finish_session(&id).map_err(|e| e.to_string()));
        match result {
            Ok(()) => publish(&app, &id, &game.id, "finished", None),
            Err(error) => publish(&app, &id, &game.id, "failed", Some(error)),
        }
        if hide && !active(&state) {
            if let Some(win) = app.get_webview_window("main") { let _ = win.unminimize(); let _ = win.show(); let _ = win.set_focus(); }
        }
    });
    Ok(display)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_install_boundaries_and_ignores_helpers() {
        let roots = vec![PathBuf::from(r"C:\Games\Example")];
        assert!(matches(Path::new(r"c:\games\example\bin\game.exe"), &roots, None));
        assert!(!matches(Path::new(r"C:\Games\ExampleTwo\game.exe"), &roots, None));
        assert!(!matches(Path::new(r"C:\Games\Example\CrashReporter.exe"), &roots, None));
        assert!(!matches(Path::new(r"D:\Other\game.exe"), &roots, None));
    }
}
