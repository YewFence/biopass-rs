//! Shared "make sure a config file exists at this path" bootstrap logic.
//!
//! Two call sites need the same "write defaults if absent" behavior:
//!
//! 1. **`config init`** (helper CLI) — invoked by a single user from the
//!    terminal; also invoked by `biopass-rs-helper install` for the current
//!    user.
//! 2. **Desktop GUI** — invoked on startup when the GUI cannot find a config
//!    file at its expected location.
//!
//! Bootstrap only writes built-in defaults. It deliberately does **not**
//! import or migrate the upstream TickLabVN `biopass` config: the upstream
//! schema drifts independently and chasing every version is unsustainable.
//! Instead the helper's `install` command copies the upstream **face images**
//! (which are schema-independent) — see `commands/install.rs`.

use super::paths::{normalize_config_paths_at_path, write_config_to_path};
use super::schema::BiopassConfig;
use std::path::Path;

/// Outcome of a `bootstrap_config_at` call.
#[derive(Debug, Clone, PartialEq)]
pub enum BootstrapOutcome {
    /// The destination file already existed; nothing was written.
    AlreadyPresent,
    /// The built-in defaults were written because the file was absent.
    WroteDefaults,
}

/// Ensure a config file exists at `destination`.
///
/// * If the file already exists, return [`BootstrapOutcome::AlreadyPresent`]
///   without touching it.
/// * Otherwise, write `default_factory()` to `destination` and resolve its
///   relative model paths against DATA_DIR so the freshly-written config is
///   self-contained regardless of which reader (CLI / PAM / GUI) loads it.
pub fn bootstrap_config_at(
    destination: &Path,
    default_factory: impl FnOnce() -> BiopassConfig,
) -> Result<BootstrapOutcome, String> {
    if destination.is_file() {
        return Ok(BootstrapOutcome::AlreadyPresent);
    }

    write_config_to_path(destination, &default_factory())?;
    let _ = normalize_config_paths_at_path(destination)?;
    Ok(BootstrapOutcome::WroteDefaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn bootstrap_writes_defaults_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(".config/biopass-rs/config.yaml");

        let outcome =
            bootstrap_config_at(&dest, BiopassConfig::default).expect("bootstrap should succeed");

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

        let outcome = bootstrap_config_at(&dest, BiopassConfig::default).expect("bootstrap ok");

        assert_eq!(outcome, BootstrapOutcome::AlreadyPresent);
        assert_eq!(fs::read_to_string(&dest).unwrap(), original);
    }
}
