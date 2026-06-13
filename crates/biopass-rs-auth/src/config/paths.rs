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
    resolve_user_home(username, PathBuf::from("/etc/biopass-rs")).join(CONFIG_FILE)
}

/// Resolve the active data directory for `username` (faces / debugs / ...).
///
/// Resolution order:
/// 1. CLI override (set by [`set_data_dir_override`]).
/// 2. `BIOPASS_DATA_DIR` environment variable.
/// 3. The home directory from the system user database joined with
///    [`DATA_DIR`].
/// 4. `$HOME` joined with [`DATA_DIR`].
/// 5. `/etc/biopass-rs` as a last resort.
pub fn user_data_dir(username: &str) -> PathBuf {
    if let Some(override_dir) = data_dir_override() {
        return override_dir;
    }
    if let Some(env_dir) = env_path(DATA_DIR_ENV) {
        return env_dir;
    }
    resolve_user_home(username, PathBuf::from("/etc/biopass-rs")).join(DATA_DIR)
}

/// Set a CLI override for the config path. Subsequent calls to
/// [`config_path`] will return this value, regardless of `username`.
///
/// First writer wins — calling this more than once is a no-op so that
/// downstream code (e.g. tests) can layer overrides safely.
pub fn set_config_path_override(path: PathBuf) {
    let _ = CONFIG_PATH_OVERRIDE.set(absolutize_configured_path(path));
}

/// Set a CLI override for the data directory. Subsequent calls to
/// [`user_data_dir`] will return this value, regardless of `username`.
///
/// First writer wins.
pub fn set_data_dir_override(path: PathBuf) {
    let _ = DATA_DIR_OVERRIDE.set(absolutize_configured_path(path));
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
        Some(absolutize_configured_path(PathBuf::from(trimmed)))
    }
}

fn absolutize_configured_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(&path))
        .unwrap_or(path)
}

/// Resolve a user's home directory, falling back to $HOME and then a provided
/// default path.
fn resolve_user_home(username: &str, fallback: PathBuf) -> PathBuf {
    users::get_user_by_name(username)
        .map(|user| user.home_dir().to_path_buf())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or(fallback)
}

/// Best-effort lookup of the current process's username.
///
/// Under `sudo` we honour `SUDO_USER` (the invoking user) rather than the
/// effective UID, so the helper invoked from a PAM session still resolves
/// the target user's config / data dir. Outside sudo we ask the OS via
/// [`users::get_current_uid`] for a stable, NSS-aware answer.
///
/// Used as the default `username` argument to [`config_path`] and
/// [`user_data_dir`] from both the helper CLI and the desktop GUI, so the
/// two stay in sync about whose config / data dir to read.
pub fn current_username() -> Option<String> {
    if let Some(value) = std::env::var_os("SUDO_USER") {
        let trimmed = value.to_string_lossy().trim().to_owned();
        if !trimmed.is_empty()
            && trimmed
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
        {
            return Some(trimmed);
        }
    }
    users::get_user_by_uid(users::get_current_uid())
        .map(|user| user.name().to_string_lossy().into_owned())
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

/// Rewrite any relative model paths in the config file at `config_path` as
/// absolute paths rooted at DATA_DIR, persisting the result to disk.
///
/// Model paths may be written as bare relative values like
/// `models/yolov8n-face.onnx`. Such paths are ambiguous at runtime because
/// they resolve against the process's current working directory, which differs
/// between the helper CLI, the PAM module, and the desktop GUI. This function
/// resolves them once, against the target user's DATA_DIR, and writes the
/// absolute paths back so every reader sees the same location.
///
/// Absolute paths and empty strings are left untouched. Returns `true` if the
/// on-disk file was rewritten.
pub fn normalize_config_paths_at_path(config_path: &Path) -> Result<bool, String> {
    let mut config = read_config_from_path(config_path)?;
    let before = config.clone();
    normalize_model_paths(&mut config, config_path);
    if config == before {
        return Ok(false);
    }
    write_config_to_path(config_path, &config)?;
    Ok(true)
}

/// Resolve relative model paths in `config` against DATA_DIR.
///
/// - Absolute paths (start with `/`) and empty strings are left unchanged.
/// - Relative paths are joined onto the DATA_DIR resolved for the user that
///   owns `config_path` (inferred from the path, falling back to the current
///   process user).
fn normalize_model_paths(config: &mut BiopassConfig, config_path: &Path) {
    let username = infer_username_from_config_path(config_path);
    let data_dir = user_data_dir(&username);

    let normalize = |path: &str| -> String {
        if path.is_empty() || Path::new(path).is_absolute() {
            path.to_string()
        } else {
            data_dir.join(path).to_string_lossy().to_string()
        }
    };

    config.methods.face.detection.model = normalize(&config.methods.face.detection.model);
    config.methods.face.recognition.model = normalize(&config.methods.face.recognition.model);
    config.methods.face.anti_spoofing.rgb.model.path =
        normalize(&config.methods.face.anti_spoofing.rgb.model.path);
    config.methods.face.anti_spoofing.ir.model.path =
        normalize(&config.methods.face.anti_spoofing.ir.model.path);

    for model in &mut config.models {
        model.path = normalize(&model.path);
    }
}

/// Best-effort extraction of the username from a config path.
///
/// Standard layout is `/home/<user>/.config/biopass-rs/config.yaml`; we walk
/// the path looking for the `.config` (or `.local`) segment and take the
/// preceding component as the username. Falls back to the current process
/// user when the path does not match the standard layout.
fn infer_username_from_config_path(config_path: &Path) -> String {
    for ancestor in config_path.ancestors() {
        if let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) {
            if name == ".config" || name == ".local" {
                if let Some(parent) = ancestor.parent() {
                    if let Some(username) = parent.file_name().and_then(|n| n.to_str()) {
                        return username.to_string();
                    }
                }
            }
        }
    }

    current_username().unwrap_or_else(|| "current".to_string())
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
    write_config_to_path(path, &BiopassConfig::default())?;
    // Resolve relative model paths against DATA_DIR so the reset config is
    // immediately usable by every reader (CLI / PAM / GUI).
    let _ = normalize_config_paths_at_path(path)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::absolutize_configured_path;
    use std::path::PathBuf;

    #[test]
    fn configured_relative_paths_are_rooted_at_current_dir() {
        let path = absolutize_configured_path(PathBuf::from("dev-data"));

        assert!(path.is_absolute());
        assert!(path.ends_with("dev-data"));
    }

    #[test]
    fn configured_absolute_paths_are_kept() {
        let path = PathBuf::from("/tmp/biopass-rs-dev-data");

        assert_eq!(absolutize_configured_path(path.clone()), path);
    }
}
