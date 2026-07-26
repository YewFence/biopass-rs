mod camera;
mod liveness;
mod outcome;
mod runtime;
mod session;
mod storage;

use outcome::FaceAuthAttemptOutcome;
use runtime::FaceAuthRuntime;
use session::DefaultFaceAuthRuntime;
use storage::{read_enrolled_face, save_debug_frame_if_enabled};

use crate::{
    emit_log, list_faces, AuthConfig, AuthMethod, AuthResult, FaceBestMatchSummary,
    FaceMethodConfig, LogComponent, LogLevel, MethodAuthOutcome,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
static DATA_DIR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct FaceAuth {
    config: FaceMethodConfig,
    runtime: Box<dyn FaceAuthRuntime>,
}

impl FaceAuth {
    pub fn new(config: FaceMethodConfig) -> Self {
        Self {
            config,
            runtime: Box::<DefaultFaceAuthRuntime>::default(),
        }
    }

    #[cfg(test)]
    fn with_runtime(config: FaceMethodConfig, runtime: Box<dyn FaceAuthRuntime>) -> Self {
        Self { config, runtime }
    }

    fn authenticate_face(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> Result<FaceAuthAttemptOutcome, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| emit_log(LogComponent::Auth, level, "FaceAuth", msg);

        log(
            LogLevel::Info,
            &format!("Starting authentication for user {username}"),
        );

        let enrolled = list_faces(username);
        if enrolled.is_empty() {
            log(LogLevel::Info, "no enrolled faces found");
            return Ok(FaceAuthAttemptOutcome::new(
                AuthResult::Unavailable,
                "no_enrolled_faces",
                "no enrolled faces found",
            ));
        }

        log(
            LogLevel::Debug,
            &format!("found {} enrolled face(s)", enrolled.len()),
        );

        if !Path::new(&self.config.detection.model).is_file()
            || !Path::new(&self.config.recognition.model).is_file()
        {
            log(LogLevel::Warn, "model files not found");
            return Ok(FaceAuthAttemptOutcome::new(
                AuthResult::Unavailable,
                "model_missing",
                "face model files were not found",
            ));
        }

        if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
            log(LogLevel::Info, "authentication cancelled");
            return Ok(FaceAuthAttemptOutcome::new(
                AuthResult::Failure,
                "cancelled",
                "face authentication was cancelled",
            ));
        }

        log(LogLevel::Debug, "capturing frame from camera");
        let frame = self.runtime.capture_frame(&self.config, debug)?;
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
        let (rgb_frame, rgb_face) = match self.runtime.detect_faces(&self.config, &frame) {
            Ok(detections) if !detections.is_empty() => {
                let best = detections
                    .into_iter()
                    .max_by(|a, b| a.bbox.area().cmp(&b.bbox.area()))
                    .expect("at least one detection");
                (frame.clone(), best)
            }
            Ok(_) => {
                log(LogLevel::Info, "no face detected in frame");
                save_debug_frame_if_enabled(debug, username, &frame, "no_face_detected");
                return Ok(FaceAuthAttemptOutcome::new(
                    AuthResult::Retry,
                    "no_face_detected",
                    "no face was detected in the captured frame",
                ));
            }
            Err(error) => {
                save_debug_frame_if_enabled(debug, username, &frame, "detection_error");
                return Err(error);
            }
        };
        let candidate = rgb_face.crop.clone();
        log(
            LogLevel::Debug,
            &format!(
                "face candidate cropped: {}x{} at bbox={}x{}@({},{})",
                candidate.width,
                candidate.height,
                rgb_face.bbox.width(),
                rgb_face.bbox.height(),
                rgb_face.bbox.x1,
                rgb_face.bbox.y1,
            ),
        );

        log(LogLevel::Debug, "loading recognition model");
        let recognition_threshold = self.config.recognition.threshold;
        let candidate_embedding = match self.runtime.embedding(&self.config, &candidate) {
            Ok(embedding) => embedding,
            Err(error) => {
                save_debug_frame_if_enabled(debug, username, &candidate, "recognition_error");
                return Err(error);
            }
        };
        log(
            LogLevel::Debug,
            &format!("comparing against {} enrolled face(s)", enrolled.len()),
        );
        let mut best_match: Option<FaceBestMatchSummary> = None;
        for (enrolled_index, enrolled_path) in enrolled.into_iter().enumerate() {
            if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                log(LogLevel::Info, "authentication cancelled during matching");
                return Ok(FaceAuthAttemptOutcome::new(
                    AuthResult::Failure,
                    "cancelled",
                    "face authentication was cancelled during matching",
                ));
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
                let enrolled_embedding = match self.runtime.embedding(&self.config, &enrolled_face)
                {
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
                match self.runtime.match_embeddings(
                    &self.config,
                    &enrolled_embedding,
                    &candidate_embedding,
                ) {
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
            let current_match = FaceBestMatchSummary {
                enrolled_index: enrolled_index + 1,
                similarity: face_match.similarity,
                threshold: recognition_threshold,
                similar: face_match.similar,
            };
            if best_match
                .map(|best| face_match.similarity > best.similarity)
                .unwrap_or(true)
            {
                best_match = Some(current_match);
            }
            if face_match.similar {
                log(LogLevel::Debug, "running anti-spoofing check");
                match self.check_anti_spoofing(
                    username,
                    auth_config,
                    cancel_signal,
                    &rgb_frame,
                    rgb_face.bbox,
                    &candidate,
                ) {
                    Ok(outcome) if outcome.passed => {}
                    Ok(outcome) => {
                        log(LogLevel::Info, "anti-spoofing check rejected the candidate");
                        save_debug_frame_if_enabled(
                            debug,
                            username,
                            &candidate,
                            "antispoof_rejected",
                        );
                        return Ok(FaceAuthAttemptOutcome {
                            result: AuthResult::Failure,
                            reason: outcome
                                .reason
                                .or_else(|| Some("anti_spoofing_rejected".to_string())),
                            message: outcome.message.unwrap_or_else(|| {
                                "anti-spoofing rejected the matched face candidate".to_string()
                            }),
                            best_match: Some(current_match),
                            ir: outcome.ir,
                        });
                    }
                    Err(error) => {
                        save_debug_frame_if_enabled(debug, username, &candidate, "antispoof_error");
                        return Err(error);
                    }
                }

                log(LogLevel::Info, "face matched, authentication successful");
                return Ok(FaceAuthAttemptOutcome {
                    result: AuthResult::Success,
                    reason: Some("face_matched".to_string()),
                    message: "face authentication succeeded".to_string(),
                    best_match: Some(current_match),
                    ir: None,
                });
            }
        }

        log(LogLevel::Info, "no enrolled face matched, will retry");
        save_debug_frame_if_enabled(debug, username, &candidate, "not_similar");
        let message = best_match
            .map(|best| {
                format!(
                    "no enrolled face matched, best match was enrolled face #{} with similarity {:.4}, threshold {:.4}",
                    best.enrolled_index, best.similarity, best.threshold
                )
            })
            .unwrap_or_else(|| "no enrolled face could be compared".to_string());
        Ok(FaceAuthAttemptOutcome {
            result: AuthResult::Retry,
            reason: Some("no_enrolled_face_matched".to_string()),
            message,
            best_match,
            ir: None,
        })
    }
}

impl AuthMethod for FaceAuth {
    fn name(&self) -> &str {
        "face"
    }

    fn is_available(&self) -> bool {
        self.runtime.is_available(&self.config)
    }

    fn retries(&self) -> u32 {
        self.config.retries
    }

    fn retry_delay_ms(&self) -> u32 {
        self.config.retry_delay
    }

    fn begin_authentication_session(&mut self) {
        self.runtime.clear();
    }

    fn end_authentication_session(&mut self) {
        self.runtime.clear();
    }

    fn authenticate(
        &mut self,
        username: &str,
        config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> MethodAuthOutcome {
        match self.authenticate_face(username, config, cancel_signal) {
            Ok(outcome) => outcome.into_method_outcome(),
            Err(error) => {
                emit_log(
                    LogComponent::Auth,
                    LogLevel::Error,
                    "FaceAuth",
                    &format!("error during authentication for {username}: {error}"),
                );
                MethodAuthOutcome::basic(
                    "face",
                    AuthResult::Retry,
                    format!("face authentication error: {error}"),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthMethod, FaceBox, FaceDetection, FaceMatch, IrLivenessSummary, RgbFrame, SpoofResult,
    };
    use std::collections::VecDeque;
    use std::path::Path;

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

    #[derive(Default)]
    struct FakeRuntime {
        available: bool,
        frame: Option<RgbFrame>,
        detections: Vec<FaceDetection>,
        embeddings: VecDeque<Vec<f32>>,
        matches: VecDeque<FaceMatch>,
        rgb_spoof: Option<SpoofResult>,
        ir: Option<IrLivenessSummary>,
    }

    impl FaceAuthRuntime for FakeRuntime {
        fn clear(&mut self) {}

        fn is_available(&self, _config: &FaceMethodConfig) -> bool {
            self.available
        }

        fn capture_frame(
            &mut self,
            _config: &FaceMethodConfig,
            _debug: bool,
        ) -> Result<RgbFrame, String> {
            self.frame
                .clone()
                .ok_or_else(|| "no fake frame".to_string())
        }

        fn detect_faces(
            &mut self,
            _config: &FaceMethodConfig,
            _frame: &RgbFrame,
        ) -> Result<Vec<FaceDetection>, String> {
            Ok(self.detections.clone())
        }

        fn embedding(
            &mut self,
            _config: &FaceMethodConfig,
            _frame: &RgbFrame,
        ) -> Result<Vec<f32>, String> {
            self.embeddings
                .pop_front()
                .ok_or_else(|| "no fake embedding".to_string())
        }

        fn match_embeddings(
            &mut self,
            _config: &FaceMethodConfig,
            _enrolled_embedding: &[f32],
            _candidate_embedding: &[f32],
        ) -> Result<FaceMatch, String> {
            self.matches
                .pop_front()
                .ok_or_else(|| "no fake match".to_string())
        }

        fn detect_rgb_spoof(
            &mut self,
            _config: &FaceMethodConfig,
            _face: &RgbFrame,
        ) -> Result<SpoofResult, String> {
            Ok(self.rgb_spoof.clone().unwrap_or(SpoofResult {
                score: 0.1,
                spoof: false,
            }))
        }

        fn run_ir_liveness(
            &mut self,
            _config: &FaceMethodConfig,
            _username: &str,
            _auth_config: &AuthConfig,
            _cancel_signal: Option<&AtomicBool>,
            _rgb_frame: &RgbFrame,
            _rgb_face_box: FaceBox,
        ) -> Result<IrLivenessSummary, String> {
            Ok(self.ir.clone().unwrap_or(IrLivenessSummary {
                frames: 0,
                passed_frames: 0,
                required_passes: 0,
                last_failure: None,
                highest_detection_confidence: None,
            }))
        }
    }

    fn write_model_files(directory: &Path, config: &mut FaceMethodConfig) {
        let detection = directory.join("detection.onnx");
        let recognition = directory.join("recognition.onnx");
        std::fs::write(&detection, b"model").unwrap();
        std::fs::write(&recognition, b"model").unwrap();
        config.detection.model = detection.to_string_lossy().to_string();
        config.recognition.model = recognition.to_string_lossy().to_string();
    }

    fn with_data_dir_override<T>(
        path: &std::path::Path,
        f: impl FnOnce() -> T + std::panic::UnwindSafe,
    ) -> T {
        let _guard = super::DATA_DIR_TEST_LOCK.lock().unwrap();
        let previous = std::env::var_os(crate::DATA_DIR_ENV);
        std::env::set_var(crate::DATA_DIR_ENV, path);
        let result = std::panic::catch_unwind(f);

        if let Some(value) = previous {
            std::env::set_var(crate::DATA_DIR_ENV, value);
        } else {
            std::env::remove_var(crate::DATA_DIR_ENV);
        }

        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    fn enrolled_face(data_dir: &Path) {
        let faces = data_dir.join("faces");
        std::fs::create_dir_all(&faces).unwrap();
        let frame = RgbFrame::new(1, 1, vec![64, 128, 255]).unwrap();
        std::fs::write(
            faces.join("face.jpg"),
            crate::encode_jpeg(&frame, 95).unwrap(),
        )
        .unwrap();
    }

    fn detection() -> FaceDetection {
        let crop = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();
        FaceDetection {
            confidence: 0.9,
            bbox: FaceBox {
                x1: 0,
                y1: 0,
                x2: 1,
                y2: 1,
            },
            crop,
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
    fn authenticate_succeeds_with_matching_face_and_fake_runtime() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = face_config();
        config.anti_spoofing.rgb.enable = false;
        config.anti_spoofing.ir.enable = false;
        write_model_files(directory.path(), &mut config);
        enrolled_face(directory.path());
        let runtime = FakeRuntime {
            available: true,
            frame: Some(RgbFrame::new(2, 2, vec![0; 12]).unwrap()),
            detections: vec![detection()],
            embeddings: VecDeque::from([vec![1.0, 0.0], vec![1.0, 0.0]]),
            matches: VecDeque::from([FaceMatch {
                similarity: 0.95,
                similar: true,
            }]),
            ..FakeRuntime::default()
        };

        with_data_dir_override(directory.path(), || {
            let mut method = FaceAuth::with_runtime(config, Box::new(runtime));
            let outcome = method.authenticate("missing-user", &AuthConfig::default(), None);

            assert_eq!(outcome.result, AuthResult::Success);
            assert_eq!(outcome.summary.reason.as_deref(), Some("face_matched"));
            assert_eq!(outcome.summary.best_match.unwrap().similarity, 0.95);
        });
    }

    #[test]
    fn authenticate_retries_when_no_face_is_detected() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = face_config();
        write_model_files(directory.path(), &mut config);
        enrolled_face(directory.path());
        let runtime = FakeRuntime {
            available: true,
            frame: Some(RgbFrame::new(2, 2, vec![0; 12]).unwrap()),
            detections: Vec::new(),
            ..FakeRuntime::default()
        };

        with_data_dir_override(directory.path(), || {
            let mut method = FaceAuth::with_runtime(config, Box::new(runtime));
            let outcome = method.authenticate("missing-user", &AuthConfig::default(), None);

            assert_eq!(outcome.result, AuthResult::Retry);
            assert_eq!(outcome.summary.reason.as_deref(), Some("no_face_detected"));
        });
    }
}
