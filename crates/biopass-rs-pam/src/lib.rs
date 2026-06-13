use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::process::Command;

const PAM_SUCCESS: c_int = 0;
const PAM_AUTH_ERR: c_int = 7;
const PAM_IGNORE: c_int = 25;
const PAM_SERVICE: c_int = 1;

type PamHandle = c_void;

extern "C" {
    fn pam_get_user(pamh: *mut PamHandle, user: *mut *const c_char, prompt: *const c_char)
        -> c_int;
    fn pam_get_item(pamh: *mut PamHandle, item_type: c_int, item: *mut *const c_void) -> c_int;
}

/// PAM authentication module entry point.
///
/// # Safety
///
/// This function is called by the PAM framework with raw C pointers.
/// The caller must ensure that `pamh` is a valid PAM handle and that
/// `_argv` (if non-null) points to a valid array of C strings.
#[no_mangle]
pub unsafe extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    let username = match pam_username(pamh) {
        Ok(username) => username,
        Err(code) => return code,
    };
    let service = pam_service(pamh);

    run_helper(&username, service.as_deref())
}

#[no_mangle]
pub extern "C" fn pam_sm_open_session(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_acct_mgmt(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_close_session(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_chauthtok(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_setcred(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

unsafe fn pam_username(pamh: *mut PamHandle) -> Result<String, c_int> {
    let mut username = std::ptr::null();
    let result = pam_get_user(pamh, &mut username, std::ptr::null());
    if result != PAM_SUCCESS {
        return Err(result);
    }

    c_string_to_string(username).ok_or(PAM_AUTH_ERR)
}

unsafe fn pam_service(pamh: *mut PamHandle) -> Option<String> {
    let mut service = std::ptr::null();
    let result = pam_get_item(pamh, PAM_SERVICE, &mut service);
    if result != PAM_SUCCESS || service.is_null() {
        return None;
    }

    c_string_to_string(service.cast())
}

unsafe fn c_string_to_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }

    CStr::from_ptr(value).to_str().ok().map(ToOwned::to_owned)
}

fn run_helper(username: &str, service: Option<&str>) -> c_int {
    let mut command = Command::new("/usr/bin/biopass-rs-helper");
    command.args(["--username", username, "auth"]);
    if let Some(service) = service.filter(|service| !service.is_empty()) {
        command.args(["--service", service]);
    }

    match command.status() {
        Ok(status) => map_helper_exit(status.code()),
        Err(_) => PAM_AUTH_ERR,
    }
}

fn map_helper_exit(code: Option<i32>) -> c_int {
    match code {
        Some(0) => PAM_SUCCESS,
        Some(2) => PAM_IGNORE,
        _ => PAM_AUTH_ERR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_helper_success() {
        assert_eq!(map_helper_exit(Some(0)), PAM_SUCCESS);
    }

    #[test]
    fn maps_helper_ignore() {
        assert_eq!(map_helper_exit(Some(2)), PAM_IGNORE);
    }

    #[test]
    fn maps_helper_failures_to_auth_error() {
        assert_eq!(map_helper_exit(Some(1)), PAM_AUTH_ERR);
        assert_eq!(map_helper_exit(None), PAM_AUTH_ERR);
    }
}
