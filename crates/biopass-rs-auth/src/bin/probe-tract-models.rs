use anyhow::bail;
use biopass_rs_auth::InferenceModel;
use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let mut failed = false;

    for model in model_paths_from_args(std::env::args().skip(1).collect())? {
        match InferenceModel::load(&model) {
            Ok(model) => {
                println!("tract-compatible {}", model.path().display());
                for input in model.inputs() {
                    println!("  input {} {}", input.name, input.tensor_type);
                }
                for output in model.outputs() {
                    println!("  output {} {}", output.name, output.tensor_type);
                }
            }
            Err(error) => {
                failed = true;
                eprintln!("tract-incompatible {}: {error}", model.display());
            }
        }
    }

    if failed {
        bail!("one or more models are not compatible with tract-onnx");
    }

    Ok(())
}

fn model_paths_from_args(args: Vec<String>) -> anyhow::Result<Vec<PathBuf>> {
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
    model_paths_in_dir(&repo_root.join("assets/models/face"))
}

fn model_paths_in_dir(dir: &Path) -> Vec<PathBuf> {
    vec![
        dir.join("yolov8n-face.onnx"),
        dir.join("edgeface_s_gamma_05.onnx"),
        dir.join("mobilenetv3_antispoof.onnx"),
    ]
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
