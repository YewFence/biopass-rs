use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = ".config/com.ticklab.biopass/config.yaml";
const DATA_DIR: &str = ".local/share/com.ticklab.biopass";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BiopassConfig {
    #[serde(default)]
    pub strategy: StrategyConfig,
    #[serde(default)]
    pub methods: MethodsConfig,
    #[serde(default)]
    pub models: Vec<ModelConfig>,
    #[serde(default = "default_appearance")]
    pub appearance: String,
}

impl Default for BiopassConfig {
    fn default() -> Self {
        Self {
            strategy: StrategyConfig::default(),
            methods: MethodsConfig::default(),
            models: Vec::new(),
            appearance: default_appearance(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrategyConfig {
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
    #[serde(default = "default_order", deserialize_with = "deserialize_order")]
    pub order: Vec<String>,
    #[serde(default = "default_ignored_services")]
    pub ignore_services: Vec<String>,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            debug: false,
            execution_mode: default_execution_mode(),
            order: default_order(),
            ignore_services: default_ignored_services(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionConfig {
    #[serde(default = "default_detection_model")]
    pub model: String,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            model: default_detection_model(),
            threshold: default_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecognitionConfig {
    #[serde(default = "default_recognition_model")]
    pub model: String,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            model: default_recognition_model(),
            threshold: default_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AntiSpoofingModelConfig {
    #[serde(default = "default_antispoofing_model")]
    pub path: String,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
}

impl Default for AntiSpoofingModelConfig {
    fn default() -> Self {
        Self {
            path: default_antispoofing_model(),
            threshold: default_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AntiSpoofingConfig {
    pub enable: bool,
    pub model: AntiSpoofingModelConfig,
    pub ir_camera: Option<String>,
    pub ir_warmup_delay_ms: i32,
}

impl Default for AntiSpoofingConfig {
    fn default() -> Self {
        Self {
            enable: false,
            model: AntiSpoofingModelConfig::default(),
            ir_camera: None,
            ir_warmup_delay_ms: 300,
        }
    }
}

impl<'de> Deserialize<'de> for AntiSpoofingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct Raw {
            #[serde(default)]
            enable: bool,
            #[serde(default)]
            model: Option<Value>,
            #[serde(default)]
            threshold: Option<f32>,
            #[serde(default)]
            ir_camera: Option<String>,
            #[serde(default = "default_ir_warmup_delay")]
            ir_warmup_delay_ms: i32,
        }

        let raw = Raw::deserialize(deserializer)?;
        let mut model = AntiSpoofingModelConfig::default();
        if let Some(model_value) = raw.model {
            read_antispoofing_model(&model_value, &mut model);
        }
        if let Some(threshold) = raw.threshold {
            model.threshold = threshold;
        }

        Ok(Self {
            enable: raw.enable,
            model,
            ir_camera: raw.ir_camera,
            ir_warmup_delay_ms: raw.ir_warmup_delay_ms,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FaceMethodConfig {
    pub enable: bool,
    pub retries: u32,
    pub retry_delay: u32,
    pub camera: Option<String>,
    pub detection: DetectionConfig,
    pub recognition: RecognitionConfig,
    pub anti_spoofing: AntiSpoofingConfig,
    pub auto_optimize_camera: bool,
}

impl Default for FaceMethodConfig {
    fn default() -> Self {
        Self {
            enable: true,
            retries: 5,
            retry_delay: 200,
            camera: None,
            detection: DetectionConfig::default(),
            recognition: RecognitionConfig::default(),
            anti_spoofing: AntiSpoofingConfig::default(),
            auto_optimize_camera: true,
        }
    }
}

impl<'de> Deserialize<'de> for FaceMethodConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct LegacyIrCamera {
            #[serde(default)]
            enable: bool,
            #[serde(default)]
            device_id: i32,
        }

        #[derive(Deserialize)]
        struct Raw {
            #[serde(default = "default_true")]
            enable: bool,
            #[serde(default = "default_face_retries")]
            retries: u32,
            #[serde(default = "default_face_retry_delay")]
            retry_delay: u32,
            #[serde(default)]
            camera: Option<String>,
            #[serde(default)]
            detection: DetectionConfig,
            #[serde(default)]
            recognition: RecognitionConfig,
            #[serde(default)]
            anti_spoofing: AntiSpoofingConfig,
            #[serde(default)]
            ir_camera: Option<LegacyIrCamera>,
            #[serde(default = "default_true")]
            auto_optimize_camera: bool,
        }

        let raw = Raw::deserialize(deserializer)?;
        let mut anti_spoofing = raw.anti_spoofing;
        if anti_spoofing.ir_camera.is_none() {
            if let Some(legacy) = raw.ir_camera {
                if legacy.enable {
                    anti_spoofing.ir_camera = Some(format!("/dev/video{}", legacy.device_id));
                }
            }
        }

        Ok(Self {
            enable: raw.enable,
            retries: raw.retries,
            retry_delay: raw.retry_delay,
            camera: raw.camera,
            detection: raw.detection,
            recognition: raw.recognition,
            anti_spoofing,
            auto_optimize_camera: raw.auto_optimize_camera,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FingerConfig {
    pub name: String,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FingerprintMethodConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_fingerprint_retries")]
    pub retries: u32,
    #[serde(default = "default_fingerprint_timeout", alias = "retry_delay")]
    pub timeout: u32,
    #[serde(default)]
    pub fingers: Vec<FingerConfig>,
}

impl Default for FingerprintMethodConfig {
    fn default() -> Self {
        Self {
            enable: false,
            retries: default_fingerprint_retries(),
            timeout: default_fingerprint_timeout(),
            fingers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MethodsConfig {
    #[serde(default)]
    pub face: FaceMethodConfig,
    #[serde(default)]
    pub fingerprint: FingerprintMethodConfig,
}

impl Default for MethodsConfig {
    fn default() -> Self {
        Self {
            face: FaceMethodConfig::default(),
            fingerprint: FingerprintMethodConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelConfig {
    pub path: String,
    #[serde(rename = "type")]
    pub model_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodConfig {
    pub name: String,
    pub enabled: bool,
    pub retries: u32,
    pub retry_delay_ms: u32,
}

pub fn config_path(username: &str) -> PathBuf {
    match home_dir_for_user(username) {
        Some(home) => home.join(CONFIG_FILE),
        None => std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(CONFIG_FILE))
            .unwrap_or_else(|| PathBuf::from("/etc/com.ticklab.biopass/config.yaml")),
    }
}

pub fn config_exists(username: &str) -> bool {
    config_path(username).is_file()
}

pub fn user_exists(username: &str) -> bool {
    home_dir_for_user(username).is_some()
}

pub fn read_config(username: &str) -> BiopassConfig {
    let path = config_path(username);
    let Ok(config_text) = fs::read_to_string(path) else {
        return BiopassConfig::default();
    };

    serde_yaml::from_str::<BiopassConfig>(&config_text).unwrap_or_default()
}

pub fn migrate_config_schema(username: &str) -> io::Result<bool> {
    let path = config_path(username);
    let Ok(config_text) = fs::read_to_string(&path) else {
        return Ok(false);
    };
    let mut yaml = serde_yaml::from_str::<Value>(&config_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let Some(face) = yaml
        .get_mut("methods")
        .and_then(Value::as_mapping_mut)
        .and_then(|methods| methods.get_mut(Value::String("face".to_string())))
        .and_then(Value::as_mapping_mut)
    else {
        return Ok(false);
    };

    let (new_anti, needs_migration) = migrated_antispoofing(face);
    if !needs_migration {
        return Ok(false);
    }
    face.insert(Value::String("anti_spoofing".to_string()), new_anti);
    face.remove(Value::String("ir_camera".to_string()));

    let serialized = serde_yaml::to_string(&yaml).map_err(io::Error::other)?;
    fs::write(path, serialized)?;
    Ok(true)
}

pub fn user_data_dir(username: &str) -> PathBuf {
    home_dir_for_user(username)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .map(|home| home.join(DATA_DIR))
        .unwrap_or_default()
}

pub fn list_faces(username: &str) -> Vec<PathBuf> {
    let faces_dir = user_data_dir(username).join("faces");
    let Ok(entries) = fs::read_dir(faces_dir) else {
        return Vec::new();
    };

    let mut faces = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_supported_face_image(path))
        .collect::<Vec<_>>();
    faces.sort();
    faces
}

pub fn setup_config(username: &str) -> io::Result<()> {
    let data_dir = user_data_dir(username);
    fs::create_dir_all(data_dir.join("faces"))?;
    fs::create_dir_all(data_dir.join("debugs"))?;
    Ok(())
}

impl BiopassConfig {
    pub fn runtime_auth_config(&self) -> crate::manager::AuthConfig {
        crate::manager::AuthConfig {
            debug: self.strategy.debug,
            antispoof: self.methods.face.anti_spoofing.enable
                || self
                    .methods
                    .face
                    .anti_spoofing
                    .ir_camera
                    .as_ref()
                    .is_some_and(|camera| !camera.is_empty()),
        }
    }

    pub fn execution_mode(&self) -> crate::manager::ExecutionMode {
        if self.strategy.execution_mode == "sequential" {
            crate::manager::ExecutionMode::Sequential
        } else {
            crate::manager::ExecutionMode::Parallel
        }
    }

    pub fn auth_methods(&self) -> Vec<MethodConfig> {
        self.strategy
            .order
            .iter()
            .filter_map(|method| match method.as_str() {
                "face" => Some(MethodConfig {
                    name: method.clone(),
                    enabled: self.methods.face.enable,
                    retries: self.methods.face.retries,
                    retry_delay_ms: self.methods.face.retry_delay,
                }),
                "fingerprint" => Some(MethodConfig {
                    name: method.clone(),
                    enabled: self.methods.fingerprint.enable,
                    retries: self.methods.fingerprint.retries,
                    retry_delay_ms: self.methods.fingerprint.timeout,
                }),
                _ => None,
            })
            .filter(|method| method.enabled)
            .collect()
    }

    pub fn ignores_service(&self, service: &str) -> bool {
        !service.is_empty()
            && self
                .strategy
                .ignore_services
                .iter()
                .any(|ignored| ignored == service)
    }
}

fn migrated_antispoofing(face: &mut Mapping) -> (Value, bool) {
    let anti = face
        .get(Value::String("anti_spoofing".to_string()))
        .and_then(Value::as_mapping);

    let mut enable = false;
    let mut model = AntiSpoofingModelConfig::default();
    let mut ir_camera_path = None;
    let mut warmup_delay = default_ir_warmup_delay();

    if let Some(anti) = anti {
        if let Some(value) = anti
            .get(Value::String("enable".to_string()))
            .and_then(Value::as_bool)
        {
            enable = value;
        }
        if let Some(value) = anti.get(Value::String("model".to_string())) {
            read_antispoofing_model(value, &mut model);
        }
        if let Some(value) = anti
            .get(Value::String("threshold".to_string()))
            .and_then(Value::as_f64)
        {
            model.threshold = value as f32;
        }
        if let Some(value) = anti
            .get(Value::String("ir_camera".to_string()))
            .and_then(Value::as_str)
        {
            ir_camera_path = Some(value.to_string());
        }
        if let Some(value) = anti
            .get(Value::String("ir_warmup_delay_ms".to_string()))
            .and_then(Value::as_i64)
        {
            warmup_delay = value as i32;
        }
    }

    if ir_camera_path.as_deref().unwrap_or_default().is_empty() {
        if let Some(legacy_ir) = face
            .get(Value::String("ir_camera".to_string()))
            .and_then(Value::as_mapping)
        {
            let enabled = legacy_ir
                .get(Value::String("enable".to_string()))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let device_id = legacy_ir
                .get(Value::String("device_id".to_string()))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            if enabled {
                ir_camera_path = Some(format!("/dev/video{}", device_id));
            }
        }
    }

    let has_legacy_face_ir = face.contains_key(Value::String("ir_camera".to_string()));
    let has_legacy_anti_threshold =
        anti.is_some_and(|anti| anti.contains_key(Value::String("threshold".to_string())));
    let has_legacy_anti_model_scalar = anti
        .and_then(|anti| anti.get(Value::String("model".to_string())))
        .is_some_and(Value::is_string);
    let has_new_model_map = anti
        .and_then(|anti| anti.get(Value::String("model".to_string())))
        .and_then(Value::as_mapping)
        .is_some_and(|model| {
            model.contains_key(Value::String("path".to_string()))
                && model.contains_key(Value::String("threshold".to_string()))
        });
    let has_new_ir_key =
        anti.is_some_and(|anti| anti.contains_key(Value::String("ir_camera".to_string())));
    let needs_migration = has_legacy_face_ir
        || has_legacy_anti_threshold
        || has_legacy_anti_model_scalar
        || !has_new_model_map
        || !has_new_ir_key;

    let mut model_value = Mapping::new();
    model_value.insert(Value::String("path".to_string()), Value::String(model.path));
    model_value.insert(
        Value::String("threshold".to_string()),
        Value::from(model.threshold),
    );

    let mut anti_value = Mapping::new();
    anti_value.insert(Value::String("enable".to_string()), Value::Bool(enable));
    anti_value.insert(
        Value::String("model".to_string()),
        Value::Mapping(model_value),
    );
    anti_value.insert(
        Value::String("ir_camera".to_string()),
        ir_camera_path.map(Value::String).unwrap_or(Value::Null),
    );
    anti_value.insert(
        Value::String("ir_warmup_delay_ms".to_string()),
        Value::from(warmup_delay),
    );

    (Value::Mapping(anti_value), needs_migration)
}

fn read_antispoofing_model(value: &Value, model: &mut AntiSpoofingModelConfig) {
    match value {
        Value::Mapping(map) => {
            if let Some(path) = map
                .get(Value::String("path".to_string()))
                .and_then(Value::as_str)
            {
                model.path = path.to_string();
            }
            if let Some(threshold) = map
                .get(Value::String("threshold".to_string()))
                .and_then(Value::as_f64)
            {
                model.threshold = threshold as f32;
            }
        }
        Value::String(path) => model.path = path.clone(),
        _ => {}
    }
}

fn deserialize_order<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<String>::deserialize(deserializer)?;
    Ok(normalize_order(raw))
}

fn normalize_order(order: Vec<String>) -> Vec<String> {
    let supported = ["face", "fingerprint"];
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for method in order {
        if supported.contains(&method.as_str()) && seen.insert(method.clone()) {
            normalized.push(method);
        }
    }

    for method in supported {
        if seen.insert(method.to_string()) {
            normalized.push(method.to_string());
        }
    }

    normalized
}

fn home_dir_for_user(username: &str) -> Option<PathBuf> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        if name != username {
            return None;
        }
        let home = parts.nth(4)?;
        if home.is_empty() {
            None
        } else {
            Some(PathBuf::from(home))
        }
    })
}

fn is_supported_face_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp" | "tga"
            )
        })
        .unwrap_or(false)
}

fn default_true() -> bool {
    true
}

fn default_face_retries() -> u32 {
    5
}

fn default_face_retry_delay() -> u32 {
    200
}

fn default_fingerprint_retries() -> u32 {
    1
}

fn default_fingerprint_timeout() -> u32 {
    5000
}

fn default_ir_warmup_delay() -> i32 {
    300
}

fn default_threshold() -> f32 {
    0.8
}

fn default_execution_mode() -> String {
    "parallel".to_string()
}

fn default_order() -> Vec<String> {
    vec!["face".to_string(), "fingerprint".to_string()]
}

fn default_ignored_services() -> Vec<String> {
    vec!["polkit-1".to_string(), "pkexec".to_string()]
}

fn default_appearance() -> String {
    "system".to_string()
}

fn default_detection_model() -> String {
    "models/yolov8n-face.onnx".to_string()
}

fn default_recognition_model() -> String {
    "models/edgeface_s_gamma_05.onnx".to_string()
}

fn default_antispoofing_model() -> String {
    "models/mobilenetv3_antispoof.onnx".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_and_normalizes_config() {
        let yaml = r#"
strategy:
  execution_mode: sequential
  order: [unknown, fingerprint, face, face]
  ignore_services: [sudo]
methods:
  face:
    retry_delay: 450
    ir_camera:
      enable: true
      device_id: 3
    anti_spoofing:
      enable: true
      model: old.onnx
      threshold: 0.42
  fingerprint:
    enable: true
    retry_delay: 9000
    fingers:
      - name: right-index-finger
        created_at: 12
"#;

        let config = serde_yaml::from_str::<BiopassConfig>(yaml).unwrap();

        assert_eq!(config.strategy.order, ["fingerprint", "face"]);
        assert_eq!(
            config.execution_mode(),
            crate::manager::ExecutionMode::Sequential
        );
        assert_eq!(config.methods.face.retry_delay, 450);
        assert_eq!(
            config.methods.face.anti_spoofing.ir_camera.as_deref(),
            Some("/dev/video3")
        );
        assert_eq!(config.methods.face.anti_spoofing.model.path, "old.onnx");
        assert_eq!(config.methods.face.anti_spoofing.model.threshold, 0.42);
        assert_eq!(config.methods.fingerprint.timeout, 9000);
    }

    #[test]
    fn auth_methods_follow_configured_order_and_enabled_flags() {
        let config = serde_yaml::from_str::<BiopassConfig>(
            r#"
strategy:
  order: [fingerprint, face]
methods:
  face:
    enable: false
  fingerprint:
    enable: true
    retries: 3
    timeout: 7000
"#,
        )
        .unwrap();

        let methods = config.auth_methods();

        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "fingerprint");
        assert_eq!(methods[0].retries, 3);
        assert_eq!(methods[0].retry_delay_ms, 7000);
    }

    #[test]
    fn migrates_legacy_antispoofing_schema() {
        let mut root = serde_yaml::from_str::<Value>(
            r#"
methods:
  face:
    ir_camera:
      enable: true
      device_id: 2
    anti_spoofing:
      enable: true
      model: legacy.onnx
      threshold: 0.67
"#,
        )
        .unwrap();

        let face = root
            .get_mut("methods")
            .and_then(Value::as_mapping_mut)
            .and_then(|methods| methods.get_mut(Value::String("face".to_string())))
            .and_then(Value::as_mapping_mut)
            .unwrap();

        let (anti, needs_migration) = migrated_antispoofing(face);

        assert!(needs_migration);
        assert_eq!(
            anti["model"]["path"],
            Value::String("legacy.onnx".to_string())
        );
        assert_eq!(anti["model"]["threshold"], Value::from(0.67_f32));
        assert_eq!(anti["ir_camera"], Value::String("/dev/video2".to_string()));
    }

    #[test]
    fn filters_supported_face_images() {
        assert!(is_supported_face_image(Path::new("a.JPG")));
        assert!(is_supported_face_image(Path::new("a.jpeg")));
        assert!(!is_supported_face_image(Path::new("a.txt")));
    }

    #[test]
    fn unknown_user_does_not_exist() {
        assert!(!user_exists("__biopass_missing_user_for_test__"));
    }
}
