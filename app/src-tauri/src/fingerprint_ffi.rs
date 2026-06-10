use tauri::{AppHandle, Emitter};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

const FPRINT_SERVICE: &str = "net.reactivated.Fprint";
const FPRINT_MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
const FPRINT_MANAGER_INTERFACE: &str = "net.reactivated.Fprint.Manager";
const FPRINT_DEVICE_INTERFACE: &str = "net.reactivated.Fprint.Device";

/// Rust fprintd client kept behind the old module name so Tauri commands stay compatible.
pub struct FingerprintAuth;

impl FingerprintAuth {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available(&self) -> bool {
        default_device().is_ok()
    }

    pub fn list_enrolled_fingers(&self, username: &str) -> Result<Vec<String>, String> {
        let device = default_device()?;
        device.call("ListEnrolledFingers", &(username,))
    }

    pub fn enroll(
        &self,
        username: &str,
        finger_name: &str,
        app_handle: &AppHandle,
    ) -> Result<bool, String> {
        let device = default_device()?;
        let mut enroll_status = device
            .proxy
            .receive_signal("EnrollStatus")
            .map_err(|error| format!("Failed to listen for fprintd EnrollStatus: {error}"))?;
        let _claim = ClaimedDevice::claim(&device, username)?;

        device.call_unit("EnrollStart", &(finger_name,))?;

        let mut completed = false;
        for message in &mut enroll_status {
            let (status, done): (String, bool) = message
                .body()
                .deserialize()
                .map_err(|error| format!("Failed to parse enrollment status: {error}"))?;

            emit_enrollment_status(app_handle, done, status.clone());

            if done {
                completed = status == "enroll-completed";
                break;
            }
        }

        device.call_unit("EnrollStop", &()).ok();
        Ok(completed)
    }

    pub fn remove_finger(&self, username: &str, finger_name: &str) -> Result<bool, String> {
        let device = default_device()?;
        let _claim = ClaimedDevice::claim(&device, username)?;
        device.call_unit("DeleteEnrolledFinger", &(username, finger_name))?;
        Ok(true)
    }
}

impl Default for FingerprintAuth {
    fn default() -> Self {
        Self::new()
    }
}

struct FprintDevice {
    proxy: Proxy<'static>,
}

impl FprintDevice {
    fn call<B, R>(&self, method_name: &str, body: &B) -> Result<R, String>
    where
        B: serde::ser::Serialize + zbus::zvariant::DynamicType,
        R: for<'de> zbus::zvariant::DynamicDeserialize<'de>,
    {
        self.proxy
            .call(method_name, body)
            .map_err(|error| format!("fprintd {method_name} failed: {error}"))
    }

    fn call_unit<B>(&self, method_name: &str, body: &B) -> Result<(), String>
    where
        B: serde::ser::Serialize + zbus::zvariant::DynamicType,
    {
        self.call::<B, ()>(method_name, body)
    }
}

struct ClaimedDevice<'a> {
    device: &'a FprintDevice,
}

impl<'a> ClaimedDevice<'a> {
    fn claim(device: &'a FprintDevice, username: &str) -> Result<Self, String> {
        device.call_unit("Claim", &(username,))?;
        Ok(Self { device })
    }
}

impl Drop for ClaimedDevice<'_> {
    fn drop(&mut self) {
        self.device.call_unit("Release", &()).ok();
    }
}

fn default_device() -> Result<FprintDevice, String> {
    let connection = Connection::system()
        .map_err(|error| format!("Failed to connect to system bus: {error}"))?;
    let manager = Proxy::new(
        &connection,
        FPRINT_SERVICE,
        FPRINT_MANAGER_PATH,
        FPRINT_MANAGER_INTERFACE,
    )
    .map_err(|error| format!("Failed to create fprintd manager proxy: {error}"))?;

    let device_path: OwnedObjectPath = manager
        .call("GetDefaultDevice", &())
        .map_err(|error| format!("No fingerprint device found: {error}"))?;

    let proxy = Proxy::new(
        &connection,
        FPRINT_SERVICE,
        device_path,
        FPRINT_DEVICE_INTERFACE,
    )
    .map_err(|error| format!("Failed to create fprintd device proxy: {error}"))?;

    Ok(FprintDevice { proxy })
}

fn emit_enrollment_status(app_handle: &AppHandle, done: bool, status: String) {
    #[derive(serde::Serialize, Clone)]
    struct ProgressPayload {
        done: bool,
        status: String,
    }

    app_handle
        .emit(
            "fingerprint-enroll-status",
            ProgressPayload { done, status },
        )
        .ok();
}

#[cfg(test)]
mod tests {
    #[test]
    fn auth_client_is_zero_sized() {
        assert_eq!(std::mem::size_of::<super::FingerprintAuth>(), 0);
    }
}
