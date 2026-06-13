use biopass_rs_auth::read_config;
use std::env;
use std::path::PathBuf;

pub(crate) fn resolve_username(explicit: Option<&str>) -> Option<String> {
    explicit
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(current_username)
}

pub(crate) fn home_dir_for_username(username: &str) -> Option<PathBuf> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        if name != username {
            return None;
        }
        let home = parts.nth(4)?;
        if home.is_empty() {
            None
        } else {
            Some(PathBuf::from(home))
        }
    })
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
