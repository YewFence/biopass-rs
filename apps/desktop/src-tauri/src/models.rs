use biopass_rs_auth::{
    builtin_models, download_models_report, BuiltinModelInfo, ModelDownloadReport,
};

#[tauri::command]
pub fn list_builtin_models() -> Result<Vec<BuiltinModelInfo>, String> {
    builtin_models()
}

#[tauri::command]
pub fn download_builtin_models() -> Result<ModelDownloadReport, String> {
    download_models_report()
}
