use crate::{InferenceModel, RgbFrame};
use std::path::Path;

const DEFAULT_INPUT_SIZE: u32 = 640;
const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.50;
const DEFAULT_IOU_THRESHOLD: f32 = 0.50;
const DEFAULT_MAX_DETECTIONS: usize = 300;
const LETTERBOX_PAD: u8 = 114;

#[derive(Debug)]
pub struct FaceDetector {
    model: InferenceModel,
    input_size: u32,
    confidence_threshold: f32,
    iou_threshold: f32,
    max_detections: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FaceDetection {
    pub confidence: f32,
    pub bbox: FaceBox,
    pub crop: RgbFrame,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceBox {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

impl FaceBox {
    pub fn width(self) -> u32 {
        self.x2.saturating_sub(self.x1)
    }

    pub fn height(self) -> u32 {
        self.y2.saturating_sub(self.y1)
    }

    pub fn area(self) -> u32 {
        self.width() * self.height()
    }
}

impl FaceDetector {
    pub fn load(model_path: impl AsRef<Path>) -> Result<Self, String> {
        Self::load_with_threshold(model_path, DEFAULT_CONFIDENCE_THRESHOLD)
    }

    pub fn load_with_threshold(
        model_path: impl AsRef<Path>,
        confidence_threshold: f32,
    ) -> Result<Self, String> {
        Ok(Self {
            model: InferenceModel::load(model_path)?,
            input_size: DEFAULT_INPUT_SIZE,
            confidence_threshold,
            iou_threshold: DEFAULT_IOU_THRESHOLD,
            max_detections: DEFAULT_MAX_DETECTIONS,
        })
    }

    pub fn detect(&mut self, frame: &RgbFrame) -> Result<Vec<FaceDetection>, String> {
        if frame.width == 0 || frame.height == 0 {
            return Err("Cannot run face detection on an empty frame".to_string());
        }

        let letterboxed = letterbox(frame, self.input_size, self.input_size)?;
        let input = image_to_chw(&letterboxed);
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
            .ok_or_else(|| "Face detection model returned no outputs".to_string())?;
        let detections = detections_from_yolov8_output(
            &output.values,
            &output.shape,
            frame,
            self.input_size,
            self.confidence_threshold,
            self.iou_threshold,
            self.max_detections,
        )?;

        detections
            .into_iter()
            .map(|raw| {
                let bbox = raw.to_clipped_box(frame.width, frame.height)?;
                let crop = crop_rgb(frame, bbox)?;
                Ok(FaceDetection {
                    confidence: raw.confidence,
                    bbox,
                    crop,
                })
            })
            .collect()
    }

    pub fn crop_largest_face(&mut self, frame: &RgbFrame) -> Result<Option<RgbFrame>, String> {
        let detections = self.detect(frame)?;
        Ok(detections
            .into_iter()
            .max_by_key(|detection| detection.bbox.area())
            .map(|detection| detection.crop))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RawDetection {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    confidence: f32,
}

impl RawDetection {
    fn area(self) -> f32 {
        ((self.x2 - self.x1).max(0.0)) * ((self.y2 - self.y1).max(0.0))
    }

    fn to_clipped_box(self, width: u32, height: u32) -> Result<FaceBox, String> {
        let x1 = self.x1.max(0.0) as u32;
        let y1 = self.y1.max(0.0) as u32;
        let x2 = self.x2.min(width as f32) as u32;
        let y2 = self.y2.min(height as f32) as u32;
        if x2 <= x1 || y2 <= y1 {
            return Err("Detected face box has no area after clipping".to_string());
        }
        Ok(FaceBox { x1, y1, x2, y2 })
    }
}

fn detections_from_yolov8_output(
    output: &[f32],
    shape: &[usize],
    frame: &RgbFrame,
    input_size: u32,
    confidence_threshold: f32,
    iou_threshold: f32,
    max_detections: usize,
) -> Result<Vec<RawDetection>, String> {
    if shape.len() != 3 {
        return Err(format!(
            "Expected YOLOv8-face output rank 3, got shape {shape:?}"
        ));
    }

    let pred_dim = shape[1];
    let num_preds = shape[2];
    if pred_dim < 5 {
        return Err(format!(
            "Expected at least 5 YOLOv8-face channels, got {pred_dim}"
        ));
    }
    if output.len() < pred_dim * num_preds {
        return Err(format!(
            "Face detection output too short: expected at least {} floats, got {}",
            pred_dim * num_preds,
            output.len()
        ));
    }

    let mut candidates = Vec::new();
    for index in 0..num_preds {
        let score = output[4 * num_preds + index];
        if score < confidence_threshold {
            continue;
        }

        let cx = output[index];
        let cy = output[num_preds + index];
        let w = output[2 * num_preds + index];
        let h = output[3 * num_preds + index];
        candidates.push(RawDetection {
            x1: cx - w / 2.0,
            y1: cy - h / 2.0,
            x2: cx + w / 2.0,
            y2: cy + h / 2.0,
            confidence: score,
        });
    }

    let mut detections = nms(&candidates, iou_threshold)
        .into_iter()
        .take(max_detections)
        .map(|index| candidates[index])
        .collect::<Vec<_>>();
    scale_boxes(input_size, frame.width, frame.height, &mut detections);
    Ok(detections
        .into_iter()
        .filter(|detection| detection.area() > 0.0)
        .collect())
}

fn nms(detections: &[RawDetection], iou_threshold: f32) -> Vec<usize> {
    let mut indices = (0..detections.len()).collect::<Vec<_>>();
    indices.sort_by(|a, b| {
        detections[*b]
            .confidence
            .total_cmp(&detections[*a].confidence)
    });

    let mut suppressed = vec![false; detections.len()];
    let mut keep = Vec::new();
    for index in indices.iter().copied() {
        if suppressed[index] {
            continue;
        }
        keep.push(index);

        for other in indices.iter().copied() {
            if suppressed[other] || other == index {
                continue;
            }
            if iou(detections[index], detections[other]) > iou_threshold {
                suppressed[other] = true;
            }
        }
    }
    keep
}

fn iou(a: RawDetection, b: RawDetection) -> f32 {
    let x1 = a.x1.max(b.x1);
    let y1 = a.y1.max(b.y1);
    let x2 = a.x2.min(b.x2);
    let y2 = a.y2.min(b.y2);
    let intersection = ((x2 - x1).max(0.0)) * ((y2 - y1).max(0.0));
    let union = a.area() + b.area() - intersection;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn scale_boxes(
    input_size: u32,
    original_width: u32,
    original_height: u32,
    detections: &mut [RawDetection],
) {
    let gain =
        (input_size as f32 / original_height as f32).min(input_size as f32 / original_width as f32);
    let pad_x = ((input_size as f32 - original_width as f32 * gain) / 2.0 - 0.1).round();
    let pad_y = ((input_size as f32 - original_height as f32 * gain) / 2.0 - 0.1).round();

    for detection in detections {
        detection.x1 = (detection.x1 - pad_x) / gain;
        detection.y1 = (detection.y1 - pad_y) / gain;
        detection.x2 = (detection.x2 - pad_x) / gain;
        detection.y2 = (detection.y2 - pad_y) / gain;
    }
}

fn letterbox(frame: &RgbFrame, target_width: u32, target_height: u32) -> Result<RgbFrame, String> {
    let scale =
        (target_width as f32 / frame.width as f32).min(target_height as f32 / frame.height as f32);
    let resized_width = (frame.width as f32 * scale).round().max(1.0) as u32;
    let resized_height = (frame.height as f32 * scale).round().max(1.0) as u32;
    let resized = resize_rgb(frame, resized_width, resized_height)?;

    let mut data = vec![LETTERBOX_PAD; target_width as usize * target_height as usize * 3];
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

fn crop_rgb(frame: &RgbFrame, bbox: FaceBox) -> Result<RgbFrame, String> {
    let crop_width = bbox.width();
    let crop_height = bbox.height();
    if crop_width == 0 || crop_height == 0 {
        return Err("Cannot crop an empty face box".to_string());
    }

    let mut data = vec![0; crop_width as usize * crop_height as usize * 3];
    for row in 0..crop_height as usize {
        let source = ((bbox.y1 as usize + row) * frame.width as usize + bbox.x1 as usize) * 3;
        let dest = row * crop_width as usize * 3;
        let len = crop_width as usize * 3;
        data[dest..dest + len].copy_from_slice(&frame.data[source..source + len]);
    }

    RgbFrame::new(crop_width, crop_height, data)
}

fn image_to_chw(frame: &RgbFrame) -> Vec<f32> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let mut output = vec![0.0; 3 * width * height];
    for channel in 0..3 {
        for y in 0..height {
            for x in 0..width {
                output[channel * width * height + y * width + x] =
                    frame.data[(y * width + x) * 3 + channel] as f32 / 255.0;
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
    fn letterbox_preserves_aspect_ratio_and_pads() {
        let frame = RgbFrame::new(2, 1, vec![255, 0, 0, 0, 255, 0]).unwrap();

        let boxed = letterbox(&frame, 4, 4).unwrap();

        assert_eq!(boxed.width, 4);
        assert_eq!(boxed.height, 4);
        assert_eq!(&boxed.data[0..12], &[114; 12]);
        assert_eq!(&boxed.data[12..15], &[255, 0, 0]);
        assert_eq!(&boxed.data[21..24], &[0, 255, 0]);
        assert_eq!(&boxed.data[36..48], &[114; 12]);
    }

    #[test]
    fn image_to_chw_normalizes_channels() {
        let frame = RgbFrame::new(2, 1, vec![255, 0, 128, 0, 255, 64]).unwrap();

        let chw = image_to_chw(&frame);

        assert_eq!(chw, vec![1.0, 0.0, 0.0, 1.0, 128.0 / 255.0, 64.0 / 255.0]);
    }

    #[test]
    fn nms_suppresses_overlapping_lower_confidence_boxes() {
        let detections = vec![
            RawDetection {
                x1: 0.0,
                y1: 0.0,
                x2: 10.0,
                y2: 10.0,
                confidence: 0.9,
            },
            RawDetection {
                x1: 1.0,
                y1: 1.0,
                x2: 11.0,
                y2: 11.0,
                confidence: 0.8,
            },
            RawDetection {
                x1: 20.0,
                y1: 20.0,
                x2: 30.0,
                y2: 30.0,
                confidence: 0.7,
            },
        ];

        assert_eq!(nms(&detections, 0.5), vec![0, 2]);
    }

    #[test]
    fn yolov8_output_decodes_and_scales_boxes() {
        let frame = RgbFrame::new(100, 50, vec![0; 100 * 50 * 3]).unwrap();
        let num_preds = 2;
        let pred_dim = 5;
        let mut output = vec![0.0; pred_dim * num_preds];
        output[0] = 320.0;
        output[1] = 160.0;
        output[num_preds] = 320.0;
        output[num_preds + 1] = 160.0;
        output[2 * num_preds] = 128.0;
        output[2 * num_preds + 1] = 128.0;
        output[3 * num_preds] = 128.0;
        output[3 * num_preds + 1] = 128.0;
        output[4 * num_preds] = 0.9;
        output[4 * num_preds + 1] = 0.4;

        let detections = detections_from_yolov8_output(
            &output,
            &[1, pred_dim, num_preds],
            &frame,
            640,
            0.5,
            0.5,
            300,
        )
        .unwrap();

        assert_eq!(detections.len(), 1);
        let bbox = detections[0]
            .to_clipped_box(frame.width, frame.height)
            .unwrap();
        assert_eq!(
            bbox,
            FaceBox {
                x1: 40,
                y1: 15,
                x2: 60,
                y2: 35
            }
        );
    }

    #[test]
    fn yolov8_output_honors_confidence_threshold() {
        let frame = RgbFrame::new(100, 100, vec![0; 100 * 100 * 3]).unwrap();
        let num_preds = 1;
        let pred_dim = 5;
        let mut output = vec![0.0; pred_dim * num_preds];
        output[0] = 50.0;
        output[num_preds] = 50.0;
        output[2 * num_preds] = 20.0;
        output[3 * num_preds] = 20.0;
        output[4 * num_preds] = 0.49;

        let detections = detections_from_yolov8_output(
            &output,
            &[1, pred_dim, num_preds],
            &frame,
            100,
            0.5,
            0.5,
            300,
        )
        .unwrap();

        assert!(detections.is_empty());
    }

    #[test]
    fn crop_rgb_copies_selected_region() {
        let frame = RgbFrame::new(
            3,
            2,
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
            ],
        )
        .unwrap();

        let crop = crop_rgb(
            &frame,
            FaceBox {
                x1: 1,
                y1: 0,
                x2: 3,
                y2: 2,
            },
        )
        .unwrap();

        assert_eq!(crop.width, 2);
        assert_eq!(crop.height, 2);
        assert_eq!(crop.data, vec![4, 5, 6, 7, 8, 9, 13, 14, 15, 16, 17, 18]);
    }
}
