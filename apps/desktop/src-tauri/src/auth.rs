use biopass_rs_auth::{
    authenticate_user_with_options, current_username, AuthSessionOptions, AuthSessionPaths,
    AuthSessionResult, LogLevel,
};

#[tauri::command]
pub async fn test_auth_flow(
    service: Option<String>,
    log_level: Option<String>,
) -> Result<AuthSessionResult, String> {
    let username =
        current_username().ok_or_else(|| "Could not determine current username".to_string())?;
    let log_level_override = match log_level.as_deref() {
        Some(level) => {
            Some(LogLevel::from_name(level).ok_or_else(|| format!("Invalid log level '{level}'"))?)
        }
        None => None,
    };
    tauri::async_runtime::spawn_blocking(move || {
        authenticate_user_with_options(
            &username,
            service.as_deref(),
            AuthSessionPaths {
                config_path: biopass_rs_auth::config_path(&username),
                data_dir: biopass_rs_auth::user_data_dir(&username),
            },
            biopass_rs_auth::user_exists(&username),
            AuthSessionOptions { log_level_override },
        )
    })
    .await
    .map_err(|error| format!("Authentication task failed: {error}"))?
}
