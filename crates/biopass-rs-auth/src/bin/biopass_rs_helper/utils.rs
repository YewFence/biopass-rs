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

#[cfg(test)]
mod tests {
    use super::*;
    use biopass_rs_auth::CONFIG_PATH_ENV;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_config_path_env<T>(
        path: &std::path::Path,
        f: impl FnOnce() -> T + std::panic::UnwindSafe,
    ) -> T {
        let _guard = ENV_TEST_LOCK.lock().unwrap();
        let previous = std::env::var_os(CONFIG_PATH_ENV);
        std::env::set_var(CONFIG_PATH_ENV, path);
        let result = std::panic::catch_unwind(f);

        if let Some(value) = previous {
            std::env::set_var(CONFIG_PATH_ENV, value);
        } else {
            std::env::remove_var(CONFIG_PATH_ENV);
        }

        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    #[test]
    fn resolve_username_prefers_non_empty_explicit_value() {
        assert_eq!(resolve_username(Some("alice")).as_deref(), Some("alice"));
    }

    #[test]
    fn helper_auto_optimize_camera_uses_config_value() {
        let directory = tempfile::tempdir().unwrap();
        let config_path = directory.path().join("config.yaml");
        let mut config = biopass_rs_auth::BiopassConfig::default_for_data_dir(directory.path());
        config.methods.face.auto_optimize_camera = false;
        biopass_rs_auth::write_config_to_path(&config_path, &config).unwrap();

        with_config_path_env(&config_path, || {
            assert!(!helper_auto_optimize_camera(Some("missing-user")));
        });
    }

    #[test]
    fn helper_auto_optimize_camera_defaults_true_for_missing_or_empty_user() {
        let directory = tempfile::tempdir().unwrap();
        let missing_config = directory.path().join("missing.yaml");

        with_config_path_env(&missing_config, || {
            assert!(helper_auto_optimize_camera(Some("missing-user")));
        });
    }
}
