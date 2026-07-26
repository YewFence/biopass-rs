use biopass_rs_auth::{
    config_path, read_config_from_path, set_runtime_logging, user_data_dir, user_exists,
    write_auth_summary, AuthManager, FaceAuth, FingerprintAuth, PamCode, RuntimeLoggingConfig,
};

pub(crate) const EXIT_SUCCESS: u8 = 0;
pub(crate) const EXIT_AUTH_ERR: u8 = 1;
pub(crate) const EXIT_IGNORE: u8 = 2;

pub(crate) fn authenticate(_username: Option<&str>, service: Option<&str>) -> u8 {
    let Some(username) = _username.filter(|name| !name.is_empty()).map(str::to_owned) else {
        eprintln!("auth: no target user provided and none could be inferred from the environment");
        return EXIT_AUTH_ERR;
    };

    if !user_exists(&username) {
        return EXIT_IGNORE;
    }

    let config_path = config_path(&username);
    if !config_path.is_file() {
        return EXIT_IGNORE;
    }

    let config = match read_config_from_path(&config_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("auth: {error}");
            return EXIT_AUTH_ERR;
        }
    };
    if service.is_some_and(|service| config.ignores_service(service)) {
        return EXIT_IGNORE;
    }

    set_runtime_logging(RuntimeLoggingConfig::from_config(
        user_data_dir(&username),
        &config.logging,
    ));

    let methods = config.auth_methods();
    if methods.is_empty() {
        return EXIT_IGNORE;
    }

    let mut manager = AuthManager::new();
    manager.set_mode(config.execution_mode());
    manager.set_config(config.runtime_auth_config());
    for method in methods {
        match method.name.as_str() {
            "face" => manager.add_method(Box::new(FaceAuth::new(config.methods.face.clone()))),
            "fingerprint" => manager.add_method(Box::new(FingerprintAuth::new(
                config.methods.fingerprint.clone(),
            ))),
            _ => {}
        }
    }

    let mut outcome = manager.authenticate(&username);
    outcome.summary.service = service.map(str::to_string);
    if let Err(error) = write_auth_summary(&outcome.summary) {
        biopass_rs_auth::emit_log(
            biopass_rs_auth::LogComponent::Auth,
            biopass_rs_auth::LogLevel::Warn,
            "helper:auth",
            &format!("failed to write auth summary: {error}"),
        );
    }
    match outcome.code {
        PamCode::Success => EXIT_SUCCESS,
        PamCode::Ignore => EXIT_IGNORE,
        PamCode::AuthError => EXIT_AUTH_ERR,
    }
}
