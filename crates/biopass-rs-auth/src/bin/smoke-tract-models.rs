use biopass_rs_auth::InferenceModel;
use std::path::{Path, PathBuf};

fn main() -> Result<(), String> {
    let mut failed = false;

    for spec in smoke_specs_from_args(std::env::args().skip(1).collect())? {
        match smoke_model(&spec) {
            Ok(outputs) => {
                println!("tract-smoke-compatible {}", spec.path.display());
                for output in outputs {
                    println!("  output {:?}", output.shape);
                }
            }
            Err(error) => {
                failed = true;
                eprintln!("tract-smoke-incompatible {}: {error}", spec.path.display());
            }
        }
    }

    if failed {
        Err("one or more models failed tract smoke inference".to_string())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmokeSpec {
    path: PathBuf,
    input_shape: Vec<usize>,
}

fn smoke_specs_from_args(args: Vec<String>) -> Result<Vec<SmokeSpec>, String> {
    if args.is_empty() {
        return Ok(default_smoke_specs());
    }

    if args.len() == 2 && args[0] == "--model-dir" {
        return Ok(smoke_specs_in_dir(Path::new(&args[1])));
    }

    Err("Usage: smoke-tract-models [--model-dir <dir>]".to_string())
}

fn default_smoke_specs() -> Vec<SmokeSpec> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    smoke_specs_in_dir(&repo_root.join("assets/models/face"))
}

fn smoke_specs_in_dir(dir: &Path) -> Vec<SmokeSpec> {
    vec![
        SmokeSpec {
            path: dir.join("yolov8n-face.onnx"),
            input_shape: vec![1, 3, 640, 640],
        },
        SmokeSpec {
            path: dir.join("edgeface_s_gamma_05.onnx"),
            input_shape: vec![1, 3, 112, 112],
        },
        SmokeSpec {
            path: dir.join("mobilenetv3_antispoof.onnx"),
            input_shape: vec![1, 3, 128, 128],
        },
    ]
}

fn smoke_model(spec: &SmokeSpec) -> Result<Vec<biopass_rs_auth::F32TensorOutput>, String> {
    let model = InferenceModel::load(&spec.path)?;
    let input = vec![0.0; spec.input_shape.iter().product()];
    model.run_f32(&spec.input_shape, &input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_current_face_models() {
        let specs = default_smoke_specs();

        assert_eq!(specs.len(), 3);
        assert!(specs
            .iter()
            .any(|spec| spec.path.ends_with("yolov8n-face.onnx")));
        assert!(specs
            .iter()
            .any(|spec| spec.path.ends_with("edgeface_s_gamma_05.onnx")));
        assert!(specs
            .iter()
            .any(|spec| spec.path.ends_with("mobilenetv3_antispoof.onnx")));
    }

    #[test]
    fn accepts_model_dir_argument() {
        let specs =
            smoke_specs_from_args(vec!["--model-dir".to_string(), "/tmp/models".to_string()])
                .unwrap();

        assert_eq!(
            specs[0].path,
            PathBuf::from("/tmp/models/yolov8n-face.onnx")
        );
        assert_eq!(specs[0].input_shape, [1, 3, 640, 640]);
        assert_eq!(
            specs[1].path,
            PathBuf::from("/tmp/models/edgeface_s_gamma_05.onnx")
        );
        assert_eq!(specs[1].input_shape, [1, 3, 112, 112]);
        assert_eq!(
            specs[2].path,
            PathBuf::from("/tmp/models/mobilenetv3_antispoof.onnx")
        );
        assert_eq!(specs[2].input_shape, [1, 3, 128, 128]);
    }
}
