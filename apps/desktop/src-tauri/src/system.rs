use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct VideoDeviceInfo {
    pub path: String,
    pub name: String,
    pub display_name: String,
}

pub fn biopass_helper_path() -> String {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let candidates = [
        workspace_root.join("target/release/biopass-helper"),
        workspace_root.join("target/debug/biopass-helper"),
        PathBuf::from("/usr/bin/biopass-helper"),
    ];

    candidates
        .iter()
        .find(|path| path.exists())
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "biopass-helper".to_string())
}

#[tauri::command]
pub fn get_current_username() -> Result<String, String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .map_err(|_| "Could not determine current username".to_string())
}

#[tauri::command]
pub fn list_video_devices() -> Result<Vec<VideoDeviceInfo>, String> {
    let devices = biopass_auth::list_video_devices();
    Ok(devices
        .into_iter()
        .map(|dev| VideoDeviceInfo {
            path: dev.path_str(),
            name: dev.card.clone(),
            display_name: dev.display_name(),
        })
        .collect())
}
