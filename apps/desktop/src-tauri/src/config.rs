use biopass_rs_auth::{
    bootstrap_config_at, read_config_from_path, write_config_to_path, BootstrapOutcome,
};
pub use biopass_rs_auth::{
    BiopassConfig, DetectionConfig, FaceMethodConfig, FingerConfig, FingerprintMethodConfig,
    MethodsConfig, ModelConfig, RecognitionConfig, StrategyConfig,
};
use serde::Serialize;
use std::fs;
use std::path::Path;
use tauri::AppHandle;

use crate::paths::{get_config_dir, get_config_path, get_data_dir};

fn get_default_config(app: &AppHandle) -> Result<BiopassConfig, String> {
    get_data_dir(app).map(|data_dir| BiopassConfig::default_for_data_dir(&data_dir))
}

/// Returned by `load_config`. The variants distinguish the three GUI-relevant
/// outcomes so the frontend can react appropriately: loaded (optionally with a
/// migration / initialization notice) or broken (let the user fix or reset).
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoadConfigResult {
    Loaded {
        path: String,
        config: Box<BiopassConfig>,
        /// True if the on-disk file was rewritten to the current schema.
        migrated: bool,
        /// True if the file was just created from built-in defaults.
        initialized: bool,
    },
    Broken {
        path: String,
        message: String,
    },
}

/// Tauri command — returns the loaded config plus state flags so the GUI can
/// surface a one-time migration notice, an "initialized from defaults"
/// notice, or a recovery overlay when the file is unparsable.
#[tauri::command]
pub fn load_config(app: AppHandle) -> Result<LoadConfigResult, String> {
    load_config_internal(&app)
}

/// Internal helper. Other Tauri commands that need the active config
/// should use [`require_loaded_config`] instead so they reject the broken
/// case with a helpful error.
pub fn load_config_internal(app: &AppHandle) -> Result<LoadConfigResult, String> {
    let config_path = get_config_path(app)?;

    if !config_path.exists() {
        return initialize_missing_config(app, &config_path);
    }

    match read_config_from_path(&config_path) {
        Ok(config) => Ok(LoadConfigResult::Loaded {
            path: path_to_string(&config_path),
            config: Box::new(config),
            migrated: false,
            initialized: false,
        }),
        Err(message) => Ok(LoadConfigResult::Broken {
            path: path_to_string(&config_path),
            message,
        }),
    }
}

/// Resolve a usable config or return an error suitable for use by Tauri
/// commands that cannot proceed when the config is missing/broken.
pub fn require_loaded_config(app: &AppHandle) -> Result<BiopassConfig, String> {
    match load_config_internal(app)? {
        LoadConfigResult::Loaded { config, .. } => Ok(*config),
        LoadConfigResult::Broken { path, message } => Err(format!(
            "Config at {path} is unreadable, fix or reset it before continuing: {message}"
        )),
    }
}

fn initialize_missing_config(
    app: &AppHandle,
    config_path: &Path,
) -> Result<LoadConfigResult, String> {
    let defaults = get_default_config(app)?;
    let defaults_for_read = defaults.clone();

    let outcome = bootstrap_config_at(config_path, move || defaults)
        .map_err(|e| format!("Failed to bootstrap config: {e}"))?;

    let (config, migrated, initialized) = match outcome {
        BootstrapOutcome::AlreadyPresent => {
            // bootstrap should not return AlreadyPresent because we only
            // call it when the file is missing, but be defensive.
            let config = read_config_from_path(config_path)?;
            (config, false, false)
        }
        BootstrapOutcome::WroteDefaults => (defaults_for_read, false, true),
    };

    Ok(LoadConfigResult::Loaded {
        path: path_to_string(config_path),
        config: Box::new(config),
        migrated,
        initialized,
    })
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: BiopassConfig) -> Result<(), String> {
    let config_dir = get_config_dir(&app)?;
    let config_path = get_config_path(&app)?;

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    write_config_to_path(&config_path, &config)
}

/// Tauri command — reset the on-disk config to GUI defaults and return the
/// loaded value. Used by the GUI's "Reset to defaults" recovery button.
#[tauri::command]
pub fn reset_config(app: AppHandle) -> Result<LoadConfigResult, String> {
    let config_path = get_config_path(&app)?;
    let defaults = get_default_config(&app)?;
    // Write the GUI-flavoured defaults (with absolute model paths) rather
    // than the bare library defaults so the user does not lose their model
    // wiring.
    write_config_to_path(&config_path, &defaults)?;
    Ok(LoadConfigResult::Loaded {
        path: path_to_string(&config_path),
        config: Box::new(defaults),
        migrated: false,
        initialized: true,
    })
}

/// Tauri command — return the path of the active config file (used by the
/// GUI when it offers a "copy path / open in editor" recovery action).
#[tauri::command]
pub fn config_file_path(app: AppHandle) -> Result<String, String> {
    Ok(path_to_string(&get_config_path(&app)?))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
