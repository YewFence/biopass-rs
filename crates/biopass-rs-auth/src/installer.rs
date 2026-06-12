use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const MODELS: &[(&str, &str)] = &[
    (
        "yolov8n-face.onnx",
        "https://biopass.ticklab.site/models/yolov8n-face.onnx",
    ),
    (
        "edgeface_s_gamma_05.onnx",
        "https://biopass.ticklab.site/models/edgeface_s_gamma_05.onnx",
    ),
    (
        "edgeface_xs_gamma_06.onnx",
        "https://biopass.ticklab.site/models/edgeface_xs_gamma_06.onnx",
    ),
    (
        "mobilenetv3_antispoof.onnx",
        "https://biopass.ticklab.site/models/mobilenetv3_antispoof.onnx",
    ),
];

const LEGACY_MODELS: &[&str] = &[
    "yolov11n-face.torchscript",
    "edgeface_s_gamma_05_ts.pt",
    "mobilenetv3_antispoof_ts.pt",
];

const OLD_CONFIG_PATH: &str = ".config/com.ticklab.biopass/config.yaml";
const NEW_CONFIG_PATH: &str = ".config/biopass-rs/config.yaml";
const OLD_DATA_DIR: &str = ".local/share/com.ticklab.biopass";
const NEW_DATA_DIR: &str = ".local/share/biopass-rs";

fn get_home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .or_else(|_| {
            std::env::var("SUDO_USER").and_then(|user| {
                Command::new("getent")
                    .args(["passwd", &user])
                    .output()
                    .ok()
                    .and_then(|out| String::from_utf8(out.stdout).ok())
                    .and_then(|s| s.split(':').nth(5).map(String::from))
                    .ok_or_else(|| std::env::VarError::NotPresent)
            })
        })
        .map(PathBuf::from)
        .map_err(|_| "Cannot determine home directory".to_string())
}

fn download_file(
    url: &str,
    dest: &Path,
    retries: u32,
    progress: Option<&ProgressBar>,
) -> Result<(), String> {
    for attempt in 1..=retries {
        match try_download(url, dest, progress) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < retries => {
                let msg = format!(
                    "Download attempt {}/{} failed: {}. Retrying...",
                    attempt, retries, e
                );
                if let Some(pb) = progress {
                    pb.set_message(msg);
                } else {
                    eprintln!("{}", msg);
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            Err(e) => return Err(e),
        }
    }
    Err("Max retries exceeded".to_string())
}

fn try_download(url: &str, dest: &Path, progress: Option<&ProgressBar>) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let total_size = response
        .headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    if let Some(pb) = progress {
        if let Some(size) = total_size {
            pb.set_length(size);
        }
    }

    let bytes = response
        .into_body()
        .read_to_vec()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if let Some(pb) = progress {
        pb.set_position(bytes.len() as u64);
    }

    let mut file = fs::File::create(dest).map_err(|e| format!("Failed to create file: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    file.flush()
        .map_err(|e| format!("Failed to flush file: {}", e))?;
    Ok(())
}

pub fn download_models() -> Result<(), String> {
    let home = get_home_dir()?;
    let data_dir = home.join(NEW_DATA_DIR).join("models");

    remove_legacy_models(&data_dir);

    fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;

    let multi = MultiProgress::new();

    let style = ProgressStyle::default_bar()
        .template("{msg:30.bold} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-");

    for (filename, url) in MODELS {
        let dest = data_dir.join(filename);
        if dest.exists() {
            eprintln!("[skip] Model already present: {}", filename);
            continue;
        }

        let pb = multi.add(ProgressBar::new(0));
        pb.set_style(style.clone());
        pb.set_message(filename.to_string());

        download_file(url, &dest, 3, Some(&pb))?;
        pb.finish_with_message(format!("[done] {}", filename));
    }

    multi.clear().map_err(|e| e.to_string())?;
    Ok(())
}

fn remove_legacy_models(data_dir: &Path) {
    for filename in LEGACY_MODELS {
        let path = data_dir.join(filename);
        if path.exists() {
            eprintln!("Removing legacy model: {}", filename);
            let _ = fs::remove_file(path);
        }
    }
}

pub fn run_ldconfig() -> Result<(), String> {
    Command::new("ldconfig")
        .status()
        .map_err(|e| format!("Failed to run ldconfig: {}", e))?;
    Ok(())
}

pub fn migrate_all_users() -> Result<(), String> {
    let passwd = fs::read_to_string("/etc/passwd")
        .map_err(|e| format!("Failed to read /etc/passwd: {}", e))?;

    for line in passwd.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 6 {
            continue;
        }
        let username = parts[0];
        let home = parts[5];

        if home.is_empty() || home == "/nonexistent" {
            continue;
        }

        let home_path = PathBuf::from(home);
        let old_config = home_path.join(OLD_CONFIG_PATH);
        let new_config = home_path.join(NEW_CONFIG_PATH);

        // Migrate old config to new location
        if old_config.exists() && !new_config.exists() {
            eprintln!(
                "Migrating config from old location for user '{}'...",
                username
            );
            if let Some(parent) = new_config.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::rename(&old_config, &new_config) {
                eprintln!(
                    "Warning: Failed to migrate config for '{}': {}",
                    username, e
                );
                // Try copy instead
                let _ = fs::copy(&old_config, &new_config);
            }
        }

        // Migrate old data dir to new location
        let old_data = home_path.join(OLD_DATA_DIR);
        let new_data = home_path.join(NEW_DATA_DIR);
        if old_data.exists() && !new_data.exists() {
            eprintln!("Migrating data directory for user '{}'...", username);
            if let Err(e) = fs::rename(&old_data, &new_data) {
                eprintln!(
                    "Warning: Failed to migrate data dir for '{}': {}",
                    username, e
                );
            }
        }

        // Migrate config schema (handles anti_spoofing structure changes)
        if new_config.exists() {
            eprintln!("Migrating config schema for user '{}'...", username);
            let _ = crate::migrate_config_schema(username);
        }
    }

    Ok(())
}

/// Check if all required models are present on disk
pub fn check_models_present() -> bool {
    let Ok(home) = get_home_dir() else {
        return false;
    };
    let data_dir = home.join(NEW_DATA_DIR).join("models");
    MODELS
        .iter()
        .all(|(filename, _)| data_dir.join(filename).exists())
}
