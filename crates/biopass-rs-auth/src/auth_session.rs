use crate::{
    config_path, read_config_from_path, set_runtime_logging, user_data_dir, user_exists,
    write_auth_summary, AuthManager, AuthOutcome, BiopassConfig, FaceAuth, FingerprintAuth,
    LogComponent, LogLevel, PamCode, RuntimeLoggingConfig,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", content = "pam_code", rename_all = "snake_case")]
pub enum AuthSessionStatus {
    Completed(PamCode),
    Ignored,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AuthSessionResult {
    #[serde(flatten)]
    pub status: AuthSessionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionPaths {
    pub config_path: PathBuf,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthSessionOptions {
    pub log_level_override: Option<LogLevel>,
}

impl AuthSessionResult {
    pub fn pam_code(&self) -> PamCode {
        match self.status {
            AuthSessionStatus::Completed(code) => code,
            AuthSessionStatus::Ignored => PamCode::Ignore,
        }
    }
}

pub fn authenticate_user(
    username: &str,
    service: Option<&str>,
) -> Result<AuthSessionResult, String> {
    authenticate_user_with_options(
        username,
        service,
        AuthSessionPaths {
            config_path: config_path(username),
            data_dir: user_data_dir(username),
        },
        user_exists(username),
        AuthSessionOptions::default(),
    )
}

pub fn authenticate_user_with(
    username: &str,
    service: Option<&str>,
    paths: AuthSessionPaths,
    user_exists: bool,
) -> Result<AuthSessionResult, String> {
    authenticate_user_with_options(
        username,
        service,
        paths,
        user_exists,
        AuthSessionOptions::default(),
    )
}

pub fn authenticate_user_with_options(
    username: &str,
    service: Option<&str>,
    paths: AuthSessionPaths,
    user_exists: bool,
    options: AuthSessionOptions,
) -> Result<AuthSessionResult, String> {
    if !user_exists {
        return Ok(AuthSessionResult {
            status: AuthSessionStatus::Ignored,
        });
    }

    if !paths.config_path.is_file() {
        return Ok(AuthSessionResult {
            status: AuthSessionStatus::Ignored,
        });
    }

    let config = read_config_from_path(&paths.config_path)?;
    if service.is_some_and(|service| config.ignores_service(service)) {
        return Ok(AuthSessionResult {
            status: AuthSessionStatus::Ignored,
        });
    }

    let mut runtime_logging = RuntimeLoggingConfig::from_config(paths.data_dir, &config.logging);
    if let Some(level) = options.log_level_override {
        runtime_logging.file_enabled = true;
        runtime_logging.file_level = level;
    }
    set_runtime_logging(runtime_logging);

    if config.auth_methods().is_empty() {
        return Ok(AuthSessionResult {
            status: AuthSessionStatus::Ignored,
        });
    }

    let mut manager = build_auth_manager(&config);
    let mut outcome = manager.authenticate(username);
    outcome.summary.service = service.map(str::to_string);
    write_summary(&outcome);

    Ok(AuthSessionResult {
        status: AuthSessionStatus::Completed(outcome.code),
    })
}

pub fn build_auth_manager(config: &BiopassConfig) -> AuthManager {
    let mut manager = AuthManager::new();
    manager.set_mode(config.execution_mode());
    manager.set_config(config.runtime_auth_config());
    for method in config.auth_methods() {
        match method.name.as_str() {
            "face" => manager.add_method(Box::new(FaceAuth::new(config.methods.face.clone()))),
            "fingerprint" => manager.add_method(Box::new(FingerprintAuth::new(
                config.methods.fingerprint.clone(),
            ))),
            _ => {}
        }
    }
    manager
}

fn write_summary(outcome: &AuthOutcome) {
    if let Err(error) = write_auth_summary(&outcome.summary) {
        crate::emit_log(
            LogComponent::Auth,
            LogLevel::Warn,
            "auth_session",
            &format!("failed to write auth summary: {error}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_config_to_path;

    #[test]
    fn authenticate_user_with_ignores_missing_user_before_reading_config() {
        let directory = tempfile::tempdir().unwrap();
        let paths = AuthSessionPaths {
            config_path: directory.path().join("missing.yaml"),
            data_dir: directory.path().join("data"),
        };

        let result = authenticate_user_with("alice", Some("sudo"), paths, false).unwrap();

        assert_eq!(result.pam_code(), PamCode::Ignore);
    }

    #[test]
    fn authenticate_user_with_ignores_missing_config() {
        let directory = tempfile::tempdir().unwrap();
        let paths = AuthSessionPaths {
            config_path: directory.path().join("missing.yaml"),
            data_dir: directory.path().join("data"),
        };

        let result = authenticate_user_with("alice", Some("sudo"), paths, true).unwrap();

        assert_eq!(result.pam_code(), PamCode::Ignore);
    }

    #[test]
    fn authenticate_user_with_ignores_configured_service() {
        let directory = tempfile::tempdir().unwrap();
        let config_path = directory.path().join("config.yaml");
        let data_dir = directory.path().join("data");
        let mut config = BiopassConfig::default_for_data_dir(&data_dir);
        config.strategy.ignore_services = vec!["sudo".to_string()];
        write_config_to_path(&config_path, &config).unwrap();

        let result = authenticate_user_with(
            "alice",
            Some("sudo"),
            AuthSessionPaths {
                config_path,
                data_dir,
            },
            true,
        )
        .unwrap();

        assert_eq!(result.pam_code(), PamCode::Ignore);
    }

    #[test]
    fn build_auth_manager_ignores_when_no_methods_are_enabled() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = BiopassConfig::default_for_data_dir(directory.path());
        config.methods.face.enable = false;
        config.methods.fingerprint.enable = false;

        let mut manager = build_auth_manager(&config);
        let outcome = manager.authenticate("alice");

        assert_eq!(outcome.code, PamCode::Ignore);
        assert!(!outcome.attempted);
    }

    #[test]
    fn authenticate_user_with_options_can_override_file_log_level() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let config_path = directory.path().join("config.yaml");
        let data_dir = directory.path().join("data");
        let mut config = BiopassConfig::default_for_data_dir(&data_dir);
        config.methods.face.enable = false;
        config.methods.fingerprint.enable = false;
        config.logging.file.enabled = false;
        config.logging.file.level = "error".to_string();
        write_config_to_path(&config_path, &config).unwrap();

        let _ = authenticate_user_with_options(
            "alice",
            Some("sudo"),
            AuthSessionPaths {
                config_path,
                data_dir: data_dir.clone(),
            },
            true,
            AuthSessionOptions {
                log_level_override: Some(LogLevel::Debug),
            },
        )
        .unwrap();

        let runtime = crate::runtime_logging();
        assert!(runtime.file_enabled);
        assert_eq!(runtime.file_level, LogLevel::Debug);
        assert_eq!(runtime.data_dir, data_dir);
    }
}
