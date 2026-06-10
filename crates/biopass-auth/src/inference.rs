use ort::session::{builder::GraphOptimizationLevel, Session};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrtTensorInfo {
    pub name: String,
    pub tensor_type: String,
}

#[derive(Debug)]
pub struct OrtModel {
    path: PathBuf,
    inputs: Vec<OrtTensorInfo>,
    outputs: Vec<OrtTensorInfo>,
    session: Session,
}

impl OrtModel {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let session = Session::builder()
            .map_err(|error| format!("Failed to create ONNX Runtime session builder: {error}"))?
            .with_optimization_level(GraphOptimizationLevel::Level1)
            .map_err(|error| format!("Failed to configure ONNX Runtime optimizations: {error}"))?
            .commit_from_file(path)
            .map_err(|error| {
                format!(
                    "Failed to load ONNX model through Rust ONNX Runtime binding {}: {error}",
                    path.display()
                )
            })?;

        let inputs = session
            .inputs
            .iter()
            .map(|input| OrtTensorInfo {
                name: input.name.clone(),
                tensor_type: format!("{:?}", input.input_type),
            })
            .collect();
        let outputs = session
            .outputs
            .iter()
            .map(|output| OrtTensorInfo {
                name: output.name.clone(),
                tensor_type: format!("{:?}", output.output_type),
            })
            .collect();

        Ok(Self {
            path: path.to_path_buf(),
            inputs,
            outputs,
            session,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn inputs(&self) -> &[OrtTensorInfo] {
        &self.inputs
    }

    pub fn outputs(&self) -> &[OrtTensorInfo] {
        &self.outputs
    }

    pub fn session(&mut self) -> &mut Session {
        &mut self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_info_records_name_and_type() {
        let info = OrtTensorInfo {
            name: "input".to_string(),
            tensor_type: "Float32".to_string(),
        };

        assert_eq!(info.name, "input");
        assert_eq!(info.tensor_type, "Float32");
    }
}
