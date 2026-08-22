mod backup;
mod commands;
mod console;
mod covers;
mod db;
mod game_watch;
mod launch;
mod metadata;
mod models;
mod playnite;
mod scanners;
mod system_info;

use commands::AppState;
use db::Database;
use std::sync::Mutex;
use tauri::http::{header::CONTENT_TYPE, StatusCode};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

fn cover_protocol_response(
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let fail = |status: StatusCode| {
        tauri::http::Response::builder()
            .status(status)
            .header("Access-Control-Allow-Origin", "*")
            .body(Vec::new())
            .unwrap()
    };

    if request.method() == tauri::http::Method::OPTIONS {
        return tauri::http::Response::builder()
            .status(StatusCode::OK)
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "GET, OPTIONS")
            .body(Vec::new())
            .unwrap();
    }

    let raw = request.uri().path().trim_start_matches('/');
    let decoded = urlencoding::decode(raw).unwrap_or(std::borrow::Cow::Borrowed(raw));
    let Some((bytes, mime)) = covers::read_named_cover(&decoded) else {
        return fail(StatusCode::NOT_FOUND);
    };

    tauri::http::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, mime)
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "public, max-age=86400")
        .body(bytes)
        .unwrap()
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    db::apply_pending_reset();
    let db_path = db::default_db_path();
    let database = Database::open(&db_path).expect("failed to open database");
    let start_hidden = std::env::args().any(|a| a == "--background")
        && database
            .get_settings()
            .ok()
            .and_then(|s| s.start_in_background)
            .unwrap_or(true);

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .register_uri_scheme_protocol("cover", |_ctx, request| {
            cover_protocol_response(&request)
        })
        .manage(AppState {
            db: Mutex::new(database),
            hide_watch_gen: std::sync::atomic::AtomicU64::new(0),
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::list_hidden_games,
            commands::rescan_library,
            commands::launch_game,
            commands::end_play_session,
            commands::toggle_favorite,
            commands::set_hidden,
            commands::open_install_folder,
            commands::add_manual_game,
            commands::get_settings,
            commands::save_settings,
            commands::library_stats,
            commands::fetch_covers,
            commands::set_custom_cover,
            commands::list_cover_choice_groups,
            commands::list_cover_choices,
            commands::choose_cover,
            commands::set_game_path,
            commands::set_game_name,
            commands::remove_game,
            commands::restore_game,
            commands::finalize_remove,
            commands::get_cover_data_url,
            commands::get_cover_data_urls,
            commands::get_game,
            commands::get_game_stats,
            commands::app_data_path,
            commands::list_groups,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::add_game_to_group,
            commands::remove_game_from_group,
            commands::set_notes,
            commands::set_tags,
            commands::set_launch_options,
            commands::fetch_game_metadata,
            commands::set_custom_logo,
            commands::get_logo_data_url,
            commands::get_logo_data_urls,
            commands::launch_action,
            commands::library_overview,
            commands::suggest_duplicates,
            commands::merge_games,
            commands::export_backup,
            commands::import_playnite,
            commands::default_playnite_path,
            commands::started_in_background,
            commands::system_info,
            commands::reset_all_art,
            commands::reset_all_stats,
            commands::reset_app,
            console::explorer_places,
            console::list_explorer,
            console::open_explorer,
            console::open_path,
            console::system_power,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            if let Some(win) = app.get_webview_window("main") {
                if !start_hidden {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                let close_handle = handle.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let hide = {
                            let mut hide = false;
                            if let Some(state) = close_handle.try_state::<AppState>() {
                                if let Ok(db) = state.db.lock() {
                                    hide = db
                                        .get_settings()
                                        .ok()
                                        .and_then(|s| s.close_to_tray)
                                        .unwrap_or(false);
                                }
                            }
                            hide
                        };
                        if hide {
                            api.prevent_close();
                            if let Some(w) = close_handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                    }
                });
            }

            let show_i = MenuItem::with_id(app, "show", "Show library", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;
            let tray_handle = handle.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .menu(&menu)
                .tooltip("IntelGen Game Launcher")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(&tray_handle);
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
