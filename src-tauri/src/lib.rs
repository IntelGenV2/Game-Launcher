mod commands;
mod covers;
mod db;
mod fps;
mod launch;
mod models;
mod scanners;

use commands::AppState;
use db::Database;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = db::default_db_path();
    let database = Database::open(&db_path).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            db: Mutex::new(database),
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
            commands::set_game_path,
            commands::set_game_name,
            commands::remove_game,
            commands::get_cover_data_url,
            commands::get_cover_data_urls,
            commands::get_game_stats,
            commands::log_fps,
            commands::record_live_fps,
            commands::stop_fps_monitor,
            commands::app_data_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
