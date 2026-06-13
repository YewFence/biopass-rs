use chrono::Local;

pub mod camera;
pub mod config;
pub mod face_antispoofing;
pub mod face_auth;
pub mod face_detection;
pub mod face_recognition;
pub mod fingerprint_auth;
pub mod image_io;
pub mod inference;
pub mod installer;
pub mod manager;

#[derive(Clone, Copy)]
pub enum LogLevel {
    Info,
    Debug,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_prefix(self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

pub fn emit_log(level: LogLevel, debug_enabled: bool, scope: &str, message: &str) {
    if !debug_enabled {
        return;
    }
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    eprintln!(
        "[{}] [biopass-rs] [{}] {}: {}",
        timestamp,
        level.as_prefix(),
        scope,
        message
    );
}

pub use camera::{
    camera_available, capture_rgb_frame, list_video_devices, CameraRequest, CameraSession,
    FrameFormat, RgbFrame, VideoDevice,
};
pub use config::{
    bootstrap_config_at, config_parse_error_message, config_path, current_username, list_faces,
    migrate_config_at_path, read_config_from_path, reset_config_at_path, set_config_path_override,
    set_data_dir_override, setup_config, upstream_config_path_relative, user_data_dir, user_exists,
    write_config_to_path, AntiSpoofingConfig, AntiSpoofingModelConfig, BiopassConfig,
    BootstrapOutcome, DetectionConfig, FaceMethodConfig, FingerConfig, FingerprintMethodConfig,
    MethodConfig, MethodsConfig, ModelConfig, RecognitionConfig, StrategyConfig, CONFIG_PATH_ENV,
    DATA_DIR_ENV,
};
pub use face_antispoofing::{FaceAntiSpoofing, SpoofResult};
pub use face_auth::FaceAuth;
pub use face_detection::{FaceBox, FaceDetection, FaceDetector};
pub use face_recognition::{FaceMatch, FaceRecognizer};
pub use fingerprint_auth::{EnrollStatusCallback, FingerprintAuth};
pub use image_io::{decode_jpeg_rgb, encode_jpeg};
pub use inference::{F32TensorOutput, InferenceModel, TensorInfo};
pub use installer::{check_models_present, download_models, run_ldconfig};
pub use manager::{
    AuthConfig, AuthManager, AuthMethod, AuthOutcome, AuthResult, ExecutionMode, PamCode,
};
