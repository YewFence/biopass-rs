use crate::user_data_dir;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use users::os::unix::UserExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModelSpec {
    filename: &'static str,
    url: &'static str,
    model_type: &'static str,
}

const MODELS: &[ModelSpec] = &[
    ModelSpec {
        filename: "yolov8n-face.onnx",
        url: "https://biopass.ticklab.site/models/yolov8n-face.onnx",
        model_type: "detection",
    },
    ModelSpec {
        filename: "edgeface_s_gamma_05.onnx",
        url: "https://biopass.ticklab.site/models/edgeface_s_gamma_05.onnx",
        model_type: "recognition",
    },
    ModelSpec {
        filename: "edgeface_xs_gamma_06.onnx",
        url: "https://biopass.ticklab.site/models/edgeface_xs_gamma_06.onnx",
        model_type: "recognition",
    },
    ModelSpec {
        filename: "mobilenetv3_antispoof.onnx",
        url: "https://biopass.ticklab.site/models/mobilenetv3_antispoof.onnx",
        model_type: "anti-spoofing",
    },
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BuiltinModelInfo {
    pub filename: String,
    pub path: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub url: String,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelDownloadReport {
    pub models: Vec<BuiltinModelInfo>,
    pub downloaded: usize,
    pub skipped: usize,
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

pub fn builtin_models() -> Result<Vec<BuiltinModelInfo>, String> {
    let data_dir = models_dir()?;
    Ok(model_infos_in_dir(&data_dir))
}

fn model_infos_in_dir(data_dir: &Path) -> Vec<BuiltinModelInfo> {
    MODELS
        .iter()
        .map(|spec| {
            let path = data_dir.join(spec.filename);
            BuiltinModelInfo {
                filename: spec.filename.to_string(),
                path: path.to_string_lossy().to_string(),
                model_type: spec.model_type.to_string(),
                url: spec.url.to_string(),
                present: path.exists(),
            }
        })
        .collect()
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
    download_models_inner(true).map(|_| ())
}

pub fn download_models_report() -> Result<ModelDownloadReport, String> {
    download_models_inner(false)
}

fn download_models_inner(show_progress: bool) -> Result<ModelDownloadReport, String> {
    let data_dir = models_dir()?;

    remove_legacy_models(&data_dir);

    fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;

    let multi = MultiProgress::new();
    let agent = http_agent();
    let mut downloaded = 0usize;
    let mut skipped = 0usize;

    let style = ProgressStyle::default_bar()
        .template("{msg:30.bold} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-");

    for spec in MODELS {
        let dest = data_dir.join(spec.filename);
        if dest.exists() {
            skipped += 1;
            if show_progress {
                eprintln!("[skip] Model already present: {}", spec.filename);
            }
            continue;
        }

        let pb = if show_progress {
            let pb = multi.add(ProgressBar::new(0));
            pb.set_style(style.clone());
            pb.set_message(spec.filename.to_string());
            Some(pb)
        } else {
            None
        };

        download_file(&agent, spec.url, &dest, 3, pb.as_ref())?;
        downloaded += 1;
        if let Some(pb) = pb {
            pb.finish_with_message(format!("[done] {}", spec.filename));
        }
    }

    if show_progress {
        multi.clear().map_err(|e| e.to_string())?;
    }

    Ok(ModelDownloadReport {
        models: model_infos_in_dir(&data_dir),
        downloaded,
        skipped,
    })
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
        .all(|spec| data_dir.join(spec.filename).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_present_in_dir_requires_every_current_model() {
        let directory = tempfile::tempdir().unwrap();

        for spec in MODELS.iter().take(MODELS.len() - 1) {
            fs::write(directory.path().join(spec.filename), b"model").unwrap();
        }

        assert!(!models_present_in_dir(directory.path()));

        let spec = MODELS.last().unwrap();
        fs::write(directory.path().join(spec.filename), b"model").unwrap();

        assert!(models_present_in_dir(directory.path()));
    }

    #[test]
    fn remove_legacy_models_keeps_current_models() {
        let directory = tempfile::tempdir().unwrap();
        for filename in LEGACY_MODELS {
            fs::write(directory.path().join(filename), b"legacy").unwrap();
        }
        let current = MODELS[0].filename;
        fs::write(directory.path().join(current), b"current").unwrap();

        remove_legacy_models(directory.path());

        for filename in LEGACY_MODELS {
            assert!(!directory.path().join(filename).exists());
        }
        assert!(directory.path().join(current).exists());
    }

    #[test]
    fn model_infos_include_paths_and_presence() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join(MODELS[0].filename), b"model").unwrap();

        let infos = model_infos_in_dir(directory.path());

        assert_eq!(infos.len(), MODELS.len());
        assert_eq!(infos[0].filename, MODELS[0].filename);
        assert!(infos[0].present);
        assert!(!infos[1].present);
        assert!(infos[0].path.ends_with(MODELS[0].filename));
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
    fn import_legacy_faces_from_creates_destination_directory() {
        let src = tempfile::tempdir().unwrap();
        let dest_parent = tempfile::tempdir().unwrap();
        let dest = dest_parent.path().join("faces");
        fs::write(src.path().join("alice.jpg"), b"alice").unwrap();

        let outcome = import_legacy_faces_from(src.path(), &dest).unwrap();

        assert_eq!(
            outcome,
            ImportLegacyFacesOutcome {
                source_found: true,
                copied: 1,
            }
        );
        assert_eq!(fs::read(dest.join("alice.jpg")).unwrap(), b"alice");
    }

    #[test]
    fn import_legacy_faces_from_ignores_unreadable_file_names() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let invalid_name = OsString::from_vec(vec![0xff, b'.', b'j', b'p', b'g']);
        fs::write(src.path().join(invalid_name), b"invalid").unwrap();

        let outcome = import_legacy_faces_from(src.path(), dest.path()).unwrap();

        assert_eq!(
            outcome,
            ImportLegacyFacesOutcome {
                source_found: true,
                copied: 0,
            }
        );
        assert_eq!(fs::read_dir(dest.path()).unwrap().count(), 0);
    }

    #[test]
    fn import_legacy_faces_from_skips_entries_that_cannot_be_copied() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        fs::create_dir(src.path().join("not-a-file.jpg")).unwrap();
        fs::write(src.path().join("alice.jpg"), b"alice").unwrap();

        let outcome = import_legacy_faces_from(src.path(), dest.path()).unwrap();

        assert_eq!(
            outcome,
            ImportLegacyFacesOutcome {
                source_found: true,
                copied: 1,
            }
        );
        assert_eq!(fs::read(dest.path().join("alice.jpg")).unwrap(), b"alice");
        assert!(!dest.path().join("not-a-file.jpg").exists());
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

    #[test]
    fn import_legacy_faces_for_user_ignores_unknown_user() {
        let outcome = import_legacy_faces_for_user("__biopass_rs_missing_user__").unwrap();

        assert_eq!(
            outcome,
            ImportLegacyFacesOutcome {
                source_found: false,
                copied: 0,
            }
        );
    }
}
