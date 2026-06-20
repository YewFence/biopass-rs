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

pub fn list_faces(username: &str) -> Vec<PathBuf> {
    crate::list_enrolled_faces(&user_data_dir(username)).unwrap_or_default()
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
pub fn reset_config_at_path(path: &Path, data_dir: &Path) -> Result<(), String> {
    write_config_to_path(path, &BiopassConfig::default_for_data_dir(data_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn with_env_var<T>(
        key: &str,
        value: Option<&Path>,
        f: impl FnOnce() -> T + std::panic::UnwindSafe,
    ) -> T {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let previous = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        let result = std::panic::catch_unwind(f);

        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }

        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

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

    #[test]
    fn env_path_trims_and_absolutizes_values() {
        let directory = tempfile::tempdir().unwrap();
        let relative = PathBuf::from("dev-config.yaml");

        with_env_var(CONFIG_PATH_ENV, Some(&relative), || {
            let path = env_path(CONFIG_PATH_ENV).unwrap();
            assert!(path.is_absolute());
            assert!(path.ends_with("dev-config.yaml"));
        });

        with_env_var(DATA_DIR_ENV, Some(directory.path()), || {
            assert_eq!(env_path(DATA_DIR_ENV).unwrap(), directory.path());
        });
    }

    #[test]
    fn env_path_ignores_missing_or_blank_values() {
        with_env_var(CONFIG_PATH_ENV, None, || {
            assert!(env_path(CONFIG_PATH_ENV).is_none());
        });

        let blank = Path::new("   ");
        with_env_var(CONFIG_PATH_ENV, Some(blank), || {
            assert!(env_path(CONFIG_PATH_ENV).is_none());
        });
    }

    #[test]
    fn config_parse_error_message_mentions_recovery_command() {
        let message = config_parse_error_message(Path::new("/tmp/config.yaml"), "bad yaml");

        assert!(message.contains("/tmp/config.yaml"));
        assert!(message.contains("bad yaml"));
        assert!(message.contains("biopass-rs-helper config reset"));
    }

    #[test]
    fn write_config_creates_parent_and_read_config_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".config/biopass-rs/config.yaml");
        let config = BiopassConfig::default_for_data_dir(directory.path());

        write_config_to_path(&path, &config).unwrap();

        assert_eq!(read_config_from_path(&path).unwrap(), config);
    }

    #[test]
    fn read_config_reports_missing_file_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.yaml");

        let error = read_config_from_path(&path).unwrap_err();

        assert!(error.contains("Failed to read config"));
        assert!(error.contains(&path.display().to_string()));
    }

    #[test]
    fn setup_config_creates_faces_and_debug_directories() {
        let directory = tempfile::tempdir().unwrap();
        with_env_var(DATA_DIR_ENV, Some(directory.path()), || {
            setup_config("missing-user").unwrap();
        });

        assert!(directory.path().join("faces").is_dir());
        assert!(directory.path().join("debugs").is_dir());
    }
}
