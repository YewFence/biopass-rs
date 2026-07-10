use std::fs::OpenOptions;
use std::io::Write;
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
    for suffix in 0..1000 {
        let name = if suffix == 0 {
            format!("face_{timestamp}.jpg")
        } else {
            format!("face_{timestamp}_{suffix}.jpg")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(jpeg).map_err(|error| {
                    format!("Failed to write face image {}: {error}", path.display())
                })?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Failed to create face image {}: {error}",
                    path.display()
                ));
            }
        }
    }

    Err("Failed to allocate a unique face image filename".to_string())
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

pub fn delete_enrolled_face(data_dir: &Path, path: &Path) -> Result<(), String> {
    let faces = faces_dir(data_dir)
        .canonicalize()
        .map_err(|error| format!("Failed to access faces directory: {error}"))?;
    let path = path
        .canonicalize()
        .map_err(|error| format!("Failed to access face image {}: {error}", path.display()))?;
    if !path.starts_with(&faces) {
        return Err(format!(
            "Refusing to delete file outside {}",
            faces.display()
        ));
    }
    std::fs::remove_file(&path)
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
    fn list_enrolled_faces_accepts_supported_extensions_case_insensitively() {
        let directory = tempfile::tempdir().unwrap();
        let faces = faces_dir(directory.path());
        std::fs::create_dir_all(&faces).unwrap();
        for name in ["a.JPG", "b.JPEG", "c.PNG", "d.BMP", "e.TGA"] {
            std::fs::write(faces.join(name), b"face").unwrap();
        }
        std::fs::write(faces.join("ignored.gif"), b"gif").unwrap();
        std::fs::write(faces.join("no_extension"), b"none").unwrap();

        let listed = list_enrolled_faces(directory.path()).unwrap();

        assert_eq!(listed.len(), 5);
        assert!(listed.iter().all(|path| is_supported_face_image(path)));
    }

    #[test]
    fn list_enrolled_faces_returns_empty_for_missing_directory() {
        let directory = tempfile::tempdir().unwrap();

        let listed = list_enrolled_faces(directory.path()).unwrap();

        assert!(listed.is_empty());
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
        let faces = faces_dir(directory.path());
        std::fs::create_dir_all(&faces).unwrap();
        let path = faces.join("face.jpg");
        std::fs::write(&path, b"jpeg").unwrap();

        delete_enrolled_face(directory.path(), &path).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn delete_enrolled_face_reports_missing_file() {
        let directory = tempfile::tempdir().unwrap();
        let faces = faces_dir(directory.path());
        std::fs::create_dir_all(&faces).unwrap();
        let path = faces.join("missing.jpg");

        let error = delete_enrolled_face(directory.path(), &path).unwrap_err();

        assert!(error.contains("missing.jpg"));
    }

    #[test]
    fn delete_enrolled_face_rejects_path_outside_faces_directory() {
        let directory = tempfile::tempdir().unwrap();
        let faces = faces_dir(directory.path());
        std::fs::create_dir_all(&faces).unwrap();
        let outside = directory.path().join("outside.jpg");
        std::fs::write(&outside, b"jpeg").unwrap();

        let error = delete_enrolled_face(directory.path(), &outside).unwrap_err();

        assert!(error.contains("outside"));
        assert!(outside.exists());
    }
}
