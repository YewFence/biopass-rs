use super::schema::{read_antispoofing_model, AntiSpoofingModelConfig};
use super::serde_defaults::{
    default_antispoofing_retry_delay, default_ir_min_face_area_ratio, default_ir_warmup_delay,
};
use serde_yaml::{Mapping, Value};
use std::fs;
use std::io;
use std::path::Path;

/// Extract a boolean field from a YAML mapping.
fn extract_bool(map: &Mapping, key: &str) -> Option<bool> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_bool)
}

/// Extract a string field from a YAML mapping.
fn extract_string(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Extract an i64 field from a YAML mapping.
fn extract_i64(map: &Mapping, key: &str) -> Option<i64> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_i64)
}

/// Extract a u64 field from a YAML mapping.
fn extract_u64(map: &Mapping, key: &str) -> Option<u64> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_u64)
}

/// Extract an f64 field from a YAML mapping.
fn extract_f64(map: &Mapping, key: &str) -> Option<f64> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_f64)
}

/// Extract a nested mapping from a YAML mapping.
fn extract_mapping<'a>(map: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_mapping)
}

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

pub(super) fn migrated_antispoofing(face: &mut Mapping) -> (Value, bool) {
    let anti = extract_mapping(face, "anti_spoofing");

    let mut enable = false;
    let mut model = AntiSpoofingModelConfig::default();
    let mut ir_model = AntiSpoofingModelConfig::default();
    let mut has_ir_model_value = false;
    let mut ir_camera_path = None;
    let mut warmup_delay = default_ir_warmup_delay();
    let mut min_face_area_ratio = default_ir_min_face_area_ratio();
    let mut ai_retries = 0;
    let mut ai_retry_delay_ms = default_antispoofing_retry_delay();
    let mut ir_enable = false;
    let mut ir_retries = 0;
    let mut ir_retry_delay_ms = default_antispoofing_retry_delay();

    if let Some(anti) = anti {
        if let Some(value) = extract_bool(anti, "enable") {
            enable = value;
        }
        if let Some(value) = anti.get(Value::String("model".to_string())) {
            read_antispoofing_model(value, &mut model);
        }
        if let Some(value) = extract_f64(anti, "threshold") {
            model.threshold = value as f32;
        }
        if let Some(value) = extract_string(anti, "ir_camera") {
            ir_camera_path = Some(value.clone());
            ir_enable = !value.is_empty();
        }
        if let Some(value) = extract_i64(anti, "ir_warmup_delay_ms") {
            warmup_delay = value as i32;
        }
        if let Some(rgb) = extract_mapping(anti, "rgb") {
            if let Some(value) = extract_bool(rgb, "enable") {
                enable = value;
            }
            if let Some(value) = rgb.get(Value::String("model".to_string())) {
                read_antispoofing_model(value, &mut model);
            }
            if let Some(value) = extract_f64(rgb, "threshold") {
                model.threshold = value as f32;
            }
            if let Some(value) = extract_u64(rgb, "retries") {
                ai_retries = value as u32;
            }
            if let Some(value) = extract_u64(rgb, "retry_delay_ms") {
                ai_retry_delay_ms = value as u32;
            }
        } else if let Some(ai) = extract_mapping(anti, "ai") {
            if let Some(value) = extract_bool(ai, "enable") {
                enable = value;
            }
            if let Some(value) = ai.get(Value::String("model".to_string())) {
                read_antispoofing_model(value, &mut model);
            }
            if let Some(value) = extract_f64(ai, "threshold") {
                model.threshold = value as f32;
            }
            if let Some(value) = extract_u64(ai, "retries") {
                ai_retries = value as u32;
            }
            if let Some(value) = extract_u64(ai, "retry_delay_ms") {
                ai_retry_delay_ms = value as u32;
            }
        }
        if let Some(ir) = extract_mapping(anti, "ir") {
            if let Some(value) = extract_bool(ir, "enable") {
                ir_enable = value;
            }
            if let Some(value) = extract_string(ir, "camera") {
                ir_camera_path = Some(value);
            }
            if let Some(value) = extract_i64(ir, "warmup_delay_ms") {
                warmup_delay = value as i32;
            }
            if let Some(value) = extract_f64(ir, "min_face_area_ratio") {
                min_face_area_ratio = value as f32;
            }
            if let Some(value) = ir.get(Value::String("model".to_string())) {
                read_antispoofing_model(value, &mut ir_model);
                has_ir_model_value = true;
            }
            if let Some(value) = extract_u64(ir, "retries") {
                ir_retries = value as u32;
            }
            if let Some(value) = extract_u64(ir, "retry_delay_ms") {
                ir_retry_delay_ms = value as u32;
            }
        }
    }

    if ir_camera_path.as_deref().unwrap_or_default().is_empty() {
        if let Some(legacy_ir) = extract_mapping(face, "ir_camera") {
            let enabled = extract_bool(legacy_ir, "enable").unwrap_or(false);
            let device_id = extract_i64(legacy_ir, "device_id").unwrap_or(0);
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
    let has_new_rgb = anti.is_some_and(|anti| anti.contains_key(Value::String("rgb".to_string())));
    let has_new_ir = anti.is_some_and(|anti| anti.contains_key(Value::String("ir".to_string())));
    let has_new_ir_model = anti
        .and_then(|anti| anti.get(Value::String("ir".to_string())))
        .and_then(Value::as_mapping)
        .is_some_and(|ir| ir.contains_key(Value::String("model".to_string())));
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
        || has_new_ai
        || !has_new_rgb
        || !has_new_ir
        || !has_new_ir_model;

    if !has_ir_model_value {
        ir_model = model.clone();
    }

    let mut model_value = Mapping::new();
    model_value.insert(Value::String("path".to_string()), Value::String(model.path));
    model_value.insert(
        Value::String("threshold".to_string()),
        Value::from(model.threshold),
    );

    let mut rgb_value = Mapping::new();
    rgb_value.insert(Value::String("enable".to_string()), Value::Bool(enable));
    rgb_value.insert(
        Value::String("retries".to_string()),
        Value::from(ai_retries),
    );
    rgb_value.insert(
        Value::String("retry_delay_ms".to_string()),
        Value::from(ai_retry_delay_ms),
    );
    rgb_value.insert(
        Value::String("model".to_string()),
        Value::Mapping(model_value.clone()),
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
    let mut ir_model_value = Mapping::new();
    ir_model_value.insert(
        Value::String("path".to_string()),
        Value::String(ir_model.path),
    );
    ir_model_value.insert(
        Value::String("threshold".to_string()),
        Value::from(ir_model.threshold),
    );

    ir_value.insert(
        Value::String("model".to_string()),
        Value::Mapping(ir_model_value),
    );
    ir_value.insert(
        Value::String("min_face_area_ratio".to_string()),
        Value::from(min_face_area_ratio),
    );

    let mut anti_value = Mapping::new();
    anti_value.insert(Value::String("rgb".to_string()), Value::Mapping(rgb_value));
    anti_value.insert(Value::String("ir".to_string()), Value::Mapping(ir_value));

    (Value::Mapping(anti_value), needs_migration)
}
