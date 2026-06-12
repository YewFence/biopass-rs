use biopass_rs_auth::{migrate_config_at_path, read_config_from_path};
pub use biopass_rs_auth::{
    BiopassConfig, DetectionConfig, FaceMethodConfig, FingerConfig, FingerprintMethodConfig,
    MethodsConfig, ModelConfig, RecognitionConfig, StrategyConfig,
};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

use crate::paths::{get_config_dir, get_config_path, get_data_dir};

fn get_default_config(app: &AppHandle) -> BiopassConfig {
    let models_dir = get_data_dir(app)
        .map(|d| d.join("models"))
        .unwrap_or_else(|_| PathBuf::from("models"));

    let model_path = |name: &str| -> String { models_dir.join(name).to_string_lossy().to_string() };

    let mut config = BiopassConfig::default();

    // 只覆盖需要动态路径的部分
    config.methods.face.detection.model = model_path("yolov8n-face.onnx");
    config.methods.face.recognition.model = model_path("edgeface_s_gamma_05.onnx");
    config.methods.face.anti_spoofing.ai.model.path = model_path("mobilenetv3_antispoof.onnx");

    config.models = vec![
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
    ];

    config
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

    let config = read_config_from_path(&config_path)?;

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
