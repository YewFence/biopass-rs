use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Clone)]
pub struct VideoDeviceInfo {
    pub path: String,
    pub name: String,
    pub display_name: String,
}

pub fn biopass_helper_path() -> String {
    const CANDIDATES: &[&str] = &[
        "../../auth/rust/target/release/biopass-helper",
        "../../auth/rust/target/debug/biopass-helper",
        "/usr/bin/biopass-helper",
        "biopass-helper",
    ];

    CANDIDATES
        .iter()
        .find(|path| *path == &"biopass-helper" || Path::new(path).exists())
        .unwrap_or(&"biopass-helper")
        .to_string()
}

#[tauri::command]
pub fn get_current_username() -> Result<String, String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .map_err(|_| "Could not determine current username".to_string())
}

fn video_device_sort_key(path: &str) -> i32 {
    path.trim_start_matches("/dev/video")
        .parse::<i32>()
        .unwrap_or(-1)
}

#[tauri::command]
pub fn list_video_devices() -> Result<Vec<VideoDeviceInfo>, String> {
    let mut devices = Vec::new();
    let entries = fs::read_dir("/dev").map_err(|e| format!("Failed to read /dev: {}", e))?;

    for entry in entries {
        if let Ok(entry) = entry {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("video") {
                continue;
            }

            let path = format!("/dev/{}", file_name);
            let name = fs::read_to_string(format!("/sys/class/video4linux/{}/name", file_name))
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            let display_name = if name.is_empty() {
                path.clone()
            } else {
                format!("{} ({})", name, path)
            };

            devices.push(VideoDeviceInfo {
                path,
                name,
                display_name,
            });
        }
    }

    // Sort naturally video0, video1, video2...
    devices.sort_by(|a, b| video_device_sort_key(&a.path).cmp(&video_device_sort_key(&b.path)));

    Ok(devices)
}
