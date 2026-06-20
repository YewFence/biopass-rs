use super::camera::face_camera_request;
use super::runtime::FaceAuthRuntime;
use crate::{
    camera_available, AuthConfig, CameraSession, FaceAntiSpoofing, FaceDetection, FaceDetector,
    FaceMatch, FaceMethodConfig, FaceRecognizer, IrLivenessSummary, RgbFrame, SpoofResult,
};
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub(super) struct DefaultFaceAuthRuntime {
    detector: Option<FaceDetector>,
    recognizer: Option<FaceRecognizer>,
    anti_spoofing: Option<FaceAntiSpoofing>,
    ir_anti_spoofing: Option<FaceAntiSpoofing>,
    camera_session: Option<CameraSession>,
    pub(super) ir_camera_session: Option<CameraSession>,
}

impl DefaultFaceAuthRuntime {
    pub(super) fn detector(
        &mut self,
        config: &FaceMethodConfig,
    ) -> Result<&mut FaceDetector, String> {
        if self.detector.is_none() {
            self.detector = Some(FaceDetector::load_with_threshold(
                &config.detection.model,
                config.detection.threshold,
            )?);
        }

        Ok(self.detector.as_mut().unwrap())
    }

    fn recognizer(&mut self, config: &FaceMethodConfig) -> Result<&mut FaceRecognizer, String> {
        if self.recognizer.is_none() {
            self.recognizer = Some(FaceRecognizer::load(
                &config.recognition.model,
                config.recognition.threshold,
            )?);
        }

        Ok(self.recognizer.as_mut().unwrap())
    }

    fn anti_spoofing(
        &mut self,
        config: &FaceMethodConfig,
    ) -> Result<&mut FaceAntiSpoofing, String> {
        if self.anti_spoofing.is_none() {
            let model = &config.anti_spoofing.rgb.model;
            self.anti_spoofing = Some(FaceAntiSpoofing::load(&model.path, model.threshold)?);
        }

        Ok(self.anti_spoofing.as_mut().unwrap())
    }

    pub(super) fn ir_anti_spoofing(
        &mut self,
        config: &FaceMethodConfig,
    ) -> Result<&mut FaceAntiSpoofing, String> {
        if self.ir_anti_spoofing.is_none() {
            let model = &config.anti_spoofing.ir.model;
            self.ir_anti_spoofing = Some(FaceAntiSpoofing::load(&model.path, model.threshold)?);
        }

        Ok(self.ir_anti_spoofing.as_mut().unwrap())
    }
}

impl FaceAuthRuntime for DefaultFaceAuthRuntime {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn is_available(&self, config: &FaceMethodConfig) -> bool {
        config.enable
            && camera_available(&face_camera_request(
                config.camera.as_deref(),
                config.auto_optimize_camera,
                false,
            ))
    }

    fn capture_frame(
        &mut self,
        config: &FaceMethodConfig,
        debug: bool,
    ) -> Result<RgbFrame, String> {
        if self.camera_session.is_none() {
            let request =
                face_camera_request(config.camera.as_deref(), config.auto_optimize_camera, debug);
            let mut session = CameraSession::open(&request)?;
            session.warmup(request.warmup_frames)?;
            self.camera_session = Some(session);
        }

        self.camera_session
            .as_mut()
            .expect("camera session is initialized")
            .next_frame()
    }

    fn detect_faces(
        &mut self,
        config: &FaceMethodConfig,
        frame: &RgbFrame,
    ) -> Result<Vec<FaceDetection>, String> {
        self.detector(config)?.detect(frame)
    }

    fn embedding(
        &mut self,
        config: &FaceMethodConfig,
        frame: &RgbFrame,
    ) -> Result<Vec<f32>, String> {
        self.recognizer(config)?.embedding(frame)
    }

    fn match_embeddings(
        &mut self,
        config: &FaceMethodConfig,
        enrolled_embedding: &[f32],
        candidate_embedding: &[f32],
    ) -> Result<FaceMatch, String> {
        self.recognizer(config)?
            .match_embeddings(enrolled_embedding, candidate_embedding)
    }

    fn detect_rgb_spoof(
        &mut self,
        config: &FaceMethodConfig,
        face: &RgbFrame,
    ) -> Result<SpoofResult, String> {
        self.anti_spoofing(config)?.detect(face)
    }

    fn run_ir_liveness(
        &mut self,
        config: &FaceMethodConfig,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
        rgb_frame: &RgbFrame,
        rgb_face_box: crate::FaceBox,
    ) -> Result<IrLivenessSummary, String> {
        self.run_ir_check_with_retries(
            config,
            username,
            auth_config,
            cancel_signal,
            rgb_frame,
            rgb_face_box,
        )
    }
}
