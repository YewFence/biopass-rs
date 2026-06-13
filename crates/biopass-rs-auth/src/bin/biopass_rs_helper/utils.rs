use biopass_rs_auth::read_config;
use std::env;

pub(crate) fn resolve_username(explicit: Option<&str>) -> Option<String> {
    explicit
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(current_username)
}

pub(crate) fn current_username() -> Option<String> {
    for key in ["SUDO_USER", "USER", "USERNAME", "LOGNAME"] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty()
                && trimmed
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
            {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

pub(crate) fn helper_auto_optimize_camera(username: Option<&str>) -> bool {
    let user = username
        .filter(|user| !user.is_empty())
        .map(str::to_owned)
        .or_else(current_username);
    let Some(user) = user else {
        return true;
    };
    read_config(&user)
        .map(|config| config.methods.face.auto_optimize_camera)
        .unwrap_or(true)
}
