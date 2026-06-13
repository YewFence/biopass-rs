use super::serde_defaults::*;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::collections::HashSet;

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
pub struct RgbAntiSpoofingConfig {
    pub enable: bool,
    #[serde(default)]
    pub retries: u32,
    #[serde(default = "default_antispoofing_retry_delay")]
    pub retry_delay_ms: u32,
    pub model: AntiSpoofingModelConfig,
}

impl Default for RgbAntiSpoofingConfig {
    fn default() -> Self {
        Self {
            enable: false,
            retries: 0,
            retry_delay_ms: default_antispoofing_retry_delay(),
            model: AntiSpoofingModelConfig::default(),
        }
    }
}

impl<'de> Deserialize<'de> for RgbAntiSpoofingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct Raw {
            #[serde(default)]
            enable: bool,
            #[serde(default)]
            retries: u32,
            #[serde(default = "default_antispoofing_retry_delay")]
            retry_delay_ms: u32,
            #[serde(default)]
            model: Option<Value>,
            #[serde(default)]
            threshold: Option<f32>,
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
            retries: raw.retries,
            retry_delay_ms: raw.retry_delay_ms,
            model,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IrAntiSpoofingConfig {
    pub enable: bool,
    #[serde(default)]
    pub retries: u32,
    #[serde(default = "default_antispoofing_retry_delay")]
    pub retry_delay_ms: u32,
    pub camera: Option<String>,
    pub warmup_delay_ms: i32,
    #[serde(default = "default_ir_min_face_area_ratio")]
    pub min_face_area_ratio: f32,
    pub model: AntiSpoofingModelConfig,
}

impl Default for IrAntiSpoofingConfig {
    fn default() -> Self {
        Self {
            enable: false,
            retries: 0,
            retry_delay_ms: default_antispoofing_retry_delay(),
            camera: None,
            warmup_delay_ms: 300,
            min_face_area_ratio: default_ir_min_face_area_ratio(),
            model: AntiSpoofingModelConfig::default(),
        }
    }
}

impl<'de> Deserialize<'de> for IrAntiSpoofingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct Raw {
            #[serde(default)]
            enable: bool,
            #[serde(default)]
            retries: u32,
            #[serde(default = "default_antispoofing_retry_delay")]
            retry_delay_ms: u32,
            #[serde(default)]
            camera: Option<String>,
            #[serde(default = "default_ir_warmup_delay")]
            warmup_delay_ms: i32,
            #[serde(default = "default_ir_min_face_area_ratio")]
            min_face_area_ratio: f32,
            #[serde(default)]
            model: Option<Value>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let mut model = AntiSpoofingModelConfig::default();
        if let Some(model_value) = raw.model {
            read_antispoofing_model(&model_value, &mut model);
        }

        Ok(Self {
            enable: raw.enable,
            retries: raw.retries,
            retry_delay_ms: raw.retry_delay_ms,
            camera: raw.camera,
            warmup_delay_ms: raw.warmup_delay_ms,
            min_face_area_ratio: raw.min_face_area_ratio,
            model,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct AntiSpoofingConfig {
    pub rgb: RgbAntiSpoofingConfig,
    pub ir: IrAntiSpoofingConfig,
}

impl<'de> Deserialize<'de> for AntiSpoofingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(default)]
            rgb: Option<RgbAntiSpoofingConfig>,
            #[serde(default)]
            ir: Option<IrAntiSpoofingConfig>,
        }

        let content: serde_yaml::Mapping = serde_yaml::from_value(
            Value::deserialize(deserializer)
                .map_err(|error| serde::de::Error::custom(format!("{error}")))?,
        )
        .map_err(|error: serde_yaml::Error| serde::de::Error::custom(format!("{error}")))?;

        let has_legacy_field = content.contains_key("enable")
            || content.contains_key("model")
            || content.contains_key("threshold")
            || content.contains_key("ir_camera")
            || content.contains_key("ir_warmup_delay_ms")
            || content.contains_key("ai"); // old 'ai' key

        if has_legacy_field {
            return Err(serde::de::Error::custom(
                "the `anti_spoofing` schema changed: `ai` was renamed to `rgb` and `ir` now requires a `model` field. \
                 Run the migration:\n\
                 \n\
                 biopass-rs-helper migrate-config <username>\n\
                 \n\
                 Or update your config manually to:\n\
                 \n\
                 anti_spoofing:\n  \
                   rgb:\n    \
                     enable: <bool>\n    \
                     model: { path: <path>, threshold: <0..1> }\n  \
                   ir:\n    \
                     enable: <bool>\n    \
                     camera: <path, e.g. /dev/video2>\n    \
                     model: { path: <path>, threshold: <0..1> }\n    \
                     warmup_delay_ms: 300",
            ));
        }

        let raw = Raw::deserialize(Value::Mapping(content))
            .map_err(|error| serde::de::Error::custom(format!("{error}")))?;

        Ok(Self {
            rgb: raw.rgb.unwrap_or_default(),
            ir: raw.ir.unwrap_or_default(),
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
        if anti_spoofing.ir.camera.is_none() {
            if let Some(legacy) = raw.ir_camera {
                if legacy.enable {
                    anti_spoofing.ir.camera = Some(format!("/dev/video{}", legacy.device_id));
                    anti_spoofing.ir.enable = true;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MethodsConfig {
    #[serde(default)]
    pub face: FaceMethodConfig,
    #[serde(default)]
    pub fingerprint: FingerprintMethodConfig,
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

impl BiopassConfig {
    pub fn runtime_auth_config(&self) -> crate::manager::AuthConfig {
        crate::manager::AuthConfig {
            debug: self.strategy.debug,
            antispoof: self.methods.face.anti_spoofing.rgb.enable
                || self.methods.face.anti_spoofing.ir.enable,
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

pub(super) fn read_antispoofing_model(value: &Value, model: &mut AntiSpoofingModelConfig) {
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
