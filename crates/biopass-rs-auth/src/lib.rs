pub mod auth_session;
pub mod camera;
pub mod config;
pub mod face_antispoofing;
pub mod face_auth;
pub mod face_detection;
pub mod face_recognition;
pub mod face_store;
pub mod face_tools;
pub mod fingerprint_auth;
pub mod image_io;
pub mod inference;
pub mod installer;
pub mod logging;
pub mod manager;

#[cfg(test)]
pub(crate) static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_prefix(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

pub use auth_session::{
    authenticate_user, authenticate_user_with, build_auth_manager, AuthSessionPaths,
    AuthSessionResult, AuthSessionStatus,
};
pub use camera::{
    camera_available, capture_rgb_frame, list_video_devices, CameraRequest, CameraSession,
    FrameFormat, RgbFrame, VideoDevice,
};
pub use config::{
    bootstrap_config_at, config_parse_error_message, config_path, current_username, list_faces,
    migrate_config_at_path, read_config_from_path, reset_config_at_path, set_config_path_override,
    set_data_dir_override, setup_config, user_data_dir, user_exists, write_config_to_path,
    AntiSpoofingConfig, AntiSpoofingModelConfig, AuthHistoryConfig, BiopassConfig,
    BootstrapOutcome, ConsoleLoggingConfig, DetectionConfig, DiagnosticsLoggingConfig,
    FaceMethodConfig, FileLoggingConfig, FingerConfig, FingerprintMethodConfig, LogRetentionConfig,
    LogRotationConfig, LoggingConfig, MethodConfig, MethodsConfig, ModelConfig, RecognitionConfig,
    StrategyConfig, CONFIG_PATH_ENV, DATA_DIR_ENV,
};
pub use face_antispoofing::{FaceAntiSpoofing, SpoofResult};
pub use face_auth::FaceAuth;
pub use face_detection::{FaceBox, FaceDetection, FaceDetector};
pub use face_recognition::{FaceMatch, FaceRecognizer};
pub use face_store::{
    delete_enrolled_face, faces_dir, list_enrolled_faces, save_enrolled_face_jpeg,
};
pub use face_tools::{
    capture_camera_frame, capture_face_jpeg, capture_face_jpeg_with_detector, crop_face_jpeg,
    crop_largest_face_jpeg,
};
pub use fingerprint_auth::{EnrollStatusCallback, FingerprintAuth};
pub use image_io::{decode_jpeg_rgb, encode_jpeg};
pub use inference::{F32TensorOutput, InferenceModel, TensorInfo};
pub use installer::{
    check_models_present, download_models, import_legacy_faces_for_user, import_legacy_faces_from,
    run_ldconfig, ImportLegacyFacesOutcome,
};
pub use logging::{
    auth_history_dir, emit_log, log_file_path, logs_dir, make_auth_summary_id, read_auth_history,
    read_log_tail, runtime_logging, save_failed_frames_enabled, set_runtime_logging,
    write_auth_summary, AuthAttemptSummary, AuthMethodSummary, AuthMethodSummaryResult,
    AuthSessionSummary, AuthSummaryResult, FaceBestMatchSummary, IrLivenessSummary, LogComponent,
    RuntimeLoggingConfig,
};
pub use manager::{
    AuthConfig, AuthManager, AuthMethod, AuthOutcome, AuthResult, ExecutionMode, MethodAuthOutcome,
    PamCode,
};
