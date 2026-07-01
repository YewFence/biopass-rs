use biopass_rs_auth::{
    auth_history_dir, cleanup_data_dir, log_file_path, logs_dir, read_auth_history, read_log_tail,
    AuthSessionSummary, CleanupMode, CleanupOptions, CleanupReport, CleanupTarget, LogComponent,
    LogLevel,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCleanupTarget {
    All,
    Debugs,
    Logs,
    AuthHistory,
}

impl From<ActivityCleanupTarget> for CleanupTarget {
    fn from(target: ActivityCleanupTarget) -> Self {
        match target {
            ActivityCleanupTarget::All => CleanupTarget::All,
            ActivityCleanupTarget::Debugs => CleanupTarget::Debugs,
            ActivityCleanupTarget::Logs => CleanupTarget::Logs,
            ActivityCleanupTarget::AuthHistory => CleanupTarget::AuthHistory,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCleanupMode {
    Retention,
    All,
}

impl From<ActivityCleanupMode> for CleanupMode {
    fn from(mode: ActivityCleanupMode) -> Self {
        match mode {
            ActivityCleanupMode::Retention => CleanupMode::Retention,
            ActivityCleanupMode::All => CleanupMode::All,
        }
    }
}

#[tauri::command]
pub fn list_auth_history(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<Vec<AuthSessionSummary>, String> {
    crate::logging::ensure_logging(&app);
    read_auth_history(limit.unwrap_or(100))
}

#[tauri::command]
pub fn read_activity_log_tail(
    app: AppHandle,
    component: ActivityLogComponent,
    max_lines: Option<usize>,
) -> Result<Vec<String>, String> {
    crate::logging::ensure_logging(&app);
    read_log_tail(component.into(), max_lines.unwrap_or(500))
}

#[tauri::command]
pub fn activity_log_file_path(
    app: AppHandle,
    component: ActivityLogComponent,
) -> Result<String, String> {
    crate::logging::ensure_logging(&app);
    Ok(log_file_path(component.into())
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub fn activity_logs_dir(app: AppHandle) -> Result<String, String> {
    crate::logging::ensure_logging(&app);
    Ok(logs_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn auth_history_dir_path(app: AppHandle) -> Result<String, String> {
    crate::logging::ensure_logging(&app);
    Ok(auth_history_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn clean_activity_data(
    app: AppHandle,
    target: ActivityCleanupTarget,
    mode: ActivityCleanupMode,
    dry_run: Option<bool>,
) -> Result<CleanupReport, String> {
    crate::logging::ensure_logging(&app);
    let dry_run = dry_run.unwrap_or(false);
    crate::logging::desktop_log(
        LogLevel::Info,
        "cleanup",
        &format!("target={:?} mode={:?} dry_run={dry_run}", target, mode),
    );
    let config = require_loaded_config(&app)?;
    let data_dir = get_data_dir(&app)?;
    let report = cleanup_data_dir(
        &data_dir,
        &config,
        CleanupOptions {
            target: target.into(),
            mode: mode.into(),
            dry_run,
        },
    );
    let removed: usize = report.sections.iter().map(|s| s.removed_entries).sum();
    let failed: usize = report.sections.iter().map(|s| s.failed_entries).sum();
    let freed: u64 = report.sections.iter().map(|s| s.freed_bytes).sum();
    crate::logging::desktop_log(
        LogLevel::Info,
        "cleanup",
        &format!("removed {removed} entries, freed {freed} bytes, {failed} failed"),
    );
    Ok(report)
}
