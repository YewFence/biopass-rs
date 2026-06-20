use std::path::Path;

use biopass_rs_auth::{
    capture_face_jpeg, delete_enrolled_face, list_enrolled_faces, save_enrolled_face_jpeg,
};
use tauri::AppHandle;

use crate::config::{require_loaded_config, BiopassConfig};
use crate::paths::get_data_dir;

#[tauri::command]
pub fn capture_face(app: AppHandle, camera: Option<String>) -> Result<String, String> {
    let data_dir = get_data_dir(&app)?;
    let app_config: BiopassConfig = require_loaded_config(&app)?;
    let face_config = app_config.methods.face;
    let jpeg = capture_face_jpeg(
        camera.as_deref(),
        &face_config.detection.model,
        90,
        face_config.auto_optimize_camera,
    )
    .map_err(|error| {
        if error == "No face detected" {
            "No face detected. Please position your face in front of the camera.".to_string()
        } else {
            format!("Capture failed: {error}")
        }
    })?;

    Ok(save_enrolled_face_jpeg(&data_dir, &jpeg)?
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub fn list_faces(app: AppHandle) -> Result<Vec<String>, String> {
    let data_dir = get_data_dir(&app)?;
    Ok(list_enrolled_faces(&data_dir)?
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub fn delete_face(path: String) -> Result<(), String> {
    delete_enrolled_face(Path::new(&path))
}
