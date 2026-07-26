use crate::{CameraRequest, FrameFormat};
use std::path::PathBuf;
use std::time::Duration;

pub(super) const IR_CAPTURE_WARMUP_FRAMES: u32 = 5;
pub(super) const IR_CAPTURE_TIMEOUT_MS: u64 = 3000;
pub(super) const IR_LIVENESS_FRAME_COUNT: usize = 3;
pub(super) const IR_LIVENESS_REQUIRED_PASSES: usize = 2;
pub(super) const IR_LIVENESS_FRAME_INTERVAL_MS: u64 = 80;

pub(super) fn ir_camera_request(
    camera: &str,
    auto_optimize_camera: bool,
    debug: bool,
) -> CameraRequest {
    CameraRequest {
        device_path: Some(PathBuf::from(camera)),
        preferred_formats: vec![FrameFormat::Grey],
        warmup_frames: IR_CAPTURE_WARMUP_FRAMES,
        timeout: Duration::from_millis(IR_CAPTURE_TIMEOUT_MS),
        auto_optimize_camera,
        debug,
        ..CameraRequest::default()
    }
}

pub(super) fn face_camera_request(
    camera: Option<&str>,
    auto_optimize_camera: bool,
    debug: bool,
) -> CameraRequest {
    CameraRequest {
        device_path: camera
            .filter(|camera| !camera.is_empty())
            .map(PathBuf::from),
        auto_optimize_camera,
        debug,
        ..CameraRequest::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_camera_request_uses_configured_camera() {
        let request = face_camera_request(Some("/dev/video4"), true, false);

        assert_eq!(request.device_path, Some(PathBuf::from("/dev/video4")));
        assert!(request.preferred_formats.contains(&FrameFormat::Yuyv));
        assert!(request.preferred_formats.contains(&FrameFormat::Grey));
        assert!(request.auto_optimize_camera);
    }

    #[test]
    fn face_camera_request_disables_auto_optimize() {
        let request = face_camera_request(None, false, false);

        assert!(!request.auto_optimize_camera);
    }

    #[test]
    fn ir_camera_request_requires_grey_frames() {
        let request = ir_camera_request("/dev/video2", false, false);

        assert_eq!(request.device_path, Some(PathBuf::from("/dev/video2")));
        assert_eq!(request.preferred_formats, vec![FrameFormat::Grey]);
        assert_eq!(request.warmup_frames, IR_CAPTURE_WARMUP_FRAMES);
        assert_eq!(
            request.timeout,
            Duration::from_millis(IR_CAPTURE_TIMEOUT_MS)
        );
        assert!(!request.auto_optimize_camera);
    }

    #[test]
    fn ir_camera_request_can_enable_auto_optimize() {
        let request = ir_camera_request("/dev/video2", true, false);

        assert!(request.auto_optimize_camera);
    }
}
