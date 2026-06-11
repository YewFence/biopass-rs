use biopass_auth::migrate_config_at_path;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

use crate::paths::{get_config_dir, get_config_path, get_data_dir};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BiopassConfig {
    pub strategy: StrategyConfig,
    pub methods: MethodsConfig,
    pub models: Vec<ModelConfig>,
    pub appearance: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StrategyConfig {
    #[serde(default)]
    pub debug: bool,
    pub execution_mode: String,
    pub order: Vec<String>,
    #[serde(default = "default_ignored_services")]
    pub ignore_services: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MethodsConfig {
    pub face: FaceMethodConfig,
    pub fingerprint: FingerprintMethodConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FaceMethodConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_face_retries")]
    pub retries: u32,
    #[serde(default = "default_face_delay")]
    pub retry_delay: u32,
    #[serde(default)]
    pub camera: Option<String>,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default)]
    pub recognition: RecognitionConfig,
    #[serde(default)]
    pub anti_spoofing: AntiSpoofingConfig,
    #[serde(default = "default_face_auto_optimize_camera")]
    pub auto_optimize_camera: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DetectionConfig {
    pub model: String,
    pub threshold: f32,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            model: "models/yolov8n-face.onnx".to_string(),
            threshold: 0.8,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecognitionConfig {
    pub model: String,
    pub threshold: f32,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            model: "models/edgeface_s_gamma_05.onnx".to_string(),
            threshold: 0.8,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AntiSpoofingModelConfig {
    pub path: String,
    pub threshold: f32,
}

impl Default for AntiSpoofingModelConfig {
    fn default() -> Self {
        Self {
            path: "models/mobilenetv3_antispoof.onnx".to_string(),
            threshold: 0.8,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AiAntiSpoofingConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub model: AntiSpoofingModelConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct IrAntiSpoofingConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub camera: Option<String>,
    #[serde(default = "default_ir_warmup_delay")]
    pub warmup_delay_ms: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AntiSpoofingConfig {
    #[serde(default)]
    pub ai: AiAntiSpoofingConfig,
    #[serde(default)]
    pub ir: IrAntiSpoofingConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FingerprintMethodConfig {
    pub enable: bool,
    #[serde(default = "default_fingerprint_retries")]
    pub retries: u32,
    #[serde(default = "default_fingerprint_timeout", alias = "retry_delay")]
    pub timeout: u32,
    #[serde(default)]
    pub fingers: Vec<FingerConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FingerConfig {
    pub name: String,
    pub created_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelConfig {
    pub path: String,
    #[serde(rename = "type")]
    pub model_type: String,
}

fn default_face_retries() -> u32 {
    5
}
fn default_face_delay() -> u32 {
    200
}
fn default_face_auto_optimize_camera() -> bool {
    true
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
fn default_ignored_services() -> Vec<String> {
    vec!["polkit-1".to_string(), "pkexec".to_string()]
}

fn get_default_config(app: &AppHandle) -> BiopassConfig {
    let models_dir = get_data_dir(app)
        .map(|d| d.join("models"))
        .unwrap_or_else(|_| PathBuf::from("models"));

    let model_path = |name: &str| -> String { models_dir.join(name).to_string_lossy().to_string() };

    BiopassConfig {
        strategy: StrategyConfig {
            debug: false,
            execution_mode: "parallel".to_string(),
            order: vec!["face".to_string(), "fingerprint".to_string()],
            ignore_services: default_ignored_services(),
        },
        methods: MethodsConfig {
            face: FaceMethodConfig {
                enable: true,
                retries: 5,
                retry_delay: 200,
                camera: None,
                detection: DetectionConfig {
                    model: model_path("yolov8n-face.onnx"),
                    threshold: 0.8,
                },
                recognition: RecognitionConfig {
                    model: model_path("edgeface_s_gamma_05.onnx"),
                    threshold: 0.8,
                },
                anti_spoofing: AntiSpoofingConfig {
                    ai: AiAntiSpoofingConfig {
                        enable: true,
                        model: AntiSpoofingModelConfig {
                            path: model_path("mobilenetv3_antispoof.onnx"),
                            threshold: 0.8,
                        },
                    },
                    ir: IrAntiSpoofingConfig {
                        enable: false,
                        camera: None,
                        warmup_delay_ms: 300,
                    },
                },
                auto_optimize_camera: true,
            },
            fingerprint: FingerprintMethodConfig {
                enable: false,
                retries: 1,
                timeout: 5000,
                fingers: vec![],
            },
        },
        models: vec![
            ModelConfig {
                path: model_path("yolov8n-face.onnx"),
                model_type: "detection".to_string(),
            },
            ModelConfig {
                path: model_path("edgeface_s_gamma_05.onnx"),
                model_type: "recognition".to_string(),
            },
            ModelConfig {
                path: model_path("mobilenetv3_antispoof.onnx"),
                model_type: "anti-spoofing".to_string(),
            },
        ],
        appearance: "system".to_string(),
    }
}

/// Returned by `load_config` so the frontend can show a one-time
/// notice when the on-disk schema was migrated automatically.
#[derive(Debug, Serialize, Clone)]
pub struct LoadConfigResult {
    pub config: BiopassConfig,
    /// True if the on-disk config was rewritten to the current schema.
    /// The frontend should surface a one-time confirmation.
    pub migrated: bool,
}

/// Tauri command — exposed to the frontend. Returns the loaded config plus
/// a flag indicating whether the on-disk file was migrated to the current
/// schema. The frontend uses the flag to show a one-time confirmation.
#[tauri::command]
pub fn load_config(app: AppHandle) -> Result<LoadConfigResult, String> {
    let result = load_config_internal(&app)?;
    Ok(LoadConfigResult {
        config: result.config,
        migrated: result.migrated,
    })
}

/// Internal helper that runs the same load/migrate logic but discards the
/// `migrated` flag. Used by other Tauri commands that only need the config.
pub fn load_config_internal(app: &AppHandle) -> Result<LoadConfigResult, String> {
    let config_path = get_config_path(app)?;

    if !config_path.exists() {
        let config = get_default_config(app);
        if let Err(e) = save_config(app.clone(), config.clone()) {
            eprintln!("Warning: failed to write default config: {e}");
        }
        return Ok(LoadConfigResult {
            config,
            migrated: false,
        });
    }

    let migrated = migrate_config_at_path(&config_path)
        .map_err(|e| format!("Failed to migrate config: {}", e))?;

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: BiopassConfig = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    Ok(LoadConfigResult { config, migrated })
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: BiopassConfig) -> Result<(), String> {
    let config_dir = get_config_dir(&app)?;
    let config_path = get_config_path(&app)?;

    let yaml_content =
        serde_yaml::to_string(&config).map_err(|e| format!("Failed to serialize config: {}", e))?;

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    fs::write(&config_path, yaml_content).map_err(|e| format!("Failed to write config file: {}", e))
}
