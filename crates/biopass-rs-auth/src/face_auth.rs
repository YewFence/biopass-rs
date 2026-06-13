use crate::{
    camera_available, capture_rgb_frame, decode_jpeg_rgb, emit_log, encode_jpeg, list_faces,
    user_data_dir, AuthConfig, AuthMethod, AuthResult, CameraRequest, FaceAntiSpoofing,
    FaceDetector, FaceMethodConfig, FaceRecognizer, FrameFormat, LogLevel, RgbFrame,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const IR_CAPTURE_WARMUP_FRAMES: u32 = 5;
const IR_CAPTURE_TIMEOUT_MS: u64 = 3000;

pub struct FaceAuth {
    config: FaceMethodConfig,
    session: FaceAuthSession,
}

#[derive(Default)]
struct FaceAuthSession {
    detector: Option<FaceDetector>,
    recognizer: Option<FaceRecognizer>,
    anti_spoofing: Option<FaceAntiSpoofing>,
    ir_anti_spoofing: Option<FaceAntiSpoofing>,
}

impl FaceAuth {
    pub fn new(config: FaceMethodConfig) -> Self {
        Self {
            config,
            session: FaceAuthSession::default(),
        }
    }

    fn clear_session(&mut self) {
        self.session = FaceAuthSession::default();
    }

    fn detector(&mut self) -> Result<&mut FaceDetector, String> {
        if self.session.detector.is_none() {
            self.session.detector = Some(FaceDetector::load_with_threshold(
                &self.config.detection.model,
                self.config.detection.threshold,
            )?);
        }

        Ok(self.session.detector.as_mut().unwrap())
    }

    fn recognizer(&mut self) -> Result<&mut FaceRecognizer, String> {
        if self.session.recognizer.is_none() {
            self.session.recognizer = Some(FaceRecognizer::load(
                &self.config.recognition.model,
                self.config.recognition.threshold,
            )?);
        }

        Ok(self.session.recognizer.as_mut().unwrap())
    }

    fn anti_spoofing(&mut self) -> Result<&mut FaceAntiSpoofing, String> {
        if self.session.anti_spoofing.is_none() {
            let model = &self.config.anti_spoofing.ai.model;
            self.session.anti_spoofing =
                Some(FaceAntiSpoofing::load(&model.path, model.threshold)?);
        }

        Ok(self.session.anti_spoofing.as_mut().unwrap())
    }

    fn ir_anti_spoofing(&mut self) -> Result<&mut FaceAntiSpoofing, String> {
        if self.session.ir_anti_spoofing.is_none() {
            let model = &self.config.anti_spoofing.ai.model;
            self.session.ir_anti_spoofing =
                Some(FaceAntiSpoofing::load(&model.path, model.threshold)?);
        }

        Ok(self.session.ir_anti_spoofing.as_mut().unwrap())
    }

    fn authenticate_face(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> Result<AuthResult, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| emit_log(level, debug, "FaceAuth", msg);

        log(
            LogLevel::Info,
            &format!("Starting authentication for user {username}"),
        );

        let enrolled = list_faces(username);
        if enrolled.is_empty() {
            log(LogLevel::Info, "no enrolled faces found");
            return Ok(AuthResult::Unavailable);
        }

        log(
            LogLevel::Debug,
            &format!("found {} enrolled face(s)", enrolled.len()),
        );

        if !Path::new(&self.config.detection.model).is_file()
            || !Path::new(&self.config.recognition.model).is_file()
        {
            log(LogLevel::Warn, "model files not found");
            return Ok(AuthResult::Unavailable);
        }

        if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
            log(LogLevel::Info, "authentication cancelled");
            return Ok(AuthResult::Failure);
        }

        log(LogLevel::Debug, "capturing frame from camera");
        let frame = capture_rgb_frame(&face_camera_request(
            self.config.camera.as_deref(),
            self.config.auto_optimize_camera,
            debug,
        ))?;
        log(
            LogLevel::Debug,
            &format!("frame captured: {}x{}", frame.width, frame.height),
        );

        log(
            LogLevel::Debug,
            &format!(
                "loading detection model from {}",
                self.config.detection.model
            ),
        );
        log(LogLevel::Debug, "running face detection");
        let candidate = match self
            .detector()
            .and_then(|detector| detector.crop_largest_face(&frame))
        {
            Ok(Some(candidate)) => candidate,
            Ok(None) => {
                log(LogLevel::Info, "no face detected in frame");
                save_debug_frame_if_enabled(debug, username, &frame, "no_face_detected");
                return Ok(AuthResult::Retry);
            }
            Err(error) => {
                save_debug_frame_if_enabled(debug, username, &frame, "detection_error");
                return Err(error);
            }
        };
        log(
            LogLevel::Debug,
            &format!(
                "face candidate cropped: {}x{}",
                candidate.width, candidate.height
            ),
        );

        log(LogLevel::Debug, "loading recognition model");
        let recognition_threshold = self.config.recognition.threshold;
        let candidate_embedding = {
            let recognizer = match self.recognizer() {
                Ok(recognizer) => recognizer,
                Err(error) => {
                    save_debug_frame_if_enabled(debug, username, &candidate, "recognition_error");
                    return Err(error);
                }
            };
            match recognizer.embedding(&candidate) {
                Ok(embedding) => embedding,
                Err(error) => {
                    save_debug_frame_if_enabled(debug, username, &candidate, "recognition_error");
                    return Err(error);
                }
            }
        };
        log(
            LogLevel::Debug,
            &format!("comparing against {} enrolled face(s)", enrolled.len()),
        );
        for enrolled_path in enrolled {
            if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                log(LogLevel::Info, "authentication cancelled during matching");
                return Ok(AuthResult::Failure);
            }

            let Ok(enrolled_face) = read_enrolled_face(&enrolled_path) else {
                log(
                    LogLevel::Warn,
                    &format!(
                        "skipping unreadable enrolled face {}",
                        enrolled_path.display()
                    ),
                );
                continue;
            };
            let face_match = {
                let recognizer = match self.recognizer() {
                    Ok(recognizer) => recognizer,
                    Err(error) => {
                        save_debug_frame_if_enabled(
                            debug,
                            username,
                            &candidate,
                            "recognition_error",
                        );
                        return Err(error);
                    }
                };
                let enrolled_embedding = match recognizer.embedding(&enrolled_face) {
                    Ok(embedding) => embedding,
                    Err(error) => {
                        save_debug_frame_if_enabled(
                            debug,
                            username,
                            &candidate,
                            "recognition_error",
                        );
                        return Err(error);
                    }
                };
                match recognizer.match_embeddings(&enrolled_embedding, &candidate_embedding) {
                    Ok(face_match) => face_match,
                    Err(error) => {
                        save_debug_frame_if_enabled(
                            debug,
                            username,
                            &candidate,
                            "recognition_error",
                        );
                        return Err(error);
                    }
                }
            };
            log(
                LogLevel::Debug,
                &format!(
                    "match against {}: similarity={:.4} threshold={:.4} similar={}",
                    enrolled_path.display(),
                    face_match.similarity,
                    recognition_threshold,
                    face_match.similar
                ),
            );
            if face_match.similar {
                log(LogLevel::Debug, "running anti-spoofing check");
                match self.check_anti_spoofing(username, auth_config, cancel_signal, &candidate) {
                    Ok(true) => {}
                    Ok(false) => {
                        log(LogLevel::Info, "anti-spoofing check rejected the candidate");
                        save_debug_frame_if_enabled(
                            debug,
                            username,
                            &candidate,
                            "antispoof_rejected",
                        );
                        return Ok(AuthResult::Failure);
                    }
                    Err(error) => {
                        save_debug_frame_if_enabled(debug, username, &candidate, "antispoof_error");
                        return Err(error);
                    }
                }

                log(LogLevel::Info, "face matched, authentication successful");
                return Ok(AuthResult::Success);
            }
        }

        log(LogLevel::Info, "no enrolled face matched, will retry");
        save_debug_frame_if_enabled(debug, username, &candidate, "not_similar");
        Ok(AuthResult::Retry)
    }

    fn check_anti_spoofing(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
        face: &RgbFrame,
    ) -> Result<bool, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| emit_log(level, debug, "FaceAntiSpoofing", msg);

        if !auth_config.antispoof {
            log(LogLevel::Info, "skipped (antispoof disabled at runtime)");
            return Ok(true);
        }

        let ai_enabled = self.config.anti_spoofing.ai.enable;
        let ir_enabled = self.config.anti_spoofing.ir.enable;
        if !ai_enabled && !ir_enabled {
            log(LogLevel::Info, "skipped (no ai or ir sub-check enabled)");
            return Ok(true);
        }

        log(
            LogLevel::Info,
            &format!("running checks (ai={ai_enabled}, ir={ir_enabled})"),
        );

        if ai_enabled {
            let model_path = self.config.anti_spoofing.ai.model.path.clone();
            let max_attempts = self.config.anti_spoofing.ai.retries.saturating_add(1);
            let retry_delay_ms = self.config.anti_spoofing.ai.retry_delay_ms;
            if model_path.is_empty() || !Path::new(&model_path).is_file() {
                log(
                    LogLevel::Warn,
                    "ai model not configured or missing on disk, treating as spoof",
                );
                save_debug_frame_if_enabled(debug, username, face, "ai_model_missing");
                return Ok(false);
            }

            let mut attempt = 0u32;
            let verdict = loop {
                attempt += 1;
                log(
                    LogLevel::Debug,
                    &format!("ai attempt {attempt}/{max_attempts}"),
                );
                let verdict = match self.anti_spoofing().and_then(|model| model.detect(face)) {
                    Ok(verdict) => verdict,
                    Err(error) => {
                        save_debug_frame_if_enabled(debug, username, face, "ai_error");
                        return Err(error);
                    }
                };
                log(
                    LogLevel::Debug,
                    &format!("ai model verdict: spoof={}", verdict.spoof),
                );
                if !verdict.spoof {
                    break verdict;
                }
                if attempt >= max_attempts {
                    break verdict;
                }
                if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                    log(LogLevel::Info, "ai check cancelled during retry");
                    return Ok(false);
                }
                if retry_delay_ms > 0 {
                    log(
                        LogLevel::Debug,
                        &format!("ai retry sleeping {retry_delay_ms}ms"),
                    );
                    std::thread::sleep(Duration::from_millis(retry_delay_ms as u64));
                }
            };
            if verdict.spoof {
                save_debug_frame_if_enabled(debug, username, face, "ai_spoof_detected");
                return Ok(false);
            }
        }

        if ir_enabled {
            log(LogLevel::Info, "running IR face liveness check");
            if !self.run_ir_check_with_retries(username, auth_config, cancel_signal)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn run_ir_check_with_retries(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> Result<bool, String> {
        let max_attempts = self.config.anti_spoofing.ir.retries.saturating_add(1);
        let retry_delay_ms = self.config.anti_spoofing.ir.retry_delay_ms;
        let debug = auth_config.debug;
        for attempt in 1..=max_attempts {
            if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                return Ok(false);
            }
            if attempt > 1 {
                emit_log(
                    LogLevel::Debug,
                    debug,
                    "FaceAntiSpoofingIr",
                    &format!("attempt {attempt}/{max_attempts}"),
                );
            }
            if self.check_ir_liveness(username, auth_config)? {
                return Ok(true);
            }
            if attempt < max_attempts {
                if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                    return Ok(false);
                }
                if retry_delay_ms > 0 {
                    emit_log(
                        LogLevel::Debug,
                        debug,
                        "FaceAntiSpoofingIr",
                        &format!("retry sleeping {retry_delay_ms}ms"),
                    );
                    std::thread::sleep(Duration::from_millis(retry_delay_ms as u64));
                }
            }
        }
        Ok(false)
    }

    fn check_ir_liveness(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
    ) -> Result<bool, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| emit_log(level, debug, "FaceAntiSpoofingIr", msg);

        let Some(camera) = self
            .config
            .anti_spoofing
            .ir
            .camera
            .as_deref()
            .filter(|camera| !camera.is_empty())
        else {
            log(
                LogLevel::Warn,
                "no IR camera configured, treating as missing",
            );
            return Ok(false);
        };

        if !Path::new(&self.config.detection.model).is_file() {
            log(
                LogLevel::Warn,
                "detection model missing, cannot run IR check",
            );
            return Ok(false);
        }

        let model_path = self.config.anti_spoofing.ai.model.path.clone();
        if model_path.is_empty() || !Path::new(&model_path).is_file() {
            log(
                LogLevel::Warn,
                "IR anti-spoofing model missing, cannot run liveness check",
            );
            return Ok(false);
        }

        if self.config.anti_spoofing.ir.warmup_delay_ms > 0 {
            log(
                LogLevel::Debug,
                &format!(
                    "sleeping {}ms for IR warmup",
                    self.config.anti_spoofing.ir.warmup_delay_ms
                ),
            );
            std::thread::sleep(Duration::from_millis(
                self.config.anti_spoofing.ir.warmup_delay_ms as u64,
            ));
        }

        log(
            LogLevel::Debug,
            &format!("capturing IR frame from {camera}"),
        );
        let frame = capture_rgb_frame(&ir_camera_request(camera, debug))?;
        log(
            LogLevel::Debug,
            &format!("IR frame captured: {}x{}", frame.width, frame.height),
        );

        let detections = match self.detector().and_then(|detector| detector.detect(&frame)) {
            Ok(detections) => detections,
            Err(error) => {
                save_debug_frame_if_enabled(debug, username, &frame, "ir_detection_error");
                return Err(error);
            }
        };
        log(
            LogLevel::Debug,
            &format!("IR detection found {} face(s)", detections.len()),
        );
        if detections.is_empty() {
            log(
                LogLevel::Info,
                "no face detected in IR frame, treating as spoof",
            );
            save_debug_frame_if_enabled(debug, username, &frame, "ir_no_face");
            return Ok(false);
        }

        let best_detection = detections
            .iter()
            .max_by(|a, b| a.confidence.total_cmp(&b.confidence))
            .expect("detections is non-empty");
        log(
            LogLevel::Debug,
            &format!(
                "selected IR face crop conf={:.4} bbox={}x{}",
                best_detection.confidence,
                best_detection.bbox.width(),
                best_detection.bbox.height()
            ),
        );

        let verdict = match self
            .ir_anti_spoofing()
            .and_then(|model| model.detect(&best_detection.crop))
        {
            Ok(verdict) => verdict,
            Err(error) => {
                save_debug_frame_if_enabled(
                    debug,
                    username,
                    &best_detection.crop,
                    "ir_classifier_error",
                );
                return Err(error);
            }
        };
        log(
            LogLevel::Debug,
            &format!("IR liveness verdict: spoof={}", verdict.spoof),
        );
        if verdict.spoof {
            save_debug_frame_if_enabled(debug, username, &best_detection.crop, "ir_spoof");
            return Ok(false);
        }

        Ok(true)
    }
}

fn ir_camera_request(camera: &str, debug: bool) -> CameraRequest {
    CameraRequest {
        device_path: Some(PathBuf::from(camera)),
        preferred_formats: vec![FrameFormat::Grey],
        warmup_frames: IR_CAPTURE_WARMUP_FRAMES,
        timeout: Duration::from_millis(IR_CAPTURE_TIMEOUT_MS),
        debug,
        ..CameraRequest::default()
    }
}

fn face_camera_request(
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

impl AuthMethod for FaceAuth {
    fn name(&self) -> &str {
        "face"
    }

    fn is_available(&self) -> bool {
        self.config.enable
            && camera_available(&face_camera_request(
                self.config.camera.as_deref(),
                self.config.auto_optimize_camera,
                false,
            ))
    }

    fn retries(&self) -> u32 {
        self.config.retries
    }

    fn retry_delay_ms(&self) -> u32 {
        self.config.retry_delay
    }

    fn begin_authentication_session(&mut self) {
        self.clear_session();
    }

    fn end_authentication_session(&mut self) {
        self.clear_session();
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
                emit_log(
                    LogLevel::Error,
                    config.debug,
                    "FaceAuth",
                    &format!("error during authentication for {username}: {error}"),
                );
                AuthResult::Retry
            }
        }
    }
}

fn save_debug_frame_if_enabled(debug: bool, username: &str, frame: &RgbFrame, reason: &str) {
    if !debug {
        return;
    }

    match save_debug_frame(username, frame, reason) {
        Ok(path) => emit_log(
            LogLevel::Debug,
            debug,
            "FaceAuth",
            &format!("saved debug frame to {}", path.display()),
        ),
        Err(error) => emit_log(
            LogLevel::Warn,
            debug,
            "FaceAuth",
            &format!("failed to save debug frame: {error}"),
        ),
    }
}

fn save_debug_frame(username: &str, frame: &RgbFrame, reason: &str) -> Result<PathBuf, String> {
    use std::time::SystemTime;

    let timestamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("{}.{}.jpg", reason, timestamp);
    let debug_dir = user_data_dir(username).join("debugs");
    let path = debug_dir.join(filename);

    std::fs::create_dir_all(&debug_dir).map_err(|error| {
        format!(
            "Failed to create debug directory {}: {error}",
            debug_dir.display()
        )
    })?;
    let jpeg = encode_jpeg(frame, 85)?;
    std::fs::write(&path, jpeg)
        .map_err(|error| format!("Failed to write debug frame {}: {error}", path.display()))?;
    Ok(path)
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
        config.camera = Some("/dev/biopass-rs-missing-camera".to_string());
        let method = FaceAuth::new(config);

        assert!(!method.is_available());
    }

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
    fn save_debug_frame_creates_debug_directory() {
        let home = tempfile::tempdir().unwrap();
        let previous_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());
        let frame = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();

        let path = save_debug_frame("biopass-rs-missing-user", &frame, "test_failure").unwrap();

        assert!(path.is_file());
        assert!(path
            .parent()
            .is_some_and(|parent| parent.ends_with(".local/share/biopass-rs/debugs")));
        let data = std::fs::read(path).unwrap();
        assert!(data.starts_with(&[0xff, 0xd8]));

        if let Some(home) = previous_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn ir_camera_request_requires_grey_frames() {
        let request = ir_camera_request("/dev/video2", false);

        assert_eq!(request.device_path, Some(PathBuf::from("/dev/video2")));
        assert_eq!(request.preferred_formats, vec![FrameFormat::Grey]);
        assert_eq!(request.warmup_frames, IR_CAPTURE_WARMUP_FRAMES);
        assert_eq!(
            request.timeout,
            Duration::from_millis(IR_CAPTURE_TIMEOUT_MS)
        );
    }
}
