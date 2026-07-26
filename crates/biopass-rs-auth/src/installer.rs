use crate::user_data_dir;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use users::os::unix::UserExt;

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

/// Upstream TickLabVN `biopass` data directory, relative to a user's home.
const UPSTREAM_DATA_DIR: &str = ".local/share/com.ticklab.biopass";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLegacyFacesOutcome {
    pub source_found: bool,
    pub copied: usize,
}

/// Resolve the directory where ONNX models live. Honours `BIOPASS_DATA_DIR`
/// and the CLI `--data-dir` override so `download_models` /
/// `check_models_present` agree with `user_data_dir()` for the rest of the
/// crate.
fn models_dir() -> Result<PathBuf, String> {
    let dir = user_data_dir("ignored");
    if dir.as_os_str().is_empty() {
        return Err("Cannot determine data directory".to_string());
    }
    Ok(dir.join("models"))
}

/// HTTP agent that fails fast while establishing the connection or waiting for
/// the server to start responding, but never aborts an in-progress (possibly
/// slow) body download.
///
/// Only the pre-body phases carry a timeout: DNS lookup, TCP+TLS handshake, and
/// waiting for the response headers (i.e. the first byte). The body read is left
/// unbounded on purpose — the ONNX models may legitimately take a long time on a
/// constrained link, and it is better to let a slow download finish than to cut
/// it off and retry into the same link.
fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_resolve(Some(Duration::from_secs(10)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn download_file(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    retries: u32,
    progress: Option<&ProgressBar>,
) -> Result<(), String> {
    for attempt in 1..=retries {
        match try_download(agent, url, dest, progress) {
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

fn try_download(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    progress: Option<&ProgressBar>,
) -> Result<(), String> {
    let response = agent
        .get(url)
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

    let mut reader = response.into_body().into_reader();
    let mut file = fs::File::create(dest).map_err(|e| format!("Failed to create file: {}", e))?;

    let mut buffer = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read response: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])
            .map_err(|e| format!("Failed to write file: {}", e))?;
        downloaded += n as u64;
        if let Some(pb) = progress {
            pb.set_position(downloaded);
        }
    }

    file.flush()
        .map_err(|e| format!("Failed to flush file: {}", e))?;
    Ok(())
}

pub fn download_models() -> Result<(), String> {
    let data_dir = models_dir()?;

    remove_legacy_models(&data_dir);

    fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;

    let multi = MultiProgress::new();
    let agent = http_agent();

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

        download_file(&agent, url, &dest, 3, Some(&pb))?;
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

/// Copy enrolled face images from an upstream `biopass` install for `username`.
///
/// The upstream config is intentionally ignored because its schema drifts
/// independently. Face images are plain files, so importing them is stable
/// across upstream versions.
pub fn import_legacy_faces_for_user(username: &str) -> Result<ImportLegacyFacesOutcome, String> {
    let Some(home) = users::get_user_by_name(username).map(|user| user.home_dir().to_path_buf())
    else {
        return Ok(ImportLegacyFacesOutcome {
            source_found: false,
            copied: 0,
        });
    };
    let src_faces = home.join(UPSTREAM_DATA_DIR).join("faces");
    let dest_faces = user_data_dir(username).join("faces");
    import_legacy_faces_from(&src_faces, &dest_faces)
}

/// Copy enrolled face images from `src_faces` into `dest_faces` without
/// overwriting existing destination files.
pub fn import_legacy_faces_from(
    src_faces: &Path,
    dest_faces: &Path,
) -> Result<ImportLegacyFacesOutcome, String> {
    let Ok(entries) = fs::read_dir(src_faces) else {
        return Ok(ImportLegacyFacesOutcome {
            source_found: false,
            copied: 0,
        });
    };

    fs::create_dir_all(dest_faces)
        .map_err(|error| format!("Failed to create {}: {error}", dest_faces.display()))?;

    let mut copied = 0usize;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let dest = dest_faces.join(name);
        if dest.exists() {
            continue;
        }
        if fs::copy(entry.path(), &dest).is_ok() {
            copied += 1;
        }
    }

    Ok(ImportLegacyFacesOutcome {
        source_found: true,
        copied,
    })
}

/// Check if all required models are present on disk
pub fn check_models_present() -> bool {
    let Ok(data_dir) = models_dir() else {
        return false;
    };
    models_present_in_dir(&data_dir)
}

fn models_present_in_dir(data_dir: &Path) -> bool {
    MODELS
        .iter()
        .all(|(filename, _)| data_dir.join(filename).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_present_in_dir_requires_every_current_model() {
        let directory = tempfile::tempdir().unwrap();

        for (filename, _) in MODELS.iter().take(MODELS.len() - 1) {
            fs::write(directory.path().join(filename), b"model").unwrap();
        }

        assert!(!models_present_in_dir(directory.path()));

        let (filename, _) = MODELS.last().unwrap();
        fs::write(directory.path().join(filename), b"model").unwrap();

        assert!(models_present_in_dir(directory.path()));
    }

    #[test]
    fn remove_legacy_models_keeps_current_models() {
        let directory = tempfile::tempdir().unwrap();
        for filename in LEGACY_MODELS {
            fs::write(directory.path().join(filename), b"legacy").unwrap();
        }
        let (current, _) = MODELS[0];
        fs::write(directory.path().join(current), b"current").unwrap();

        remove_legacy_models(directory.path());

        for filename in LEGACY_MODELS {
            assert!(!directory.path().join(filename).exists());
        }
        assert!(directory.path().join(current).exists());
    }

    #[test]
    fn import_legacy_faces_from_copies_without_overwriting() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        fs::write(src.path().join("alice.jpg"), b"new").unwrap();
        fs::write(src.path().join("bob.jpg"), b"bob").unwrap();
        fs::write(dest.path().join("alice.jpg"), b"existing").unwrap();

        let outcome = import_legacy_faces_from(src.path(), dest.path()).unwrap();

        assert_eq!(
            outcome,
            ImportLegacyFacesOutcome {
                source_found: true,
                copied: 1,
            }
        );
        assert_eq!(
            fs::read(dest.path().join("alice.jpg")).unwrap(),
            b"existing"
        );
        assert_eq!(fs::read(dest.path().join("bob.jpg")).unwrap(), b"bob");
    }

    #[test]
    fn import_legacy_faces_from_ignores_missing_source() {
        let dest = tempfile::tempdir().unwrap();

        let outcome = import_legacy_faces_from(Path::new("/missing/source"), dest.path()).unwrap();

        assert_eq!(
            outcome,
            ImportLegacyFacesOutcome {
                source_found: false,
                copied: 0,
            }
        );
    }
}
