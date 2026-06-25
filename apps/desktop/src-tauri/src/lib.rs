// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod activity;
pub mod auth;
pub mod config;
pub mod face;
pub mod face_session;
pub mod fingerprint;
pub mod fingerprint_auth;
pub mod models;
pub mod paths;
pub mod system;

use activity::{
    activity_log_file_path, activity_logs_dir, auth_history_dir_path, clean_activity_data,
    list_auth_history, read_activity_log_tail,
};
use auth::test_auth_flow;
use config::{config_file_path, load_config, reset_config, save_config};
use face::{capture_face, delete_face, list_faces};
use face_session::{capture_face_in_session, start_face_preview, stop_face_preview};
use fingerprint::{
    add_fingerprint, delete_fingerprint, enroll_fingerprint, fingerprint_is_available,
    list_enrolled_fingerprints, list_fingerprint_devices, remove_fingerprint,
};
use models::{download_builtin_models, list_builtin_models};
use system::{get_current_username, list_video_devices, path_exists};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = crate::paths::get_data_dir(app.handle())
                .map_err(|error| format!("failed to resolve data dir: {error}"))?;
            app.asset_protocol_scope()
                .allow_directory(&data_dir, true)
                .map_err(|error| {
                    format!("failed to allow asset dir {}: {error}", data_dir.display())
                })?;

            #[cfg(target_os = "linux")]
            {
                use webkit2gtk::{PermissionRequestExt, WebViewExt};
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.with_webview(|webview| {
                        webview.inner().connect_permission_request(
                            |_view, request: &webkit2gtk::PermissionRequest| {
                                request.allow();
                                true
                            },
                        );
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            reset_config,
            config_file_path,
            test_auth_flow,
            list_auth_history,
            read_activity_log_tail,
            activity_log_file_path,
            activity_logs_dir,
            auth_history_dir_path,
            clean_activity_data,
            get_current_username,
            capture_face,
            start_face_preview,
            stop_face_preview,
            capture_face_in_session,
            list_faces,
            list_video_devices,
            delete_face,
            add_fingerprint,
            delete_fingerprint,
            enroll_fingerprint,
            remove_fingerprint,
            fingerprint_is_available,
            list_enrolled_fingerprints,
            list_fingerprint_devices,
            list_builtin_models,
            download_builtin_models,
            path_exists
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
