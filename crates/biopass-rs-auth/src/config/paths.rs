use super::schema::BiopassConfig;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use users::os::unix::UserExt;

const CONFIG_FILE: &str = ".config/biopass-rs/config.yaml";
const DATA_DIR: &str = ".local/share/biopass-rs";

pub const CONFIG_PATH_ENV: &str = "BIOPASS_CONFIG";
pub const DATA_DIR_ENV: &str = "BIOPASS_DATA_DIR";

static CONFIG_PATH_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();
static DATA_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Single source of truth for the "config could not be parsed" error message.
/// PAM, helper CLI and the desktop GUI all surface this exact string, so the
/// user sees the same recovery instructions no matter where the failure is
/// detected.
pub fn config_parse_error_message(path: &Path, cause: &str) -> String {
    format!(
        "Failed to parse config at {}: {}\n\nYou can edit it manually, or run `biopass-rs-helper config reset` to restore defaults.",
        path.display(),
        cause
    )
}

/// Resolve the active config path for `username`.
///
/// Resolution order:
/// 1. CLI override (set by [`set_config_path_override`]).
/// 2. `BIOPASS_CONFIG` environment variable.
/// 3. The home directory from the system user database joined with
///    [`CONFIG_FILE`].
/// 4. `$HOME` joined with [`CONFIG_FILE`].
/// 5. `/etc/biopass-rs/config.yaml` as a last resort.
pub fn config_path(username: &str) -> PathBuf {
    if let Some(override_path) = config_path_override() {
        return override_path;
    }
    if let Some(env_path) = env_path(CONFIG_PATH_ENV) {
        return env_path;
    }
    match users::get_user_by_name(username) {
        Some(user) => user.home_dir().to_path_buf().join(CONFIG_FILE),
        None => std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(CONFIG_FILE))
            .unwrap_or_else(|| PathBuf::from("/etc/biopass-rs/config.yaml")),
    }
}

/// Resolve the active data directory for `username` (faces / debugs / ...).
///
/// Resolution order:
/// 1. CLI override (set by [`set_data_dir_override`]).
/// 2. `BIOPASS_DATA_DIR` environment variable.
/// 3. The home directory from the system user database joined with
///    [`DATA_DIR`].
/// 4. `$HOME` joined with [`DATA_DIR`].
/// 5. Empty path as a last resort.
pub fn user_data_dir(username: &str) -> PathBuf {
    if let Some(override_dir) = data_dir_override() {
        return override_dir;
    }
    if let Some(env_dir) = env_path(DATA_DIR_ENV) {
        return env_dir;
    }
    let home_from_user =
        users::get_user_by_name(username).map(|user| user.home_dir().to_path_buf());
    home_from_user
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .map(|home| home.join(DATA_DIR))
        .unwrap_or_default()
}

/// Set a CLI override for the config path. Subsequent calls to
/// [`config_path`] will return this value, regardless of `username`.
///
/// First writer wins — calling this more than once is a no-op so that
/// downstream code (e.g. tests) can layer overrides safely.
pub fn set_config_path_override(path: PathBuf) {
    let _ = CONFIG_PATH_OVERRIDE.set(path);
}

/// Set a CLI override for the data directory. Subsequent calls to
/// [`user_data_dir`] will return this value, regardless of `username`.
///
/// First writer wins.
pub fn set_data_dir_override(path: PathBuf) {
    let _ = DATA_DIR_OVERRIDE.set(path);
}

fn config_path_override() -> Option<PathBuf> {
    CONFIG_PATH_OVERRIDE.get().cloned()
}

fn data_dir_override() -> Option<PathBuf> {
    DATA_DIR_OVERRIDE.get().cloned()
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    let owned = value.to_string_lossy();
    let trimmed = owned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn config_exists(username: &str) -> bool {
    config_path(username).is_file()
}

pub fn user_exists(username: &str) -> bool {
    users::get_user_by_name(username).is_some()
}

pub fn read_config_from_path(config_path: &Path) -> Result<BiopassConfig, String> {
    let config_text = fs::read_to_string(config_path)
        .map_err(|error| format!("Failed to read config {}: {error}", config_path.display()))?;
    serde_yaml::from_str::<BiopassConfig>(&config_text)
        .map_err(|error| config_parse_error_message(config_path, &error.to_string()))
}

pub fn read_config(username: &str) -> Result<BiopassConfig, String> {
    read_config_from_path(&config_path(username))
}

pub fn list_faces(username: &str) -> Vec<PathBuf> {
    let faces_dir = user_data_dir(username).join("faces");
    let Ok(entries) = fs::read_dir(faces_dir) else {
        return Vec::new();
    };

    let mut faces = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_supported_face_image(path))
        .collect::<Vec<_>>();
    faces.sort();
    faces
}

pub fn setup_config(username: &str) -> std::io::Result<()> {
    let data_dir = user_data_dir(username);
    fs::create_dir_all(data_dir.join("faces"))?;
    fs::create_dir_all(data_dir.join("debugs"))?;
    Ok(())
}

/// Serialize a [`BiopassConfig`] to disk, creating the parent directory if
/// needed. Used by `config reset` and the GUI when persisting changes.
pub fn write_config_to_path(path: &Path, config: &BiopassConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }
    let yaml = serde_yaml::to_string(config)
        .map_err(|error| format!("Failed to serialize config: {error}"))?;
    fs::write(path, yaml).map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

/// Force-rewrite the config at `path` with built-in defaults. Always
/// overwrites; used by `config reset` and the GUI "Reset to defaults" flow.
pub fn reset_config_at_path(path: &Path) -> Result<(), String> {
    write_config_to_path(path, &BiopassConfig::default())
}

pub fn reset_config(username: &str) -> Result<(), String> {
    reset_config_at_path(&config_path(username))
}

pub(super) fn is_supported_face_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp" | "tga"
            )
        })
        .unwrap_or(false)
}
