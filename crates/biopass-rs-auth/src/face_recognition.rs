use crate::{InferenceModel, RgbFrame};
use std::path::Path;

const DEFAULT_INPUT_SIZE: u32 = 112;
const NORMALIZATION_MEAN: [f32; 3] = [0.5, 0.5, 0.5];
const NORMALIZATION_STD: [f32; 3] = [0.5, 0.5, 0.5];

#[derive(Debug)]
pub struct FaceRecognizer {
    model: InferenceModel,
    input_size: u32,
    threshold: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FaceMatch {
    pub similarity: f32,
    pub similar: bool,
}

impl FaceRecognizer {
    pub fn load(model_path: impl AsRef<Path>, threshold: f32) -> Result<Self, String> {
        Ok(Self {
            model: InferenceModel::load(model_path)?,
            input_size: DEFAULT_INPUT_SIZE,
            threshold,
        })
    }

    pub fn embedding(&mut self, frame: &RgbFrame) -> Result<Vec<f32>, String> {
        if frame.width == 0 || frame.height == 0 {
            return Err("Cannot run face recognition on an empty frame".to_string());
        }

        let prepared = resize_pad(frame, self.input_size, self.input_size)?;
        let input = image_to_normalized_chw(&prepared, NORMALIZATION_MEAN, NORMALIZATION_STD);
        let outputs = self.model.run_f32(
            &[
                1usize,
                3usize,
                self.input_size as usize,
                self.input_size as usize,
            ],
            &input,
        )?;
        let output = outputs
            .first()
            .ok_or_else(|| "Face recognition model returned no outputs".to_string())?;
        if output.shape.len() != 2 || output.shape[0] != 1 {
            return Err(format!(
                "Expected face recognition output shape [1, embedding], got {:?}",
                output.shape
            ));
        }

        Ok(output.values.clone())
    }

    pub fn match_faces(
        &mut self,
        enrolled: &RgbFrame,
        candidate: &RgbFrame,
    ) -> Result<FaceMatch, String> {
        let enrolled_embedding = self.embedding(enrolled)?;
        let candidate_embedding = self.embedding(candidate)?;
        let similarity = cosine_similarity(&enrolled_embedding, &candidate_embedding)?;
        Ok(FaceMatch {
            similarity,
            similar: similarity > self.threshold,
        })
    }
}

fn cosine_similarity(first: &[f32], second: &[f32]) -> Result<f32, String> {
    if first.len() != second.len() {
        return Err(format!(
            "Embedding length mismatch: {} vs {}",
            first.len(),
            second.len()
        ));
    }

    let mut dot = 0.0;
    let mut first_norm = 0.0;
    let mut second_norm = 0.0;
    for (a, b) in first.iter().zip(second) {
        dot += a * b;
        first_norm += a * a;
        second_norm += b * b;
    }

    let first_norm = first_norm.sqrt();
    let second_norm = second_norm.sqrt();
    if first_norm == 0.0 || second_norm == 0.0 {
        return Err("One of the face embeddings has zero magnitude".to_string());
    }

    Ok(dot / (first_norm * second_norm))
}

fn resize_pad(frame: &RgbFrame, target_width: u32, target_height: u32) -> Result<RgbFrame, String> {
    letterbox(frame, target_width, target_height, 0)
}

fn letterbox(
    frame: &RgbFrame,
    target_width: u32,
    target_height: u32,
    pad_value: u8,
) -> Result<RgbFrame, String> {
    let scale =
        (target_width as f32 / frame.width as f32).min(target_height as f32 / frame.height as f32);
    let resized_width = (frame.width as f32 * scale).round().max(1.0) as u32;
    let resized_height = (frame.height as f32 * scale).round().max(1.0) as u32;
    let resized = resize_rgb(frame, resized_width, resized_height)?;

    let mut data = vec![pad_value; target_width as usize * target_height as usize * 3];
    let dx = (target_width - resized_width) / 2;
    let dy = (target_height - resized_height) / 2;
    for row in 0..resized_height as usize {
        let source = row * resized_width as usize * 3;
        let dest = ((dy as usize + row) * target_width as usize + dx as usize) * 3;
        let len = resized_width as usize * 3;
        data[dest..dest + len].copy_from_slice(&resized.data[source..source + len]);
    }

    RgbFrame::new(target_width, target_height, data)
}

fn resize_rgb(frame: &RgbFrame, target_width: u32, target_height: u32) -> Result<RgbFrame, String> {
    if target_width == 0 || target_height == 0 {
        return Err("Cannot resize RGB frame to an empty size".to_string());
    }

    let mut data = vec![0; target_width as usize * target_height as usize * 3];
    let sx = frame.width as f32 / target_width as f32;
    let sy = frame.height as f32 / target_height as f32;

    for y in 0..target_height {
        let fy = (y as f32 + 0.5) * sy - 0.5;
        let y0 = fy.floor() as i32;
        let y1 = y0 + 1;
        let wy = fy - y0 as f32;
        let y0 = y0.clamp(0, frame.height as i32 - 1) as u32;
        let y1 = y1.clamp(0, frame.height as i32 - 1) as u32;

        for x in 0..target_width {
            let fx = (x as f32 + 0.5) * sx - 0.5;
            let x0 = fx.floor() as i32;
            let x1 = x0 + 1;
            let wx = fx - x0 as f32;
            let x0 = x0.clamp(0, frame.width as i32 - 1) as u32;
            let x1 = x1.clamp(0, frame.width as i32 - 1) as u32;

            for channel in 0..3 {
                let top = (1.0 - wx) * frame_value(frame, x0, y0, channel)
                    + wx * frame_value(frame, x1, y0, channel);
                let bottom = (1.0 - wx) * frame_value(frame, x0, y1, channel)
                    + wx * frame_value(frame, x1, y1, channel);
                let value = (1.0 - wy) * top + wy * bottom;
                data[pixel_offset(target_width, x, y, channel)] =
                    (value + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }

    RgbFrame::new(target_width, target_height, data)
}

fn image_to_normalized_chw(frame: &RgbFrame, mean: [f32; 3], std: [f32; 3]) -> Vec<f32> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let mut output = vec![0.0; 3 * width * height];
    for channel in 0..3 {
        for y in 0..height {
            for x in 0..width {
                let value = frame.data[(y * width + x) * 3 + channel] as f32 / 255.0;
                output[channel * width * height + y * width + x] =
                    (value - mean[channel]) / std[channel];
            }
        }
    }
    output
}

fn frame_value(frame: &RgbFrame, x: u32, y: u32, channel: usize) -> f32 {
    frame.data[pixel_offset(frame.width, x, y, channel)] as f32
}

fn pixel_offset(width: u32, x: u32, y: u32, channel: usize) -> usize {
    (y as usize * width as usize + x as usize) * 3 + channel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_pad_uses_zero_padding() {
        let frame = RgbFrame::new(2, 1, vec![255, 0, 0, 0, 255, 0]).unwrap();

        let resized = resize_pad(&frame, 4, 4).unwrap();

        assert_eq!(resized.width, 4);
        assert_eq!(resized.height, 4);
        assert_eq!(&resized.data[0..12], &[0; 12]);
        assert_eq!(&resized.data[12..15], &[255, 0, 0]);
        assert_eq!(&resized.data[21..24], &[0, 255, 0]);
        assert_eq!(&resized.data[36..48], &[0; 12]);
    }

    #[test]
    fn image_to_normalized_chw_matches_cpp_formula() {
        let frame = RgbFrame::new(2, 1, vec![255, 0, 128, 0, 255, 64]).unwrap();

        let chw = image_to_normalized_chw(&frame, NORMALIZATION_MEAN, NORMALIZATION_STD);

        assert_eq!(
            chw,
            vec![
                1.0,
                -1.0,
                -1.0,
                1.0,
                (128.0 / 255.0 - 0.5) / 0.5,
                (64.0 / 255.0 - 0.5) / 0.5
            ]
        );
    }

    #[test]
    fn cosine_similarity_matches_identical_vectors() {
        let similarity = cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).unwrap();

        assert!((similarity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cosine_similarity_rejects_zero_vectors() {
        let error = cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]).unwrap_err();

        assert_eq!(error, "One of the face embeddings has zero magnitude");
    }
}
