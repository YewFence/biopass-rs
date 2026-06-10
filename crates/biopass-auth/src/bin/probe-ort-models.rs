use biopass_auth::OrtModel;
use std::path::{Path, PathBuf};

fn main() -> Result<(), String> {
    let mut failed = false;

    for model in model_paths_from_args(std::env::args().skip(1).collect())? {
        match OrtModel::load(&model) {
            Ok(model) => {
                println!("ort-compatible {}", model.path().display());
                for input in model.inputs() {
                    println!("  input {} {}", input.name, input.tensor_type);
                }
                for output in model.outputs() {
                    println!("  output {} {}", output.name, output.tensor_type);
                }
            }
            Err(error) => {
                failed = true;
                eprintln!("ort-incompatible {}: {error}", model.display());
            }
        }
    }

    if failed {
        Err("one or more models are not compatible with Rust ONNX Runtime binding".to_string())
    } else {
        Ok(())
    }
}

fn model_paths_from_args(args: Vec<String>) -> Result<Vec<PathBuf>, String> {
    if args.is_empty() {
        return Ok(default_model_paths());
    }

    if args.len() == 2 && args[0] == "--model-dir" {
        return Ok(model_paths_in_dir(Path::new(&args[1])));
    }

    if args[0] == "--model-dir" {
        return Err("Usage: probe-ort-models [--model-dir <dir>] [model.onnx ...]".to_string());
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
}
