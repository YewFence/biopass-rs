//! Shared "make sure a config file exists at this path" bootstrap logic.
//!
//! Three call sites need the same three-step fallback chain:
//!
//! 1. **`installer::migrate_all_users`** — invoked by `biopass-rs-helper install`,
//!    runs once per system user and only triggers when the destination does
//!    not already exist.
//! 2. **`config init`** (helper CLI) — invoked by a single user from the
//!    terminal; same chain but for one target user.
//! 3. **Desktop GUI** — invoked on startup when the GUI cannot find a config
//!    file at its expected location.
//!
//! The chain is: (a) try to import the upstream TickLabVN `biopass` config
//! and migrate its schema, (b) otherwise write the built-in default config
//! produced by `default_factory`. Steps (b)'s `default_factory` is a closure
//! so the GUI can inject dynamic model paths without the library crate
//! depending on Tauri.

use super::migration::migrate_config_at_path;
use super::paths::{read_config_from_path, write_config_to_path};
use super::schema::BiopassConfig;
use std::fs;
use std::path::{Path, PathBuf};

const UPSTREAM_CONFIG_PATH: &str = ".config/com.ticklab.biopass/config.yaml";

/// Outcome of a `bootstrap_config_at` call. The variants distinguish the
/// three relevant states so callers (GUI, CLI) can show different messages.
#[derive(Debug, Clone, PartialEq)]
pub enum BootstrapOutcome {
    /// The destination file already existed; nothing was written.
    AlreadyPresent,
    /// The upstream `biopass` config was copied in and migrated to the
    /// current schema.
    ImportedFromUpstream,
    /// The built-in defaults were written because nothing else applied.
    WroteDefaults,
}

/// Ensure a config file exists at `destination`.
///
/// * If the file already exists, return [`BootstrapOutcome::AlreadyPresent`]
///   without touching it.
/// * Otherwise, if `upstream_home` contains a TickLabVN `biopass` config,
///   copy it in and migrate it to the current schema.
/// * Otherwise, write `default_factory()` to `destination`.
///
/// `upstream_home` is the home directory used to locate the upstream config.
/// Pass `None` to skip the upstream import attempt entirely (e.g. when the
/// caller has no concept of a per-user home directory).
pub fn bootstrap_config_at(
    destination: &Path,
    upstream_home: Option<&Path>,
    default_factory: impl FnOnce() -> BiopassConfig,
) -> Result<BootstrapOutcome, String> {
    if destination.is_file() {
        return Ok(BootstrapOutcome::AlreadyPresent);
    }

    if let Some(home) = upstream_home {
        let upstream = home.join(UPSTREAM_CONFIG_PATH);
        if upstream.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
            }
            match try_import_upstream(&upstream, destination) {
                Ok(()) => return Ok(BootstrapOutcome::ImportedFromUpstream),
                Err(err) => {
                    // Clean up partial copies and fall through to defaults.
                    eprintln!(
                        "Warning: failed to import upstream config from {}: {err}",
                        upstream.display()
                    );
                    let _ = fs::remove_file(destination);
                }
            }
        }
    }

    write_config_to_path(destination, &default_factory())?;
    Ok(BootstrapOutcome::WroteDefaults)
}

fn try_import_upstream(source: &Path, destination: &Path) -> Result<(), String> {
    fs::copy(source, destination)
        .map_err(|e| format!("copy {} → {}: {e}", source.display(), destination.display()))?;
    migrate_config_at_path(destination)
        .map_err(|e| format!("migrate {}: {e}", destination.display()))?;
    // Make sure the result parses; if not, bubble the structured error up so
    // the caller drops the file and falls back to defaults.
    read_config_from_path(destination).map(|_| ())
}

/// The upstream-config path relative to a user's home directory. Exported so
/// other modules (the installer) can probe for it without duplicating the
/// constant.
pub fn upstream_config_path_relative() -> PathBuf {
    PathBuf::from(UPSTREAM_CONFIG_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn bootstrap_writes_defaults_when_no_upstream() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(".config/biopass-rs/config.yaml");
        let fake_home = dir.path().join("home");

        let outcome = bootstrap_config_at(&dest, Some(&fake_home), BiopassConfig::default)
            .expect("bootstrap should succeed");

        assert_eq!(outcome, BootstrapOutcome::WroteDefaults);
        assert!(dest.is_file());
    }

    #[test]
    fn bootstrap_is_idempotent_when_destination_exists() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("config.yaml");
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(&dest, "preexisting: true").unwrap();
        let original = fs::read_to_string(&dest).unwrap();

        let outcome =
            bootstrap_config_at(&dest, None, BiopassConfig::default).expect("bootstrap ok");

        assert_eq!(outcome, BootstrapOutcome::AlreadyPresent);
        assert_eq!(fs::read_to_string(&dest).unwrap(), original);
    }

    #[test]
    fn bootstrap_imports_and_migrates_upstream_config() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let upstream = home.join(UPSTREAM_CONFIG_PATH);
        fs::create_dir_all(upstream.parent().unwrap()).unwrap();
        // Legacy 'ai' key — will be migrated to 'rgb' by migrate_config_at_path.
        fs::write(
            &upstream,
            r#"
methods:
  face:
    anti_spoofing:
      ai:
        enable: false
        model:
          path: legacy.onnx
          threshold: 0.5
"#,
        )
        .unwrap();

        let dest = home.join(".config/biopass-rs/config.yaml");
        let outcome =
            bootstrap_config_at(&dest, Some(home), BiopassConfig::default).expect("bootstrap ok");

        assert_eq!(outcome, BootstrapOutcome::ImportedFromUpstream);
        let migrated = fs::read_to_string(&dest).unwrap();
        assert!(
            migrated.contains("rgb:"),
            "expected migrated content to contain rgb key, got: {migrated}"
        );
        assert!(
            !migrated.contains(" ai:"),
            "expected migrated content to drop ai key, got: {migrated}"
        );
    }
}
