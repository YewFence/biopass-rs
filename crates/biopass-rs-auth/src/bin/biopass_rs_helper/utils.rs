use biopass_rs_auth::{config_path, current_username, read_config_from_path};

pub(crate) fn resolve_username(explicit: Option<&str>) -> Option<String> {
    explicit
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(current_username)
}

pub(crate) fn helper_auto_optimize_camera(username: Option<&str>) -> bool {
    let user = username
        .filter(|user| !user.is_empty())
        .map(str::to_owned)
        .or_else(current_username);
    let Some(user) = user else {
        return true;
    };
    read_config_from_path(&config_path(&user))
        .map(|config| config.methods.face.auto_optimize_camera)
        .unwrap_or(true)
}
