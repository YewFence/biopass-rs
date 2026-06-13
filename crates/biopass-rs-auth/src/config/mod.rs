mod bootstrap;
mod migration;
mod paths;
mod schema;
mod serde_defaults;

pub use bootstrap::{bootstrap_config_at, upstream_config_path_relative, BootstrapOutcome};
pub use migration::migrate_config_at_path;
pub use paths::{
    config_parse_error_message, config_path, current_username, list_faces, read_config_from_path,
    reset_config_at_path, set_config_path_override, set_data_dir_override, setup_config,
    user_data_dir, user_exists, write_config_to_path, CONFIG_PATH_ENV, DATA_DIR_ENV,
};
pub use schema::{
    AntiSpoofingConfig, AntiSpoofingModelConfig, BiopassConfig, DetectionConfig, FaceMethodConfig,
    FingerConfig, FingerprintMethodConfig, MethodConfig, MethodsConfig, ModelConfig,
    RecognitionConfig, StrategyConfig,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::migration::migrated_antispoofing;
    use crate::config::paths::is_supported_face_image;
    use serde_yaml::Value;
    use std::fs;
    use std::path::Path;

    #[test]
    fn normalizes_relative_model_paths_to_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("testuser");
        let config_path = home.join(".config/biopass-rs/config.yaml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        // 写入包含相对路径的配置
        fs::write(
            &config_path,
            r#"
methods:
  face:
    detection:
      model: models/yolov8n-face.onnx
    recognition:
      model: models/edgeface_s_gamma_05.onnx
    anti_spoofing:
      rgb:
        model:
          path: models/mobilenetv3_antispoof.onnx
      ir:
        model:
          path: models/ir_antispoof.onnx
models:
  - path: models/yolov8n-face.onnx
    type: detection
"#,
        )
        .unwrap();

        // 读取配置，应该自动规范化路径
        let config = read_config_from_path(&config_path).unwrap();

        // 验证路径已被规范化为绝对路径（包含 models/ 前缀）
        assert!(
            config
                .methods
                .face
                .detection
                .model
                .contains("models/yolov8n-face.onnx"),
            "detection model path should contain models/yolov8n-face.onnx: {}",
            config.methods.face.detection.model
        );
        assert!(
            config
                .methods
                .face
                .recognition
                .model
                .contains("models/edgeface_s_gamma_05.onnx"),
            "recognition model path should contain models/edgeface_s_gamma_05.onnx: {}",
            config.methods.face.recognition.model
        );
        assert!(
            config
                .methods
                .face
                .anti_spoofing
                .rgb
                .model
                .path
                .contains("models/mobilenetv3_antispoof.onnx"),
            "rgb antispoof model path should contain models/mobilenetv3_antispoof.onnx: {}",
            config.methods.face.anti_spoofing.rgb.model.path
        );
        assert!(
            config
                .methods
                .face
                .anti_spoofing
                .ir
                .model
                .path
                .contains("models/ir_antispoof.onnx"),
            "ir antispoof model path should contain models/ir_antispoof.onnx: {}",
            config.methods.face.anti_spoofing.ir.model.path
        );
        assert!(
            config.models[0].path.contains("models/yolov8n-face.onnx"),
            "models array path should contain models/yolov8n-face.onnx: {}",
            config.models[0].path
        );

        // 验证相对路径已经被转换为绝对路径（不再以 models/ 开头）
        assert!(
            !config.methods.face.detection.model.starts_with("models/"),
            "detection model path should not start with models/: {}",
            config.methods.face.detection.model
        );
    }

    #[test]
    fn leaves_absolute_model_paths_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.yaml");

        // 写入包含绝对路径的配置
        fs::write(
            &config_path,
            r#"
methods:
  face:
    detection:
      model: /absolute/path/to/model.onnx
    recognition:
      model: /another/absolute/model.onnx
"#,
        )
        .unwrap();

        let config = read_config_from_path(&config_path).unwrap();

        // 绝对路径应该保持不变
        assert_eq!(
            config.methods.face.detection.model,
            "/absolute/path/to/model.onnx"
        );
        assert_eq!(
            config.methods.face.recognition.model,
            "/another/absolute/model.onnx"
        );
    }

    #[test]
    fn reads_current_and_normalizes_config() {
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
      rgb:
        enable: true
        model: old.onnx
        threshold: 0.42
      ir:
        enable: true
        camera: /dev/video3
        warmup_delay_ms: 250
        model:
          path: old.onnx
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
            config.methods.face.anti_spoofing.ir.camera.as_deref(),
            Some("/dev/video3")
        );
        assert_eq!(config.methods.face.anti_spoofing.rgb.model.path, "old.onnx");
        assert_eq!(config.methods.face.anti_spoofing.rgb.model.threshold, 0.42);
        assert_eq!(config.methods.fingerprint.timeout, 9000);
    }

    #[test]
    fn legacy_anti_spoofing_top_level_enable_is_rejected() {
        let yaml = r#"
methods:
  face:
    anti_spoofing:
      enable: true
      ir_camera: /dev/video2
"#;

        let error = serde_yaml::from_str::<BiopassConfig>(yaml).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("anti_spoofing"),
            "expected migration error mentioning anti_spoofing, got: {message}"
        );
    }

    #[test]
    fn legacy_face_level_ir_camera_still_normalizes_to_ir_subconfig() {
        let yaml = r#"
methods:
  face:
    ir_camera:
      enable: true
      device_id: 2
"#;

        let config = serde_yaml::from_str::<BiopassConfig>(yaml).unwrap();
        assert!(config.methods.face.anti_spoofing.ir.enable);
        assert_eq!(
            config.methods.face.anti_spoofing.ir.camera.as_deref(),
            Some("/dev/video2")
        );
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
            anti["rgb"]["model"]["path"],
            Value::String("legacy.onnx".to_string())
        );
        assert_eq!(anti["rgb"]["model"]["threshold"], Value::from(0.67_f32));
        assert_eq!(anti["rgb"]["enable"], Value::Bool(true));
        assert_eq!(anti["ir"]["enable"], Value::Bool(true));
        assert_eq!(
            anti["ir"]["model"]["path"],
            Value::String("legacy.onnx".to_string())
        );
        assert_eq!(
            anti["ir"]["camera"],
            Value::String("/dev/video2".to_string())
        );
    }

    #[test]
    fn migrate_config_at_path_writes_current_antispoofing_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        fs::write(
            &path,
            r#"
methods:
  face:
    anti_spoofing:
      enable: true
      model: legacy.onnx
      threshold: 0.67
      ir_camera: /dev/video2
      ir_warmup_delay_ms: 250
"#,
        )
        .unwrap();

        assert!(migrate_config_at_path(&path).unwrap());
        let config = read_config_from_path(&path).unwrap();

        assert!(config.methods.face.anti_spoofing.rgb.enable);
        // 相对路径会被规范化，所以只检查包含原始文件名
        assert!(
            config
                .methods
                .face
                .anti_spoofing
                .rgb
                .model
                .path
                .ends_with("legacy.onnx"),
            "expected path to end with legacy.onnx, got: {}",
            config.methods.face.anti_spoofing.rgb.model.path
        );
        assert_eq!(config.methods.face.anti_spoofing.rgb.model.threshold, 0.67);
        assert!(config.methods.face.anti_spoofing.ir.enable);
        assert_eq!(
            config.methods.face.anti_spoofing.ir.camera.as_deref(),
            Some("/dev/video2")
        );
        assert_eq!(config.methods.face.anti_spoofing.ir.warmup_delay_ms, 250);
    }

    #[test]
    fn migrate_config_at_path_renames_ai_to_rgb_and_adds_ir_model() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        fs::write(
            &path,
            r#"
methods:
  face:
    anti_spoofing:
      ai:
        enable: false
        retries: 0
        retry_delay_ms: 200
        model:
          path: models/mobilenetv3_antispoof.onnx
          threshold: 0.6
      ir:
        enable: true
        retries: 3
        retry_delay_ms: 200
        camera: /dev/video2
        warmup_delay_ms: 600
"#,
        )
        .unwrap();

        assert!(migrate_config_at_path(&path).unwrap());
        let migrated = fs::read_to_string(&path).unwrap();
        let yaml = serde_yaml::from_str::<Value>(&migrated).unwrap();
        let anti = &yaml["methods"]["face"]["anti_spoofing"];

        assert!(anti.get("ai").is_none());
        assert_eq!(
            anti["rgb"]["model"]["path"],
            Value::String("models/mobilenetv3_antispoof.onnx".to_string())
        );
        assert_eq!(
            anti["ir"]["model"]["path"],
            Value::String("models/mobilenetv3_antispoof.onnx".to_string())
        );
        assert_eq!(anti["ir"]["retries"], Value::from(3));
        assert_eq!(anti["ir"]["warmup_delay_ms"], Value::from(600));

        let config = read_config_from_path(&path).unwrap();
        assert!(!config.methods.face.anti_spoofing.rgb.enable);
        assert!(config.methods.face.anti_spoofing.ir.enable);
    }

    #[test]
    fn migrate_config_at_path_leaves_current_schema_untouched() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        let yaml = r#"
methods:
  face:
    anti_spoofing:
      rgb:
        enable: true
        model:
          path: current.onnx
          threshold: 0.7
      ir:
        enable: true
        camera: /dev/video4
        warmup_delay_ms: 400
        model:
          path: current-ir.onnx
          threshold: 0.8
"#;
        fs::write(&path, yaml).unwrap();

        assert!(!migrate_config_at_path(&path).unwrap());

        assert_eq!(fs::read_to_string(&path).unwrap(), yaml);
    }

    #[test]
    fn antispoofing_subchecks_have_independent_retry_defaults() {
        let config = serde_yaml::from_str::<BiopassConfig>("").unwrap();
        assert_eq!(config.methods.face.anti_spoofing.rgb.retries, 0);
        assert_eq!(config.methods.face.anti_spoofing.rgb.retry_delay_ms, 200);
        assert_eq!(config.methods.face.anti_spoofing.ir.retries, 0);
        assert_eq!(config.methods.face.anti_spoofing.ir.retry_delay_ms, 200);
    }

    #[test]
    fn antispoofing_subchecks_retry_config_is_per_subcheck() {
        let yaml = r#"
methods:
  face:
    anti_spoofing:
      rgb:
        enable: true
        retries: 2
        retry_delay_ms: 350
        model:
          path: /test/model.onnx
          threshold: 0.9
      ir:
        enable: true
        retries: 5
        retry_delay_ms: 750
        camera: /dev/video2
        model:
          path: /test/model.onnx
          threshold: 0.9
"#;

        let config = serde_yaml::from_str::<BiopassConfig>(yaml).unwrap();
        let anti = &config.methods.face.anti_spoofing;
        assert_eq!(anti.rgb.retries, 2);
        assert_eq!(anti.rgb.retry_delay_ms, 350);
        assert_eq!(anti.ir.retries, 5);
        assert_eq!(anti.ir.retry_delay_ms, 750);
    }

    #[test]
    fn filters_supported_face_images() {
        assert!(is_supported_face_image(Path::new("a.JPG")));
        assert!(is_supported_face_image(Path::new("a.jpeg")));
        assert!(!is_supported_face_image(Path::new("a.txt")));
    }

    #[test]
    fn unknown_user_does_not_exist() {
        assert!(!user_exists("__biopass_rs_missing_user_for_test__"));
    }
}
