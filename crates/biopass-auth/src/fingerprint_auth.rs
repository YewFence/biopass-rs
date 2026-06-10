use crate::{AuthConfig, AuthMethod, AuthResult, FingerprintMethodConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

const FPRINT_SERVICE: &str = "net.reactivated.Fprint";
const FPRINT_MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
const FPRINT_MANAGER_INTERFACE: &str = "net.reactivated.Fprint.Manager";
const FPRINT_DEVICE_INTERFACE: &str = "net.reactivated.Fprint.Device";
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const SIGNAL_LISTENER_SETUP_TIMEOUT: Duration = Duration::from_secs(2);

pub struct FingerprintAuth {
    config: FingerprintMethodConfig,
}

impl FingerprintAuth {
    pub fn new(config: FingerprintMethodConfig) -> Self {
        Self { config }
    }

    fn authenticate_fingerprint(
        &self,
        username: &str,
        cancel_signal: Option<&AtomicBool>,
    ) -> AuthResult {
        let Ok(device) = default_device() else {
            return AuthResult::Unavailable;
        };

        let Ok(enrolled_fingers) = device.list_enrolled_fingers(username) else {
            return AuthResult::Unavailable;
        };
        if enrolled_fingers.is_empty() {
            return AuthResult::Unavailable;
        }

        let Ok(_claim) = ClaimedDevice::claim(&device, username) else {
            return AuthResult::Unavailable;
        };

        let Ok(verify_results) = receive_verify_results(device.clone()) else {
            return AuthResult::Unavailable;
        };

        if device.call_unit("VerifyStart", &("any",)).is_err() {
            return AuthResult::Failure;
        }
        let _verification = VerificationSession { device: &device };

        wait_for_verify_result(&verify_results, self.config.timeout, cancel_signal)
    }
}

impl AuthMethod for FingerprintAuth {
    fn name(&self) -> &str {
        "fingerprint"
    }

    fn is_available(&self) -> bool {
        self.config.enable && default_device().is_ok()
    }

    fn retries(&self) -> u32 {
        self.config.retries
    }

    fn retry_delay_ms(&self) -> u32 {
        0
    }

    fn authenticate(
        &mut self,
        username: &str,
        _config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> AuthResult {
        self.authenticate_fingerprint(username, cancel_signal)
    }
}

#[derive(Clone)]
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

    fn list_enrolled_fingers(&self, username: &str) -> Result<Vec<String>, String> {
        self.call("ListEnrolledFingers", &(username,))
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

struct VerificationSession<'a> {
    device: &'a FprintDevice,
}

impl Drop for VerificationSession<'_> {
    fn drop(&mut self) {
        self.device.call_unit("VerifyStop", &()).ok();
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

fn receive_verify_results(device: FprintDevice) -> Result<mpsc::Receiver<AuthResult>, String> {
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let (result_sender, result_receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut verify_status = match device
            .proxy
            .receive_signal("VerifyStatus")
            .map_err(|error| format!("Failed to listen for fprintd VerifyStatus: {error}"))
        {
            Ok(verify_status) => {
                let _ = ready_sender.send(Ok(()));
                verify_status
            }
            Err(error) => {
                let _ = ready_sender.send(Err(error));
                return;
            }
        };

        for message in &mut verify_status {
            let status: Result<(String, bool), String> = message
                .body()
                .deserialize()
                .map_err(|error| format!("Failed to parse fingerprint verify status: {error}"));

            let result = match status {
                Ok((status, done)) => verify_status_to_result(&status, done),
                Err(_) => Some(AuthResult::Unavailable),
            };

            if let Some(result) = result {
                let _ = result_sender.send(result);
                return;
            }
        }

        let _ = result_sender.send(AuthResult::Unavailable);
    });

    match ready_receiver.recv_timeout(SIGNAL_LISTENER_SETUP_TIMEOUT) {
        Ok(Ok(())) => Ok(result_receiver),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(format!(
            "Timed out waiting for fprintd VerifyStatus listener: {error}"
        )),
    }
}

fn wait_for_verify_result(
    verify_results: &mpsc::Receiver<AuthResult>,
    timeout_ms: u32,
    cancel_signal: Option<&AtomicBool>,
) -> AuthResult {
    let deadline = (timeout_ms > 0)
        .then(|| Instant::now().checked_add(Duration::from_millis(timeout_ms.into())))
        .flatten();

    loop {
        if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
            return AuthResult::Failure;
        }

        let wait = match deadline {
            Some(deadline) => match deadline.checked_duration_since(Instant::now()) {
                Some(remaining) => remaining.min(CANCEL_POLL_INTERVAL),
                None => return AuthResult::Retry,
            },
            None => CANCEL_POLL_INTERVAL,
        };

        match verify_results.recv_timeout(wait) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return AuthResult::Retry;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return AuthResult::Unavailable,
        }
    }
}

fn verify_status_to_result(status: &str, done: bool) -> Option<AuthResult> {
    match status {
        "verify-match" => Some(AuthResult::Success),
        "verify-no-match" if done => Some(AuthResult::Failure),
        "verify-no-match" => None,
        "verify-unknown-error" | "verify-disconnected" => Some(AuthResult::Unavailable),
        _ if done => Some(AuthResult::Retry),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint_config() -> FingerprintMethodConfig {
        FingerprintMethodConfig {
            enable: true,
            retries: 3,
            timeout: 9000,
            fingers: Vec::new(),
        }
    }

    #[test]
    fn reports_fingerprint_method_metadata_from_config() {
        let method = FingerprintAuth::new(fingerprint_config());

        assert_eq!(method.name(), "fingerprint");
        assert_eq!(method.retries(), 3);
        assert_eq!(method.retry_delay_ms(), 0);
    }

    #[test]
    fn disabled_fingerprint_method_is_unavailable() {
        let mut config = fingerprint_config();
        config.enable = false;
        let method = FingerprintAuth::new(config);

        assert!(!method.is_available());
    }

    #[test]
    fn maps_terminal_verify_statuses() {
        assert_eq!(
            verify_status_to_result("verify-match", false),
            Some(AuthResult::Success)
        );
        assert_eq!(
            verify_status_to_result("verify-no-match", true),
            Some(AuthResult::Failure)
        );
        assert_eq!(
            verify_status_to_result("verify-unknown-error", false),
            Some(AuthResult::Unavailable)
        );
        assert_eq!(
            verify_status_to_result("verify-disconnected", false),
            Some(AuthResult::Unavailable)
        );
        assert_eq!(
            verify_status_to_result("verify-swipe-too-short", true),
            Some(AuthResult::Retry)
        );
    }

    #[test]
    fn ignores_nonterminal_verify_statuses() {
        assert_eq!(verify_status_to_result("verify-no-match", false), None);
        assert_eq!(
            verify_status_to_result("verify-swipe-too-short", false),
            None
        );
    }

    #[test]
    fn wait_for_verify_result_returns_received_result() {
        let (sender, receiver) = mpsc::channel();
        sender.send(AuthResult::Success).unwrap();

        assert_eq!(
            wait_for_verify_result(&receiver, 1000, None),
            AuthResult::Success
        );
    }

    #[test]
    fn wait_for_verify_result_times_out_as_retry() {
        let (_sender, receiver) = mpsc::channel();

        assert_eq!(
            wait_for_verify_result(&receiver, 1, None),
            AuthResult::Retry
        );
    }

    #[test]
    fn wait_for_verify_result_honors_cancellation() {
        let (_sender, receiver) = mpsc::channel();
        let cancel_signal = AtomicBool::new(true);

        assert_eq!(
            wait_for_verify_result(&receiver, 1000, Some(&cancel_signal)),
            AuthResult::Failure
        );
    }
}
