use biopass_auth::{EnrollStatusCallback, FingerprintAuth as BioFingerprintAuth};
use tauri::{AppHandle, Emitter};

struct TauriEnrollCallback<'a> {
    app: &'a AppHandle,
}

impl EnrollStatusCallback for TauriEnrollCallback<'_> {
    fn on_status(&self, status: &str, done: bool) {
        #[derive(serde::Serialize, Clone)]
        struct ProgressPayload {
            done: bool,
            status: String,
        }

        self.app
            .emit(
                "fingerprint-enroll-status",
                ProgressPayload {
                    done,
                    status: status.to_string(),
                },
            )
            .ok();
    }
}

/// Rust fprintd client kept behind the old module name so Tauri commands stay compatible.
pub struct FingerprintAuth;

impl FingerprintAuth {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available(&self) -> bool {
        let auth = BioFingerprintAuth::new(Default::default());
        auth.is_available()
    }

    pub fn list_enrolled_fingers(&self, username: &str) -> Result<Vec<String>, String> {
        let auth = BioFingerprintAuth::new(Default::default());
        auth.list_enrolled_fingers(username)
    }

    pub fn enroll(
        &self,
        username: &str,
        finger_name: &str,
        app_handle: &AppHandle,
    ) -> Result<bool, String> {
        let auth = BioFingerprintAuth::new(Default::default());
        let callback = TauriEnrollCallback { app: app_handle };
        auth.enroll_with_callback(username, finger_name, Some(&callback))
    }

    pub fn remove_finger(&self, username: &str, finger_name: &str) -> Result<bool, String> {
        let auth = BioFingerprintAuth::new(Default::default());
        auth.remove_finger(username, finger_name)
    }
}

impl Default for FingerprintAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn auth_client_is_zero_sized() {
        assert_eq!(std::mem::size_of::<super::FingerprintAuth>(), 0);
    }
}
