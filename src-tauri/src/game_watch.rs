use crate::commands::AppState;
use crate::models::Game;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Manager};

const POLL_START: Duration = Duration::from_secs(1);
const POLL_RUNNING: Duration = Duration::from_secs(3);
const START_TIMEOUT: Duration = Duration::from_secs(120);
const EXIT_CONFIRM: u32 = 2;

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn hide_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

fn normalize_dir(path: &Path) -> PathBuf {
    let s = path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    PathBuf::from(s)
}

/// Roots (install folders) and exact exe basenames to treat as "the game is running".
fn watch_targets(game: &Game) -> (Vec<PathBuf>, Vec<String>) {
    let mut roots = Vec::new();
    let mut names = Vec::new();

    if let Some(install) = game.install_path.as_deref() {
        let p = PathBuf::from(install);
        if p.is_dir() {
            roots.push(normalize_dir(&p));
        }
    }

    let target = Path::new(&game.launch_target);
    if target.is_file() {
        if let Some(parent) = target.parent() {
            roots.push(normalize_dir(parent));
        }
        if let Some(name) = target.file_name() {
            names.push(name.to_string_lossy().to_lowercase());
        }
    }

    roots.sort();
    roots.dedup();
    names.sort();
    names.dedup();
    (roots, names)
}

fn process_matches(exe: &Path, roots: &[PathBuf], names: &[String]) -> bool {
    let exe_lower = exe.to_string_lossy().to_lowercase();
    for root in roots {
        let root_s = root.to_string_lossy().to_lowercase();
        if root_s.is_empty() {
            continue;
        }
        if exe_lower.starts_with(&root_s) {
            let rest = &exe_lower[root_s.len()..];
            if rest.is_empty() || rest.starts_with('\\') || rest.starts_with('/') {
                return true;
            }
        }
    }
    if let Some(name) = exe.file_name() {
        let n = name.to_string_lossy().to_lowercase();
        if names.iter().any(|e| e == &n) {
            return true;
        }
    }
    false
}

fn refresh_kind() -> ProcessRefreshKind {
    // Exe paths only — full refresh (cpu/memory/disk) balloons RAM when polled every second.
    ProcessRefreshKind::nothing().with_exe(UpdateKind::Always)
}

fn any_game_process(sys: &mut System, roots: &[PathBuf], names: &[String]) -> bool {
    if roots.is_empty() && names.is_empty() {
        return false;
    }
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
    for process in sys.processes().values() {
        let Some(exe) = process.exe() else {
            continue;
        };
        if process_matches(exe, roots, names) {
            return true;
        }
    }
    false
}

fn generation_current(app: &AppHandle, gen: u64) -> bool {
    app.try_state::<AppState>()
        .map(|s| s.hide_watch_gen.load(Ordering::SeqCst) == gen)
        .unwrap_or(false)
}

/// Hide the launcher and restore it after the game process starts and then exits.
/// No-op when the setting is off. Does not quit the app.
pub fn maybe_hide_while_playing(app: &AppHandle, state: &AppState, game: &Game) {
    let enabled = state
        .db
        .lock()
        .ok()
        .and_then(|db| db.get_settings().ok())
        .and_then(|s| s.hide_on_game_launch)
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let (roots, names) = watch_targets(game);
    let gen = state.hide_watch_gen.fetch_add(1, Ordering::SeqCst) + 1;
    hide_main_window(app);

    let app = app.clone();
    thread::spawn(move || {
        // One System for the whole watch — recreating each poll OOMs over a long session.
        let mut sys = System::new();
        let started = Instant::now();
        let mut saw_running = false;

        while generation_current(&app, gen) {
            if any_game_process(&mut sys, &roots, &names) {
                saw_running = true;
                break;
            }
            if started.elapsed() >= START_TIMEOUT {
                break;
            }
            thread::sleep(POLL_START);
        }

        if !generation_current(&app, gen) {
            return;
        }

        if !saw_running {
            show_main_window(&app);
            return;
        }

        let mut gone = 0u32;
        while generation_current(&app, gen) {
            if any_game_process(&mut sys, &roots, &names) {
                gone = 0;
            } else {
                gone += 1;
                if gone >= EXIT_CONFIRM {
                    break;
                }
            }
            thread::sleep(POLL_RUNNING);
        }

        if generation_current(&app, gen) {
            show_main_window(&app);
        }
    });
}
