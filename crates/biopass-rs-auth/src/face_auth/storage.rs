use crate::{
    decode_jpeg_rgb, emit_log, encode_jpeg, save_failed_frames_enabled, user_data_dir,
    LogComponent, LogLevel, RgbFrame,
};
use std::path::{Path, PathBuf};

pub(super) fn save_debug_frame_if_enabled(
    _debug: bool,
    username: &str,
    frame: &RgbFrame,
    reason: &str,
) {
    if !save_failed_frames_enabled() {
        return;
    }

    match save_debug_frame(username, frame, reason) {
        Ok(path) => emit_log(
            LogComponent::Auth,
            LogLevel::Debug,
            "FaceAuth",
            &format!("saved debug frame to {}", path.display()),
        ),
        Err(error) => emit_log(
            LogComponent::Auth,
            LogLevel::Warn,
            "FaceAuth",
            &format!("failed to save debug frame: {error}"),
        ),
    }
}

pub(super) fn save_debug_frame(
    username: &str,
    frame: &RgbFrame,
    reason: &str,
) -> Result<PathBuf, String> {
    use std::time::SystemTime;

    let timestamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("{}.{}.jpg", reason, timestamp);
    let debug_dir = user_data_dir(username).join("debugs");
    let path = debug_dir.join(filename);

    std::fs::create_dir_all(&debug_dir).map_err(|error| {
        format!(
            "Failed to create debug directory {}: {error}",
            debug_dir.display()
        )
    })?;
    let jpeg = encode_jpeg(frame, 85)?;
    std::fs::write(&path, jpeg)
        .map_err(|error| format!("Failed to write debug frame {}: {error}", path.display()))?;
    Ok(path)
}

pub(super) fn read_enrolled_face(path: &Path) -> Result<RgbFrame, String> {
    let data = std::fs::read(path)
        .map_err(|error| format!("Failed to read enrolled face {}: {error}", path.display()))?;
    decode_jpeg_rgb(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_data_dir_override<T>(
        path: &std::path::Path,
        f: impl FnOnce() -> T + std::panic::UnwindSafe,
    ) -> T {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let previous = std::env::var_os(crate::DATA_DIR_ENV);
        std::env::set_var(crate::DATA_DIR_ENV, path);
        let result = std::panic::catch_unwind(f);

        if let Some(value) = previous {
            std::env::set_var(crate::DATA_DIR_ENV, value);
        } else {
            std::env::remove_var(crate::DATA_DIR_ENV);
        }

        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    #[test]
    fn reads_enrolled_jpeg_faces() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("face.jpg");
        let frame = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();
        std::fs::write(&path, crate::encode_jpeg(&frame, 95).unwrap()).unwrap();

        let loaded = read_enrolled_face(&path).unwrap();

        assert_eq!(loaded.width, 1);
        assert_eq!(loaded.height, 1);
        assert_eq!(loaded.data.len(), 3);
    }

    #[test]
    fn save_debug_frame_creates_debug_directory() {
        let directory = tempfile::tempdir().unwrap();
        with_data_dir_override(directory.path(), || {
            let username = "biopass-rs-missing-user";
            let frame = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();

            let path = save_debug_frame(username, &frame, "test_failure").unwrap();

            assert!(path.is_file());
            assert_eq!(
                path.parent(),
                Some(user_data_dir(username).join("debugs").as_path())
            );
            let data = std::fs::read(path).unwrap();
            assert!(data.starts_with(&[0xff, 0xd8]));
        });
    }
}
