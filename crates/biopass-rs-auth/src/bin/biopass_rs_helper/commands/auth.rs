use biopass_rs_auth::{authenticate_user, PamCode};

pub(crate) const EXIT_SUCCESS: u8 = 0;
pub(crate) const EXIT_AUTH_ERR: u8 = 1;
pub(crate) const EXIT_IGNORE: u8 = 2;

pub(crate) fn authenticate(username: Option<&str>, service: Option<&str>) -> u8 {
    let Some(username) = username.filter(|name| !name.is_empty()) else {
        eprintln!("auth: no target user provided and none could be inferred from the environment");
        return EXIT_AUTH_ERR;
    };

    let result = match authenticate_user(username, service) {
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
