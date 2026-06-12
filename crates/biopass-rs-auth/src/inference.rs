use std::path::{Path, PathBuf};
use tract_onnx::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorInfo {
    pub name: String,
    pub tensor_type: String,
}

#[derive(Debug)]
pub struct InferenceModel {
    path: PathBuf,
    inputs: Vec<TensorInfo>,
    outputs: Vec<TensorInfo>,
    model: std::sync::Arc<TypedRunnableModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct F32TensorOutput {
    pub shape: Vec<usize>,
    pub values: Vec<f32>,
}

impl InferenceModel {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        reject_lfs_pointer(path)?;

        let model = tract_onnx::onnx()
            .model_for_path(path)
            .map_err(|error| {
                format!(
                    "Failed to parse ONNX model through tract {}: {error:#}",
                    path.display()
                )
            })
            .and_then(|model| {
                let typed = match model.clone().into_optimized() {
                    Ok(model) => model,
                    Err(optimization_error) => model.into_typed().map_err(|typed_error| {
                        format!(
                            "Failed to type ONNX model after tract optimization failed. optimization error: {optimization_error:#}; typed fallback error: {typed_error:#}"
                        )
                    })?,
                };

                typed.into_runnable().map_err(|error| {
                    format!(
                        "Failed to create runnable tract model {}: {error:#}",
                        path.display()
                    )
                })
            })?;

        let graph = model.model();
        let inputs = graph
            .input_outlets()
            .map_err(|error| format!("Failed to inspect tract model inputs: {error:#}"))?
            .iter()
            .map(|outlet| {
                let node = &graph.nodes()[outlet.node];
                TensorInfo {
                    name: node.name.clone(),
                    tensor_type: format!("{:?}", node.outputs[outlet.slot].fact),
                }
            })
            .collect();
        let outputs = graph
            .output_outlets()
            .map_err(|error| format!("Failed to inspect tract model outputs: {error:#}"))?
            .iter()
            .map(|outlet| {
                let node = &graph.nodes()[outlet.node];
                TensorInfo {
                    name: node.name.clone(),
                    tensor_type: format!("{:?}", node.outputs[outlet.slot].fact),
                }
            })
            .collect();

        Ok(Self {
            path: path.to_path_buf(),
            inputs,
            outputs,
            model,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn inputs(&self) -> &[TensorInfo] {
        &self.inputs
    }

    pub fn outputs(&self) -> &[TensorInfo] {
        &self.outputs
    }

    pub fn run_f32(&self, shape: &[usize], input: &[f32]) -> Result<Vec<F32TensorOutput>, String> {
        let tensor = Tensor::from_shape(shape, input)
            .map_err(|error| format!("Failed to create tract input tensor: {error:#}"))?;
        let outputs = self.model.run(tvec![tensor.into()]).map_err(|error| {
            format!(
                "Failed to run tract model {}: {error:#}",
                self.path.display()
            )
        })?;

        outputs
            .into_iter()
            .map(|output| {
                let tensor = output.into_tensor();
                let shape = tensor.shape().to_vec();
                let values = tensor
                    .to_plain_array_view::<f32>()
                    .map_err(|error| format!("Failed to read tract f32 output tensor: {error:#}"))?
                    .iter()
                    .copied()
                    .collect();

                Ok(F32TensorOutput { shape, values })
            })
            .collect()
    }
}

pub fn reject_lfs_pointer(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n") {
        return Err(format!(
            "{} is a Git LFS pointer, run git lfs pull before probing ONNX compatibility",
            path.display()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_info_records_name_and_type() {
        let info = TensorInfo {
            name: "input".to_string(),
            tensor_type: "Float32".to_string(),
        };

        assert_eq!(info.name, "input");
        assert_eq!(info.tensor_type, "Float32");
    }

    #[test]
    fn reports_lfs_pointer_models_before_tract_parse() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            temp.path(),
            "version https://git-lfs.github.com/spec/v1\noid sha256:test\nsize 1\n",
        )
        .unwrap();

        let error = reject_lfs_pointer(temp.path()).unwrap_err();

        assert!(error.contains("Git LFS pointer"));
    }
}
