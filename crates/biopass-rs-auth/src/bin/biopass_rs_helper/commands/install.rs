use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use crate::utils::current_username;
use biopass_rs_auth::{
    bootstrap_config_at, download_models, run_ldconfig, BiopassConfig, BootstrapOutcome,
};
use users::os::unix::UserExt;

const NEW_CONFIG_PATH: &str = ".config/biopass-rs/config.yaml";
const OLD_DATA_DIR: &str = ".local/share/com.ticklab.biopass";
const NEW_DATA_DIR: &str = ".local/share/biopass-rs";

pub(crate) fn install() -> u8 {
    eprintln!("Running ldconfig...");
    if let Err(error) = run_ldconfig() {
        eprintln!("Warning: {error}");
    }

    eprintln!("Bootstrapping config for the current user...");
    match bootstrap_current_user() {
        Ok(message) => {
            if let Some(message) = message {
                eprintln!("{message}");
            }
        }
        Err(error) => eprintln!("Warning: {error}"),
    }

    eprintln!("Downloading models...");
    match download_models() {
        Ok(_) => {
            eprintln!("Installation complete.");
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to download models: {error}");
            EXIT_AUTH_ERR
        }
    }
}

pub(crate) fn model_download() -> u8 {
    eprintln!("Downloading models...");
    match download_models() {
        Ok(_) => {
            eprintln!("Models downloaded successfully.");
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to download models: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn bootstrap_current_user() -> Result<Option<String>, String> {
    let username = current_username()
        .ok_or_else(|| "Cannot determine current user (set USER/SUDO_USER)".to_string())?;
    let home = users::get_user_by_name(&username)
        .map(|user| user.home_dir().to_path_buf())
        .ok_or_else(|| format!("Cannot determine home directory for user '{username}'"))?;

    let new_config = home.join(NEW_CONFIG_PATH);
    let outcome_message =
        match bootstrap_config_at(&new_config, Some(&home), BiopassConfig::default) {
            Ok(BootstrapOutcome::AlreadyPresent) => format!(
                "Skipping config bootstrap for user '{username}': config already exists at {}",
                new_config.display()
            ),
            Ok(BootstrapOutcome::ImportedFromUpstream) => format!(
                "Imported upstream config for user '{username}' into {}",
                new_config.display()
            ),
            Ok(BootstrapOutcome::WroteDefaults) => format!(
                "Wrote default config for user '{username}' at {}",
                new_config.display()
            ),
            Err(error) => {
                return Err(format!(
                    "failed to bootstrap config for '{username}': {error}"
                ))
            }
        };

    let old_data = home.join(OLD_DATA_DIR);
    let new_data = home.join(NEW_DATA_DIR);
    if old_data.exists() && !new_data.exists() {
        eprintln!("Migrating data directory for user '{username}'...");
        if let Err(error) = std::fs::rename(&old_data, &new_data) {
            eprintln!("Warning: Failed to migrate data dir for '{username}': {error}");
        }
    }

    Ok(Some(outcome_message))
}
