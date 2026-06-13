use super::paths::config_path;
use super::schema::{read_antispoofing_model, AntiSpoofingModelConfig};
use super::serde_defaults::{default_antispoofing_retry_delay, default_ir_warmup_delay};
use serde_yaml::{Mapping, Value};
use std::fs;
use std::io;
use std::path::Path;

pub fn migrate_config_at_path(path: &Path) -> io::Result<bool> {
    let Ok(config_text) = fs::read_to_string(path) else {
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

pub fn migrate_config_schema(username: &str) -> io::Result<bool> {
    migrate_config_at_path(&config_path(username))
}

pub(super) fn migrated_antispoofing(face: &mut Mapping) -> (Value, bool) {
    let anti = face
        .get(Value::String("anti_spoofing".to_string()))
        .and_then(Value::as_mapping);

    let mut enable = false;
    let mut model = AntiSpoofingModelConfig::default();
    let mut ir_camera_path = None;
    let mut warmup_delay = default_ir_warmup_delay();
    let mut ai_retries = 0;
    let mut ai_retry_delay_ms = default_antispoofing_retry_delay();
    let mut ir_enable = false;
    let mut ir_retries = 0;
    let mut ir_retry_delay_ms = default_antispoofing_retry_delay();

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
            ir_enable = !value.is_empty();
        }
        if let Some(value) = anti
            .get(Value::String("ir_warmup_delay_ms".to_string()))
            .and_then(Value::as_i64)
        {
            warmup_delay = value as i32;
        }
        if let Some(ai) = anti
            .get(Value::String("ai".to_string()))
            .and_then(Value::as_mapping)
        {
            if let Some(value) = ai
                .get(Value::String("enable".to_string()))
                .and_then(Value::as_bool)
            {
                enable = value;
            }
            if let Some(value) = ai.get(Value::String("model".to_string())) {
                read_antispoofing_model(value, &mut model);
            }
            if let Some(value) = ai
                .get(Value::String("threshold".to_string()))
                .and_then(Value::as_f64)
            {
                model.threshold = value as f32;
            }
            if let Some(value) = ai
                .get(Value::String("retries".to_string()))
                .and_then(Value::as_u64)
            {
                ai_retries = value as u32;
            }
            if let Some(value) = ai
                .get(Value::String("retry_delay_ms".to_string()))
                .and_then(Value::as_u64)
            {
                ai_retry_delay_ms = value as u32;
            }
        }
        if let Some(ir) = anti
            .get(Value::String("ir".to_string()))
            .and_then(Value::as_mapping)
        {
            if let Some(value) = ir
                .get(Value::String("enable".to_string()))
                .and_then(Value::as_bool)
            {
                ir_enable = value;
            }
            if let Some(value) = ir
                .get(Value::String("camera".to_string()))
                .and_then(Value::as_str)
            {
                ir_camera_path = Some(value.to_string());
            }
            if let Some(value) = ir
                .get(Value::String("warmup_delay_ms".to_string()))
                .and_then(Value::as_i64)
            {
                warmup_delay = value as i32;
            }
            if let Some(value) = ir
                .get(Value::String("retries".to_string()))
                .and_then(Value::as_u64)
            {
                ir_retries = value as u32;
            }
            if let Some(value) = ir
                .get(Value::String("retry_delay_ms".to_string()))
                .and_then(Value::as_u64)
            {
                ir_retry_delay_ms = value as u32;
            }
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
                ir_enable = true;
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
    let has_new_ai = anti.is_some_and(|anti| anti.contains_key(Value::String("ai".to_string())));
    let has_new_ir = anti.is_some_and(|anti| anti.contains_key(Value::String("ir".to_string())));
    let has_legacy_ir_key =
        anti.is_some_and(|anti| anti.contains_key(Value::String("ir_camera".to_string())));
    let has_legacy_ir_warmup =
        anti.is_some_and(|anti| anti.contains_key(Value::String("ir_warmup_delay_ms".to_string())));
    let needs_migration = has_legacy_face_ir
        || has_legacy_anti_threshold
        || has_legacy_anti_model_scalar
        || has_new_model_map
        || has_legacy_ir_key
        || has_legacy_ir_warmup
        || !has_new_ai
        || !has_new_ir;

    let mut model_value = Mapping::new();
    model_value.insert(Value::String("path".to_string()), Value::String(model.path));
    model_value.insert(
        Value::String("threshold".to_string()),
        Value::from(model.threshold),
    );

    let mut ai_value = Mapping::new();
    ai_value.insert(Value::String("enable".to_string()), Value::Bool(enable));
    ai_value.insert(
        Value::String("retries".to_string()),
        Value::from(ai_retries),
    );
    ai_value.insert(
        Value::String("retry_delay_ms".to_string()),
        Value::from(ai_retry_delay_ms),
    );
    ai_value.insert(
        Value::String("model".to_string()),
        Value::Mapping(model_value),
    );

    let mut ir_value = Mapping::new();
    ir_value.insert(Value::String("enable".to_string()), Value::Bool(ir_enable));
    ir_value.insert(
        Value::String("retries".to_string()),
        Value::from(ir_retries),
    );
    ir_value.insert(
        Value::String("retry_delay_ms".to_string()),
        Value::from(ir_retry_delay_ms),
    );
    ir_value.insert(
        Value::String("camera".to_string()),
        ir_camera_path.map(Value::String).unwrap_or(Value::Null),
    );
    ir_value.insert(
        Value::String("warmup_delay_ms".to_string()),
        Value::from(warmup_delay),
    );

    let mut anti_value = Mapping::new();
    anti_value.insert(Value::String("ai".to_string()), Value::Mapping(ai_value));
    anti_value.insert(Value::String("ir".to_string()), Value::Mapping(ir_value));

    (Value::Mapping(anti_value), needs_migration)
}
