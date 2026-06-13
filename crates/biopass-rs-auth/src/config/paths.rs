use super::schema::BiopassConfig;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = ".config/biopass-rs/config.yaml";
const DATA_DIR: &str = ".local/share/biopass-rs";

pub fn config_path(username: &str) -> PathBuf {
    match home_dir_for_user(username) {
        Some(home) => home.join(CONFIG_FILE),
        None => std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(CONFIG_FILE))
            .unwrap_or_else(|| PathBuf::from("/etc/biopass-rs/config.yaml")),
    }
}

pub fn config_exists(username: &str) -> bool {
    config_path(username).is_file()
}

pub fn user_exists(username: &str) -> bool {
    home_dir_for_user(username).is_some()
}

pub fn read_config_from_path(config_path: &Path) -> Result<BiopassConfig, String> {
    let config_text = fs::read_to_string(config_path)
        .map_err(|error| format!("Failed to read config {}: {error}", config_path.display()))?;
    serde_yaml::from_str::<BiopassConfig>(&config_text)
        .map_err(|error| format!("Failed to parse config {}: {error}", config_path.display()))
}

pub fn read_config(username: &str) -> Result<BiopassConfig, String> {
    read_config_from_path(&config_path(username))
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

pub fn setup_config(username: &str) -> std::io::Result<()> {
    let data_dir = user_data_dir(username);
    fs::create_dir_all(data_dir.join("faces"))?;
    fs::create_dir_all(data_dir.join("debugs"))?;
    Ok(())
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

pub(super) fn is_supported_face_image(path: &Path) -> bool {
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
