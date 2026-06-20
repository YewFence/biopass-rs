use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use crate::cli::ConfigAction;
use biopass_rs_auth::{
    bootstrap_config_at, config_path, migrate_config_at_path, reset_config_at_path, user_data_dir,
    user_exists, BiopassConfig, BootstrapOutcome,
};
use std::path::Path;

pub(crate) fn run(username: &str, action: ConfigAction) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }
    match action {
        ConfigAction::Init { force } => init_at_paths(
            username,
            &config_path(username),
            &user_data_dir(username),
            force,
        ),
        ConfigAction::Reset => {
            reset_at_paths(username, &config_path(username), &user_data_dir(username))
        }
        ConfigAction::Migrate => migrate_at_path(username, &config_path(username)),
    }
}

fn init_at_paths(username: &str, path: &Path, data_dir: &Path, force: bool) -> u8 {
    if force {
        if let Err(error) = reset_config_at_path(path, data_dir) {
            eprintln!("Failed to initialize config: {error}");
            return EXIT_AUTH_ERR;
        }
        eprintln!(
            "Wrote default config (forced) for user '{username}' at {}",
            path.display()
        );
        return EXIT_SUCCESS;
    }

    let default_factory = || BiopassConfig::default_for_data_dir(data_dir);
    match bootstrap_config_at(path, default_factory) {
        Ok(BootstrapOutcome::AlreadyPresent) => {
            eprintln!(
                "Config already exists for user '{username}' at {} (use --force to overwrite)",
                path.display()
            );
            EXIT_SUCCESS
        }
        Ok(BootstrapOutcome::WroteDefaults) => {
            eprintln!(
                "Wrote default config for user '{username}' at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to initialize config: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn reset_at_paths(username: &str, path: &Path, data_dir: &Path) -> u8 {
    match reset_config_at_path(path, data_dir) {
        Ok(()) => {
            eprintln!(
                "Reset config for user '{username}' to defaults at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to reset config: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn migrate_at_path(username: &str, path: &Path) -> u8 {
    if !path.is_file() {
        eprintln!(
            "No config found for user '{username}' at {}",
            path.display()
        );
        return EXIT_SUCCESS;
    }

    match migrate_config_at_path(path) {
        Ok(true) => {
            eprintln!(
                "Migrated config schema for user '{username}' at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Ok(false) => {
            eprintln!(
                "Config schema already current for user '{username}' at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to migrate config schema: {error}");
            EXIT_AUTH_ERR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use biopass_rs_auth::read_config_from_path;
    use std::fs;

    #[test]
    fn init_writes_default_config_when_absent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".config/biopass-rs/config.yaml");
        let data_dir = directory.path().join("data");

        let status = init_at_paths("alice", &path, &data_dir, false);

        assert_eq!(status, EXIT_SUCCESS);
        let config = read_config_from_path(&path).unwrap();
        assert_eq!(
            config.methods.face.detection.model,
            data_dir
                .join("models/yolov8n-face.onnx")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn init_keeps_existing_config_without_force() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        let data_dir = directory.path().join("data");
        fs::write(&path, "preexisting: true").unwrap();

        let status = init_at_paths("alice", &path, &data_dir, false);

        assert_eq!(status, EXIT_SUCCESS);
        assert_eq!(fs::read_to_string(path).unwrap(), "preexisting: true");
    }

    #[test]
    fn init_force_rewrites_existing_config() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        let data_dir = directory.path().join("data");
        fs::write(&path, "not: current").unwrap();

        let status = init_at_paths("alice", &path, &data_dir, true);

        assert_eq!(status, EXIT_SUCCESS);
        assert!(read_config_from_path(&path).is_ok());
    }

    #[test]
    fn reset_rewrites_default_config() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        let data_dir = directory.path().join("data");
        fs::write(&path, "not: current").unwrap();

        let status = reset_at_paths("alice", &path, &data_dir);

        assert_eq!(status, EXIT_SUCCESS);
        assert!(read_config_from_path(&path).is_ok());
    }

    #[test]
    fn migrate_missing_config_is_successful_noop() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.yaml");

        assert_eq!(migrate_at_path("alice", &path), EXIT_SUCCESS);
    }

    #[test]
    fn migrate_current_config_is_successful_noop() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        let data_dir = directory.path().join("data");
        reset_config_at_path(&path, &data_dir).unwrap();

        assert_eq!(migrate_at_path("alice", &path), EXIT_SUCCESS);
        assert!(read_config_from_path(&path).is_ok());
    }
}
