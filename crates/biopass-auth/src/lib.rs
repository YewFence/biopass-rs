pub mod camera;
pub mod config;
pub mod face_antispoofing;
pub mod face_auth;
pub mod face_detection;
pub mod face_recognition;
pub mod fingerprint_auth;
pub mod image_io;
pub mod inference;
pub mod manager;

pub use camera::{
    camera_available, capture_rgb_frame, list_video_devices, CameraRequest, FrameFormat, RgbFrame,
    VideoDevice,
};
pub use config::{
    config_exists, config_path, list_faces, migrate_config_schema, read_config, setup_config,
    user_data_dir, user_exists, AntiSpoofingConfig, AntiSpoofingModelConfig, BiopassConfig,
    DetectionConfig, FaceMethodConfig, FingerConfig, FingerprintMethodConfig, MethodConfig,
    MethodsConfig, ModelConfig, RecognitionConfig, StrategyConfig,
};
pub use face_antispoofing::{FaceAntiSpoofing, SpoofResult};
pub use face_auth::FaceAuth;
pub use face_detection::{FaceBox, FaceDetection, FaceDetector};
pub use face_recognition::{FaceMatch, FaceRecognizer};
pub use fingerprint_auth::FingerprintAuth;
pub use image_io::{decode_jpeg_rgb, encode_jpeg};
pub use inference::{OrtModel, OrtTensorInfo};
pub use manager::{
    AuthConfig, AuthManager, AuthMethod, AuthOutcome, AuthResult, ExecutionMode, PamCode,
};
