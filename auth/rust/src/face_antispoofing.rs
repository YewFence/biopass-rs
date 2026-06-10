use crate::{OrtModel, RgbFrame};
use ort::value::TensorRef;
use std::path::Path;

const DEFAULT_INPUT_SIZE: u32 = 128;
const NORMALIZATION_MEAN: [f32; 3] = [0.5931, 0.4690, 0.4229];
const NORMALIZATION_STD: [f32; 3] = [0.2471, 0.2214, 0.2157];

#[derive(Debug)]
pub struct FaceAntiSpoofing {
    model: OrtModel,
    input_size: u32,
    threshold: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpoofResult {
    pub score: f32,
    pub spoof: bool,
}

impl FaceAntiSpoofing {
    pub fn load(model_path: impl AsRef<Path>, threshold: f32) -> Result<Self, String> {
        Ok(Self {
            model: OrtModel::load(model_path)?,
            input_size: DEFAULT_INPUT_SIZE,
            threshold,
        })
    }

    pub fn detect(&mut self, frame: &RgbFrame) -> Result<SpoofResult, String> {
        if frame.width == 0 || frame.height == 0 {
            return Err("Cannot run face anti-spoofing on an empty frame".to_string());
        }

        let prepared = resize_rgb(frame, self.input_size, self.input_size)?;
        let input = image_to_normalized_chw(&prepared, NORMALIZATION_MEAN, NORMALIZATION_STD);
        let tensor = TensorRef::from_array_view((
            [
                1usize,
                3usize,
                self.input_size as usize,
                self.input_size as usize,
            ],
            &input[..],
        ))
        .map_err(|error| format!("Failed to create anti-spoofing input tensor: {error}"))?;

        let outputs = self
            .model
            .session()
            .run(ort::inputs![tensor])
            .map_err(|error| format!("Failed to run face anti-spoofing model: {error}"))?;
        let (shape, logits) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| format!("Failed to read anti-spoofing output tensor: {error}"))?;
        if logits.len() < 2 {
            return Err(format!(
                "Expected at least 2 anti-spoofing logits, got shape {shape:?}"
            ));
        }

        Ok(spoof_result_from_logits(&logits[0..2], self.threshold))
    }
}

fn spoof_result_from_logits(logits: &[f32], threshold: f32) -> SpoofResult {
    let class = argmax(logits);
    let score = logits[class];
    SpoofResult {
        score,
        spoof: class == 0 && score >= threshold,
    }
}

fn argmax(values: &[f32]) -> usize {
    let mut max_index = 0;
    let mut max_value = values[0];
    for (index, value) in values.iter().enumerate().skip(1) {
        if *value > max_value {
            max_value = *value;
            max_index = index;
        }
    }
    max_index
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
    fn argmax_returns_first_largest_index() {
        assert_eq!(argmax(&[0.8, 0.2]), 0);
        assert_eq!(argmax(&[0.1, 0.9]), 1);
    }

    #[test]
    fn spoof_result_matches_cpp_threshold_logic() {
        assert_eq!(
            spoof_result_from_logits(&[0.8, 0.2], 0.8),
            SpoofResult {
                score: 0.8,
                spoof: true
            }
        );
        assert_eq!(
            spoof_result_from_logits(&[0.7, 0.9], 0.8),
            SpoofResult {
                score: 0.9,
                spoof: false
            }
        );
    }

    #[test]
    fn image_to_normalized_chw_uses_antispoofing_constants() {
        let frame = RgbFrame::new(1, 1, vec![255, 0, 128]).unwrap();

        let chw = image_to_normalized_chw(&frame, NORMALIZATION_MEAN, NORMALIZATION_STD);

        assert_eq!(chw.len(), 3);
        assert!((chw[0] - ((1.0 - 0.5931) / 0.2471)).abs() < f32::EPSILON);
        assert!((chw[1] - ((0.0 - 0.4690) / 0.2214)).abs() < f32::EPSILON);
        assert!((chw[2] - ((128.0 / 255.0 - 0.4229) / 0.2157)).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_rgb_uses_bilinear_sampling() {
        let frame = RgbFrame::new(2, 1, vec![0, 0, 0, 255, 255, 255]).unwrap();

        let resized = resize_rgb(&frame, 1, 1).unwrap();

        assert_eq!(resized.data, vec![128, 128, 128]);
    }
}
