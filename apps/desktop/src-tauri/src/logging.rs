//! Desktop-layer logging glue.
//!
//! Thin wrappers over the [`biopass_rs_auth`] logging primitives that bind
//! every emission to [`LogComponent::Desktop`], plus the startup and panic
//! wiring needed so app crashes and config-deserialization failures are
//! captured to disk instead of vanishing into the void.

use std::path::PathBuf;

use biopass_rs_auth::{
    emit_log, set_runtime_logging, LogComponent, LogLevel, LoggingConfig, RuntimeLoggingConfig,
};
use tauri::AppHandle;

use crate::config::{load_config_internal, LoadConfigResult};
use crate::paths::get_data_dir;

/// Append a line to the Desktop component log. A thin scope-binding wrapper so
/// call sites don't repeat [`LogComponent::Desktop`] at every emission.
pub fn desktop_log(level: LogLevel, scope: &str, message: &str) {
    emit_log(LogComponent::Desktop, level, scope, message);
}

/// Best-effort refresh of the global logging config from the on-disk config.
///
/// Unlike the old `configure_logging` helper, this never fails its caller: on
/// a broken config it leaves the previously-seeded runtime logging untouched
/// and records the parse error to the Desktop log instead. Call this at the
/// top of any command that emits Desktop logs so level changes take effect
/// even after the auth session re-initializes runtime logging mid-run.
pub fn ensure_logging(app: &AppHandle) {
    let data_dir = match get_data_dir(app) {
        Ok(dir) => dir,
        Err(error) => {
            desktop_log(
                LogLevel::Warn,
                "logging",
                &format!("data dir unavailable, skipping reconfigure: {error}"),
            );
            return;
        }
    };
    match load_config_internal(app) {
        Ok(LoadConfigResult::Loaded { config, .. }) => {
            set_runtime_logging(RuntimeLoggingConfig::from_config(data_dir, &config.logging));
        }
        Ok(LoadConfigResult::Broken { message, .. }) => {
            desktop_log(
                LogLevel::Error,
                "config",
                &format!("config unreadable, keeping startup logging defaults: {message}"),
            );
        }
        Err(error) => {
            desktop_log(
                LogLevel::Error,
                "config",
                &format!("failed to load config for logging: {error}"),
            );
        }
    }
}

/// Seed runtime logging at the very start of `setup`, then re-assert from the
/// real config once it loads. Returns the resolved data dir so the caller can
/// reuse it for asset-protocol scoping.
///
/// Seeding with [`LoggingConfig::default`] (file logging on at `info`) pins
/// the data dir before the config is parsed, so any panic later in startup —
/// including a config parse failure — still lands on disk.
pub fn init_startup_logging(app: &AppHandle) -> PathBuf {
    let data_dir = get_data_dir(app).unwrap_or_else(|_| PathBuf::from("."));
    set_runtime_logging(RuntimeLoggingConfig::from_config(
        data_dir.clone(),
        &LoggingConfig::default(),
    ));

    match load_config_internal(app) {
        Ok(LoadConfigResult::Loaded {
            path,
            migrated,
            initialized,
            config,
        }) => {
            set_runtime_logging(RuntimeLoggingConfig::from_config(
                data_dir.clone(),
                &config.logging,
            ));
            let tag = if initialized {
                "initialized"
            } else if migrated {
                "migrated"
            } else {
                "loaded"
            };
            desktop_log(
                LogLevel::Info,
                "startup",
                &format!("config {tag} from {path}"),
            );
        }
        Ok(LoadConfigResult::Broken { path, message }) => {
            desktop_log(
                LogLevel::Error,
                "startup",
                &format!("config parse failed at {path}: {message}"),
            );
        }
        Err(error) => {
            desktop_log(
                LogLevel::Error,
                "startup",
                &format!("config load error: {error}"),
            );
        }
    }
    data_dir
}

/// Install a panic hook that records the panic to the Desktop log before
/// forwarding to the previous hook (the default one prints to stderr).
///
/// The previous hook is captured once at install time and moved into the new
/// closure — taking the hook from inside the handler would recurse.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        desktop_log(LogLevel::Error, "panic", &format!("{info}"));
        previous(info);
    }));
}
