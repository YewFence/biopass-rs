use biopass_rs_auth::{
    authenticate_user_with_options, current_username, AuthSessionOptions, AuthSessionPaths,
    AuthSessionResult, AuthSessionStatus, LogLevel, PamCode,
};
use tauri::AppHandle;

#[tauri::command]
pub async fn test_auth_flow(
    app: AppHandle,
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
    crate::logging::ensure_logging(&app);
    crate::logging::desktop_log(
        LogLevel::Info,
        "auth_test",
        &format!(
            "starting auth test for '{}' with level override {:?}",
            service.as_deref().unwrap_or("sudo"),
            log_level_override,
        ),
    );
    let outcome = tauri::async_runtime::spawn_blocking(move || {
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
    .map_err(|error| format!("Authentication task failed: {error}"))?;
    // The auth session re-initializes runtime logging with its own override,
    // so re-assert the desktop app's configured level before emitting again.
    crate::logging::ensure_logging(&app);
    let result = match outcome {
        Ok(result) => result,
        Err(error) => {
            crate::logging::desktop_log(
                LogLevel::Error,
                "auth_test",
                &format!("authentication run failed: {error}"),
            );
            return Err(error);
        }
    };
    match &result.status {
        AuthSessionStatus::Completed(PamCode::Success) => {
            crate::logging::desktop_log(LogLevel::Info, "auth_test", "authentication succeeded");
        }
        AuthSessionStatus::Completed(code) => {
            let detail = match code {
                PamCode::Success => "succeeded",
                PamCode::AuthError => "failed (auth_error)",
                PamCode::Ignore => "failed (ignored)",
            };
            crate::logging::desktop_log(
                LogLevel::Warn,
                "auth_test",
                &format!("authentication {detail}"),
            );
        }
        AuthSessionStatus::Ignored => {
            crate::logging::desktop_log(
                LogLevel::Info,
                "auth_test",
                "authentication ignored by config",
            );
        }
    }
    Ok(result)
}
