// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod config;
pub mod face;
pub mod face_session;
pub mod fingerprint;
pub mod fingerprint_auth;
pub mod paths;
pub mod system;

use config::{config_file_path, load_config, reset_config, save_config};
use face::{capture_face, delete_face, list_faces};
use face_session::{capture_face_in_session, start_face_preview, stop_face_preview};
use fingerprint::{
    add_fingerprint, delete_fingerprint, enroll_fingerprint, fingerprint_is_available,
    list_enrolled_fingerprints, list_fingerprint_devices, remove_fingerprint,
};
use system::{get_current_username, list_video_devices};

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
            list_fingerprint_devices
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
