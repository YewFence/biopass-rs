//! Field-level defaults used by serde deserialization.

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_face_retries() -> u32 {
    5
}

pub(super) fn default_face_retry_delay() -> u32 {
    200
}

pub(super) fn default_fingerprint_retries() -> u32 {
    1
}

pub(super) fn default_fingerprint_timeout() -> u32 {
    5000
}

pub(super) fn default_ir_warmup_delay() -> i32 {
    300
}

pub(super) fn default_antispoofing_retry_delay() -> u32 {
    200
}

pub(super) fn default_threshold() -> f32 {
    0.8
}

pub(super) fn default_execution_mode() -> String {
    "parallel".to_string()
}

pub(super) fn default_order() -> Vec<String> {
    vec!["face".to_string(), "fingerprint".to_string()]
}

pub(super) fn default_ignored_services() -> Vec<String> {
    vec!["polkit-1".to_string(), "pkexec".to_string()]
}

pub(super) fn default_appearance() -> String {
    "system".to_string()
}

pub(super) fn default_detection_model() -> String {
    "models/yolov8n-face.onnx".to_string()
}

pub(super) fn default_recognition_model() -> String {
    "models/edgeface_s_gamma_05.onnx".to_string()
}

pub(super) fn default_antispoofing_model() -> String {
    "models/mobilenetv3_antispoof.onnx".to_string()
}
