use biopass_rs_auth::{
    auth_history_dir, log_file_path, logs_dir, read_auth_history, read_log_tail,
    set_runtime_logging, AuthSessionSummary, LogComponent, RuntimeLoggingConfig,
};
use serde::Deserialize;
use tauri::AppHandle;

use crate::config::require_loaded_config;
use crate::paths::get_data_dir;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityLogComponent {
    Auth,
    Helper,
    Desktop,
}

impl From<ActivityLogComponent> for LogComponent {
    fn from(component: ActivityLogComponent) -> Self {
        match component {
            ActivityLogComponent::Auth => LogComponent::Auth,
            ActivityLogComponent::Helper => LogComponent::Helper,
            ActivityLogComponent::Desktop => LogComponent::Desktop,
        }
    }
}

fn configure_logging(app: &AppHandle) -> Result<(), String> {
    let config = require_loaded_config(app)?;
    let data_dir = get_data_dir(app)?;
    set_runtime_logging(RuntimeLoggingConfig::from_config(data_dir, &config.logging));
    Ok(())
}

#[tauri::command]
pub fn list_auth_history(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<Vec<AuthSessionSummary>, String> {
    configure_logging(&app)?;
    read_auth_history(limit.unwrap_or(100))
}

#[tauri::command]
pub fn read_activity_log_tail(
    app: AppHandle,
    component: ActivityLogComponent,
    max_lines: Option<usize>,
) -> Result<Vec<String>, String> {
    configure_logging(&app)?;
    read_log_tail(component.into(), max_lines.unwrap_or(500))
}

#[tauri::command]
pub fn activity_log_file_path(
    app: AppHandle,
    component: ActivityLogComponent,
) -> Result<String, String> {
    configure_logging(&app)?;
    Ok(log_file_path(component.into())
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub fn activity_logs_dir(app: AppHandle) -> Result<String, String> {
    configure_logging(&app)?;
    Ok(logs_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn auth_history_dir_path(app: AppHandle) -> Result<String, String> {
    configure_logging(&app)?;
    Ok(auth_history_dir().to_string_lossy().to_string())
}
