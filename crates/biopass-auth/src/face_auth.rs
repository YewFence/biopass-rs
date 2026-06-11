use crate::{
    camera_available, capture_rgb_frame, decode_jpeg_rgb, list_faces, AuthConfig, AuthMethod,
    AuthResult, CameraRequest, FaceAntiSpoofing, FaceDetector, FaceMethodConfig, FaceRecognizer,
    FrameFormat, RgbFrame,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const IR_CAPTURE_WARMUP_FRAMES: u32 = 5;
const IR_CAPTURE_TIMEOUT_MS: u64 = 3000;

pub struct FaceAuth {
    config: FaceMethodConfig,
}

impl FaceAuth {
    pub fn new(config: FaceMethodConfig) -> Self {
        Self { config }
    }

    fn authenticate_face(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> Result<AuthResult, String> {
        let log = |msg: &str| {
            if auth_config.debug {
                eprintln!("FaceAuth: {msg}");
            }
        };

        log(&format!("Starting authentication for user {username}"));

        let enrolled = list_faces(username);
        if enrolled.is_empty() {
            log("no enrolled faces found");
            return Ok(AuthResult::Unavailable);
        }

        log(&format!("found {} enrolled face(s)", enrolled.len()));

        if !Path::new(&self.config.detection.model).is_file()
            || !Path::new(&self.config.recognition.model).is_file()
        {
            log("model files not found");
            return Ok(AuthResult::Unavailable);
        }

        if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
            log("authentication cancelled");
            return Ok(AuthResult::Failure);
        }

        log("capturing frame from camera");
        let frame = capture_rgb_frame(&face_camera_request(
            self.config.camera.as_deref(),
            self.config.auto_optimize_camera,
        ))?;
        log(&format!("frame captured: {}x{}", frame.width, frame.height));

        log(&format!(
            "loading detection model from {}",
            self.config.detection.model
        ));
        let mut detector = FaceDetector::load_with_threshold(
            &self.config.detection.model,
            self.config.detection.threshold,
        )?;

        log("running face detection");
        let Some(candidate) = detector.crop_largest_face(&frame)? else {
            log("no face detected in frame");
            return Ok(AuthResult::Retry);
        };
        log(&format!(
            "face candidate cropped: {}x{}",
            candidate.width, candidate.height
        ));

        log("running anti-spoofing check");
        if !self.check_anti_spoofing(username, auth_config, &candidate)? {
            log("anti-spoofing check rejected the candidate");
            return Ok(AuthResult::Failure);
        }

        log("loading recognition model");
        let mut recognizer = FaceRecognizer::load(
            &self.config.recognition.model,
            self.config.recognition.threshold,
        )?;
        log(&format!(
            "comparing against {} enrolled face(s)",
            enrolled.len()
        ));
        for enrolled_path in enrolled {
            if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                log("authentication cancelled during matching");
                return Ok(AuthResult::Failure);
            }

            let Ok(enrolled_face) = read_enrolled_face(&enrolled_path) else {
                log(&format!(
                    "skipping unreadable enrolled face {}",
                    enrolled_path.display()
                ));
                continue;
            };
            let face_match = recognizer.match_faces(&enrolled_face, &candidate)?;
            log(&format!(
                "match against {}: similarity={:.4} threshold={:.4} similar={}",
                enrolled_path.display(),
                face_match.similarity,
                self.config.recognition.threshold,
                face_match.similar
            ));
            if face_match.similar {
                log("face matched, authentication successful");
                return Ok(AuthResult::Success);
            }
        }

        log("no enrolled face matched, will retry");
        Ok(AuthResult::Retry)
    }

    fn check_anti_spoofing(
        &self,
        _username: &str,
        auth_config: &AuthConfig,
        face: &RgbFrame,
    ) -> Result<bool, String> {
        let log = |msg: &str| {
            if auth_config.debug {
                eprintln!("FaceAuth: anti-spoofing: {msg}");
            }
        };

        if !auth_config.antispoof {
            log("skipped (antispoof disabled at runtime)");
            return Ok(true);
        }

        let ai_enabled = self.config.anti_spoofing.ai.enable;
        let ir_enabled = self.config.anti_spoofing.ir.enable;
        if !ai_enabled && !ir_enabled {
            log("skipped (no ai or ir sub-check enabled)");
            return Ok(true);
        }

        log(&format!(
            "running checks (ai={ai_enabled}, ir={ir_enabled})"
        ));

        if ai_enabled {
            let model = &self.config.anti_spoofing.ai.model;
            if model.path.is_empty() || !Path::new(&model.path).is_file() {
                log("ai model not configured or missing on disk, treating as spoof");
                return Ok(false);
            }

            let mut anti_spoofing = FaceAntiSpoofing::load(&model.path, model.threshold)?;
            let verdict = anti_spoofing.detect(face)?;
            log(&format!("ai model verdict: spoof={}", verdict.spoof));
            if verdict.spoof {
                return Ok(false);
            }
        }

        if ir_enabled {
            log("running IR face presence check");
            let ir_result = self.check_ir_face_presence(auth_config)?;
            log(&format!("IR face presence verdict: {ir_result}"));
            if !ir_result {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn check_ir_face_presence(&self, auth_config: &AuthConfig) -> Result<bool, String> {
        let log = |msg: &str| {
            if auth_config.debug {
                eprintln!("FaceAuth: ir: {msg}");
            }
        };

        let Some(camera) = self
            .config
            .anti_spoofing
            .ir
            .camera
            .as_deref()
            .filter(|camera| !camera.is_empty())
        else {
            log("no IR camera configured, treating as missing");
            return Ok(false);
        };

        if !Path::new(&self.config.detection.model).is_file() {
            log("detection model missing, cannot run IR check");
            return Ok(false);
        }

        if self.config.anti_spoofing.ir.warmup_delay_ms > 0 {
            log(&format!(
                "sleeping {}ms for IR warmup",
                self.config.anti_spoofing.ir.warmup_delay_ms
            ));
            std::thread::sleep(Duration::from_millis(
                self.config.anti_spoofing.ir.warmup_delay_ms as u64,
            ));
        }

        log(&format!("capturing IR frame from {camera}"));
        let frame = capture_rgb_frame(&ir_camera_request(camera))?;
        log(&format!(
            "IR frame captured: {}x{}",
            frame.width, frame.height
        ));

        let mut detector = FaceDetector::load_with_threshold(
            &self.config.detection.model,
            self.config.detection.threshold,
        )?;
        let detections = detector.detect(&frame)?;
        log(&format!("IR detection found {} face(s)", detections.len()));
        Ok(!detections.is_empty())
    }
}

fn ir_camera_request(camera: &str) -> CameraRequest {
    CameraRequest {
        device_path: Some(PathBuf::from(camera)),
        preferred_formats: vec![FrameFormat::Grey],
        warmup_frames: IR_CAPTURE_WARMUP_FRAMES,
        timeout: Duration::from_millis(IR_CAPTURE_TIMEOUT_MS),
        ..CameraRequest::default()
    }
}

fn face_camera_request(camera: Option<&str>, auto_optimize_camera: bool) -> CameraRequest {
    CameraRequest {
        device_path: camera
            .filter(|camera| !camera.is_empty())
            .map(PathBuf::from),
        auto_optimize_camera,
        ..CameraRequest::default()
    }
}

impl AuthMethod for FaceAuth {
    fn name(&self) -> &str {
        "face"
    }

    fn is_available(&self) -> bool {
        self.config.enable
            && camera_available(&face_camera_request(
                self.config.camera.as_deref(),
                self.config.auto_optimize_camera,
            ))
    }

    fn retries(&self) -> u32 {
        self.config.retries
    }

    fn retry_delay_ms(&self) -> u32 {
        self.config.retry_delay
    }

    fn authenticate(
        &mut self,
        username: &str,
        config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> AuthResult {
        match self.authenticate_face(username, config, cancel_signal) {
            Ok(result) => result,
            Err(error) => {
                if config.debug {
                    eprintln!("FaceAuth: error during authentication: {error}");
                }
                AuthResult::Retry
            }
        }
    }
}

fn read_enrolled_face(path: &Path) -> Result<RgbFrame, String> {
    let data = std::fs::read(path)
        .map_err(|error| format!("Failed to read enrolled face {}: {error}", path.display()))?;
    decode_jpeg_rgb(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face_config() -> FaceMethodConfig {
        FaceMethodConfig {
            detection: crate::DetectionConfig {
                model: "/tmp/detection.onnx".to_string(),
                threshold: 0.4,
            },
            recognition: crate::RecognitionConfig {
                model: "/tmp/recognition.onnx".to_string(),
                threshold: 0.6,
            },
            ..FaceMethodConfig::default()
        }
    }

    #[test]
    fn reports_face_method_metadata_from_config() {
        let mut config = face_config();
        config.retries = 3;
        config.retry_delay = 25;
        let method = FaceAuth::new(config);

        assert_eq!(method.name(), "face");
        assert_eq!(method.retries(), 3);
        assert_eq!(method.retry_delay_ms(), 25);
    }

    #[test]
    fn disabled_face_method_is_unavailable() {
        let mut config = face_config();
        config.enable = false;
        let method = FaceAuth::new(config);

        assert!(!method.is_available());
    }

    #[test]
    fn missing_configured_face_camera_is_unavailable() {
        let mut config = face_config();
        config.camera = Some("/dev/biopass-missing-camera".to_string());
        let method = FaceAuth::new(config);

        assert!(!method.is_available());
    }

    #[test]
    fn face_camera_request_uses_configured_camera() {
        let request = face_camera_request(Some("/dev/video4"), true);

        assert_eq!(request.device_path, Some(PathBuf::from("/dev/video4")));
        assert!(request.preferred_formats.contains(&FrameFormat::Yuyv));
        assert!(request.preferred_formats.contains(&FrameFormat::Grey));
        assert!(request.auto_optimize_camera);
    }

    #[test]
    fn face_camera_request_disables_auto_optimize() {
        let request = face_camera_request(None, false);

        assert!(request.auto_optimize_camera == false);
    }

    #[test]
    fn reads_enrolled_jpeg_faces() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("face.jpg");
        let frame = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();
        std::fs::write(&path, crate::encode_jpeg(&frame, 95).unwrap()).unwrap();

        let loaded = read_enrolled_face(&path).unwrap();

        assert_eq!(loaded.width, 1);
        assert_eq!(loaded.height, 1);
        assert_eq!(loaded.data.len(), 3);
    }

    #[test]
    fn ir_camera_request_requires_grey_frames() {
        let request = ir_camera_request("/dev/video2");

        assert_eq!(request.device_path, Some(PathBuf::from("/dev/video2")));
        assert_eq!(request.preferred_formats, vec![FrameFormat::Grey]);
        assert_eq!(request.warmup_frames, IR_CAPTURE_WARMUP_FRAMES);
        assert_eq!(
            request.timeout,
            Duration::from_millis(IR_CAPTURE_TIMEOUT_MS)
        );
    }
}
