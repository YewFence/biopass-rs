use crate::{
    AuthConfig, FaceDetection, FaceMatch, FaceMethodConfig, IrLivenessSummary, RgbFrame,
    SpoofResult,
};
use std::sync::atomic::AtomicBool;

pub(super) trait FaceAuthRuntime: Send {
    fn clear(&mut self);

    fn is_available(&self, config: &FaceMethodConfig) -> bool;

    fn capture_frame(&mut self, config: &FaceMethodConfig, debug: bool)
        -> Result<RgbFrame, String>;

    fn detect_faces(
        &mut self,
        config: &FaceMethodConfig,
        frame: &RgbFrame,
    ) -> Result<Vec<FaceDetection>, String>;

    fn embedding(
        &mut self,
        config: &FaceMethodConfig,
        frame: &RgbFrame,
    ) -> Result<Vec<f32>, String>;

    fn match_embeddings(
        &mut self,
        config: &FaceMethodConfig,
        enrolled_embedding: &[f32],
        candidate_embedding: &[f32],
    ) -> Result<FaceMatch, String>;

    fn detect_rgb_spoof(
        &mut self,
        config: &FaceMethodConfig,
        face: &RgbFrame,
    ) -> Result<SpoofResult, String>;

    fn run_ir_liveness(
        &mut self,
        config: &FaceMethodConfig,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
        rgb_frame: &RgbFrame,
        rgb_face_box: crate::FaceBox,
    ) -> Result<IrLivenessSummary, String>;
}
