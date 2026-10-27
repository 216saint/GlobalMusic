mod aggregate;
mod artwork;
mod audio;
mod auth;
mod commands;
mod config;
mod error;
mod favorites;
mod imports;
mod lyrics;
mod models;
mod player;
mod presence;
mod providers;
mod social;
mod sponsorblock;
mod state;
mod updates;
mod window;

use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use state::AppState;
use updates::Updates;

/// Log em `%LOCALAPPDATA%\com.globalmusic.app\logs\globalmusic.log` (Windows),
/// `~/Library/Logs/com.globalmusic.app` (macOS) ou `~/.local/share/com.globalmusic.app/logs` (Linux).
/// No app instalado não existe terminal: é aqui que a gente descobre por que algo falhou.
fn logger() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::LogDir { file_name: Some("globalmusic".into()) }),
            Target::new(TargetKind::Stdout),
        ])
        .level(log::LevelFilter::Info)
        // Bibliotecas de rede falam demais em Info.
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("hyper_util", log::LevelFilter::Warn)
        .level_for("reqwest", log::LevelFilter::Warn)
        .level_for("stream_download", log::LevelFilter::Warn)
        .max_file_size(2_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(logger())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_oauth::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(AppState::new(app.handle()));
            app.manage(Updates::default());
            window::create_main(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::library_playlists,
            commands::playlist_tracks,
            commands::accounts,
            commands::connect_account,
            commands::disconnect_account,
            commands::favorites_list,
            commands::set_favorite,
            commands::spotify_custom_client_id,
            commands::set_spotify_custom_client_id,
            commands::player_state,
            commands::player_play,
            commands::player_toggle,
            commands::player_next,
            commands::player_previous,
            commands::player_seek,
            commands::player_enqueue,
            commands::player_play_next,
            commands::player_remove,
            commands::player_jump,
            commands::player_set_volume,
            commands::player_prepare,
            commands::lyrics,
            commands::track_alternatives,
            commands::choose_track_version,
            commands::imported_playlists,
            commands::import_playlist,
            commands::remove_imported_playlist,
            updates::update_check,
            updates::update_install,
            social::social_me,
            social::social_login,
            social::social_link,
            social::social_logout,
            social::social_update_profile,
            social::social_use_profile,
            social::social_upload_avatar,
            social::social_search,
            social::social_friends,
            social::social_add_friend,
            social::social_remove_friend,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o GlobalMusic");
}
