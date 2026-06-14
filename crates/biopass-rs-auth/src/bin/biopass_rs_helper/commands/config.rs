use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use crate::cli::ConfigAction;
use biopass_rs_auth::{
    bootstrap_config_at, config_path, migrate_config_at_path, reset_config_at_path, user_data_dir,
    user_exists, BiopassConfig, BootstrapOutcome,
};

pub(crate) fn run(username: &str, action: ConfigAction) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }
    match action {
        ConfigAction::Init { force } => init(username, force),
        ConfigAction::Reset => reset(username),
        ConfigAction::Migrate => migrate(username),
    }
}

fn init(username: &str, force: bool) -> u8 {
    let path = config_path(username);
    let data_dir = user_data_dir(username);
    if force {
        if let Err(error) = reset_config_at_path(&path, &data_dir) {
            eprintln!("Failed to initialize config: {error}");
            return EXIT_AUTH_ERR;
        }
        eprintln!(
            "Wrote default config (forced) for user '{username}' at {}",
            path.display()
        );
        return EXIT_SUCCESS;
    }

    let default_factory = || BiopassConfig::default_for_data_dir(&data_dir);
    match bootstrap_config_at(&path, default_factory) {
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

fn reset(username: &str) -> u8 {
    let path = config_path(username);
    let data_dir = user_data_dir(username);
    match reset_config_at_path(&path, &data_dir) {
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

fn migrate(username: &str) -> u8 {
    let path = config_path(username);
    if !path.is_file() {
        eprintln!(
            "No config found for user '{username}' at {}",
            path.display()
        );
        return EXIT_SUCCESS;
    }

    match migrate_config_at_path(&path) {
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
