use biopass_rs_auth::{
    builtin_models, download_models_report, BuiltinModelInfo, LogLevel, ModelDownloadReport,
};
use tauri::AppHandle;

#[tauri::command]
pub fn list_builtin_models(app: AppHandle) -> Result<Vec<BuiltinModelInfo>, String> {
    crate::logging::ensure_logging(&app);
    crate::logging::desktop_log(LogLevel::Debug, "models", "listing builtin models");
    builtin_models()
}

#[tauri::command]
pub async fn download_builtin_models(app: AppHandle) -> Result<ModelDownloadReport, String> {
    crate::logging::ensure_logging(&app);
    crate::logging::desktop_log(LogLevel::Info, "models", "starting builtin model download");
    let outcome = tauri::async_runtime::spawn_blocking(download_models_report)
        .await
        .map_err(|error| format!("Model download task failed: {error}"))?;
    match outcome {
        Ok(report) => {
            crate::logging::desktop_log(
                LogLevel::Info,
                "models",
                &format!(
                    "downloaded {}, skipped {}",
                    report.downloaded, report.skipped
                ),
            );
            Ok(report)
        }
        Err(error) => {
            crate::logging::desktop_log(
                LogLevel::Error,
                "models",
                &format!("download failed: {error}"),
            );
            Err(error)
        }
    }
}
