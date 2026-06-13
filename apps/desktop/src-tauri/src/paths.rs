use biopass_rs_auth::{config_path, current_username, user_data_dir};
use std::path::PathBuf;
use tauri::AppHandle;

pub const CONFIG_FILE: &str = "config.yaml";

/// The Tauri command layer still passes `&AppHandle` for consistency with
/// other commands, but the desktop now reads its config / data locations
/// from the same source the helper CLI uses (`BIOPASS_CONFIG` /
/// `BIOPASS_DATA_DIR`, plus XDG defaults). Tauri-specific dir resolution
/// is no longer used.
fn resolved_username() -> String {
    current_username().unwrap_or_else(|| "current".to_string())
}

pub fn get_config_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    let path = config_path(&resolved_username());
    Ok(path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".")))
}

pub fn get_config_path(_app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_path(&resolved_username()))
}

pub fn get_data_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    Ok(user_data_dir(&resolved_username()))
}

pub fn get_faces_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    Ok(get_data_dir(_app)?.join("faces"))
}
