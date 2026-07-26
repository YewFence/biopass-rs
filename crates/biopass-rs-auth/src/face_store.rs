use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn faces_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("faces")
}

pub fn save_enrolled_face_jpeg(data_dir: &Path, jpeg: &[u8]) -> Result<PathBuf, String> {
    let dir = faces_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "Failed to create faces directory {}: {error}",
            dir.display()
        )
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to get timestamp: {error}"))?
        .as_millis();
    let path = dir.join(format!("face_{timestamp}.jpg"));
    std::fs::write(&path, jpeg)
        .map_err(|error| format!("Failed to write face image {}: {error}", path.display()))?;
    Ok(path)
}

pub fn list_enrolled_faces(data_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = faces_dir(data_dir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };

    let mut faces = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read faces directory {}: {error}", dir.display()))?
        .into_iter()
        .filter(|path| is_supported_face_image(path))
        .collect::<Vec<_>>();
    faces.sort();
    Ok(faces)
}

pub fn delete_enrolled_face(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path)
        .map_err(|error| format!("Failed to delete face image {}: {error}", path.display()))
}

fn is_supported_face_image(path: &Path) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_enrolled_faces_returns_sorted_supported_images() {
        let directory = tempfile::tempdir().unwrap();
        let faces = faces_dir(directory.path());
        std::fs::create_dir_all(&faces).unwrap();
        std::fs::write(faces.join("b.png"), b"b").unwrap();
        std::fs::write(faces.join("a.jpg"), b"a").unwrap();
        std::fs::write(faces.join("ignored.txt"), b"x").unwrap();

        let listed = list_enrolled_faces(directory.path()).unwrap();

        assert_eq!(
            listed
                .iter()
                .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            ["a.jpg", "b.png"]
        );
    }

    #[test]
    fn save_enrolled_face_jpeg_creates_faces_dir() {
        let directory = tempfile::tempdir().unwrap();

        let path = save_enrolled_face_jpeg(directory.path(), b"jpeg").unwrap();

        assert_eq!(path.parent(), Some(faces_dir(directory.path()).as_path()));
        assert_eq!(std::fs::read(path).unwrap(), b"jpeg");
    }

    #[test]
    fn delete_enrolled_face_removes_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("face.jpg");
        std::fs::write(&path, b"jpeg").unwrap();

        delete_enrolled_face(&path).unwrap();

        assert!(!path.exists());
    }
}
