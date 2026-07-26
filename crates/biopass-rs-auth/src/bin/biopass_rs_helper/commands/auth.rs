use biopass_rs_auth::{authenticate_user_with_options, AuthSessionOptions, LogLevel, PamCode};

pub(crate) const EXIT_SUCCESS: u8 = 0;
pub(crate) const EXIT_AUTH_ERR: u8 = 1;
pub(crate) const EXIT_IGNORE: u8 = 2;

pub(crate) fn authenticate(
    username: Option<&str>,
    service: Option<&str>,
    log_level: Option<&str>,
) -> u8 {
    let Some(username) = username.filter(|name| !name.is_empty()) else {
        eprintln!("auth: no target user provided and none could be inferred from the environment");
        return EXIT_AUTH_ERR;
    };

    let log_level_override = match log_level {
        Some(level) => match LogLevel::from_name(level) {
            Some(level) => Some(level),
            None => {
                eprintln!("auth: invalid log level '{level}'");
                return EXIT_AUTH_ERR;
            }
        },
        None => None,
    };

    let result = match authenticate_user_with_options(
        username,
        service,
        biopass_rs_auth::AuthSessionPaths {
            config_path: biopass_rs_auth::config_path(username),
            data_dir: biopass_rs_auth::user_data_dir(username),
        },
        biopass_rs_auth::user_exists(username),
        AuthSessionOptions { log_level_override },
    ) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("auth: {error}");
            return EXIT_AUTH_ERR;
        }
    };
    match result.pam_code() {
        PamCode::Success => EXIT_SUCCESS,
        PamCode::Ignore => EXIT_IGNORE,
        PamCode::AuthError => EXIT_AUTH_ERR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticate_rejects_missing_or_empty_username() {
        assert_eq!(authenticate(None, Some("sudo"), None), EXIT_AUTH_ERR);
        assert_eq!(authenticate(Some(""), Some("sudo"), None), EXIT_AUTH_ERR);
    }

    #[test]
    fn authenticate_rejects_invalid_log_level() {
        assert_eq!(
            authenticate(Some("alice"), Some("sudo"), Some("verbose")),
            EXIT_AUTH_ERR
        );
    }
}
