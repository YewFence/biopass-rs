use biopass_rs_auth::{authenticate_user, current_username, AuthSessionResult};

#[tauri::command]
pub fn test_auth_flow(service: Option<String>) -> Result<AuthSessionResult, String> {
    let username =
        current_username().ok_or_else(|| "Could not determine current username".to_string())?;
    authenticate_user(&username, service.as_deref())
}
