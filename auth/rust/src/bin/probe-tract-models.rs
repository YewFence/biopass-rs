use anyhow::bail;
use std::path::{Path, PathBuf};
use tract_onnx::prelude::*;

fn main() -> TractResult<()> {
    let mut failed = false;

    for model in model_paths_from_args(std::env::args().skip(1).collect())? {
        match probe_model(&model) {
            Ok(()) => println!("tract-compatible {}", model.display()),
            Err(error) => {
                failed = true;
                eprintln!("tract-incompatible {}: {error:#}", model.display());
            }
        }
    }

    if failed {
        bail!("one or more models are not compatible with tract-onnx");
    }

    Ok(())
}

fn model_paths_from_args(args: Vec<String>) -> TractResult<Vec<PathBuf>> {
    if args.is_empty() {
        return Ok(default_model_paths());
    }

    if args.len() == 2 && args[0] == "--model-dir" {
        return Ok(model_paths_in_dir(Path::new(&args[1])));
    }

    if args[0] == "--model-dir" {
        bail!("Usage: probe-tract-models [--model-dir <dir>] [model.onnx ...]");
    }

    Ok(args.into_iter().map(PathBuf::from).collect())
}

fn default_model_paths() -> Vec<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    model_paths_in_dir(&repo_root.join("auth/face/models"))
}

fn model_paths_in_dir(dir: &Path) -> Vec<PathBuf> {
    vec![
        dir.join("yolov8n-face.onnx"),
        dir.join("edgeface_s_gamma_05.onnx"),
        dir.join("mobilenetv3_antispoof.onnx"),
    ]
}

fn probe_model(path: &Path) -> TractResult<()> {
    reject_lfs_pointer(path)?;
    tract_onnx::onnx()
        .model_for_path(path)?
        .into_optimized()?
        .into_runnable()?;
    Ok(())
}

fn reject_lfs_pointer(path: &Path) -> TractResult<()> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n") {
        bail!(
            "{} is a Git LFS pointer, run git lfs pull before probing ONNX compatibility",
            path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_current_face_models() {
        let paths = default_model_paths();

        assert_eq!(paths.len(), 3);
        assert!(paths.iter().any(|path| path.ends_with("yolov8n-face.onnx")));
        assert!(paths
            .iter()
            .any(|path| path.ends_with("edgeface_s_gamma_05.onnx")));
        assert!(paths
            .iter()
            .any(|path| path.ends_with("mobilenetv3_antispoof.onnx")));
    }

    #[test]
    fn reports_lfs_pointer_models_before_tract_parse() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            temp.path(),
            "version https://git-lfs.github.com/spec/v1\noid sha256:test\nsize 1\n",
        )
        .unwrap();

        let error = reject_lfs_pointer(temp.path()).unwrap_err().to_string();

        assert!(error.contains("Git LFS pointer"));
    }

    #[test]
    fn accepts_model_dir_argument() {
        let paths =
            model_paths_from_args(vec!["--model-dir".to_string(), "/tmp/models".to_string()])
                .unwrap();

        assert_eq!(paths[0], PathBuf::from("/tmp/models/yolov8n-face.onnx"));
        assert_eq!(
            paths[1],
            PathBuf::from("/tmp/models/edgeface_s_gamma_05.onnx")
        );
        assert_eq!(
            paths[2],
            PathBuf::from("/tmp/models/mobilenetv3_antispoof.onnx")
        );
    }

    #[test]
    fn accepts_explicit_model_paths() {
        let paths =
            model_paths_from_args(vec!["a.onnx".to_string(), "b.onnx".to_string()]).unwrap();

        assert_eq!(paths, [PathBuf::from("a.onnx"), PathBuf::from("b.onnx")]);
    }
}
