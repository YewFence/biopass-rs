use crate::{
    capture_rgb_frame, decode_jpeg_rgb, encode_jpeg, CameraRequest, FaceDetector, RgbFrame,
};
use std::path::{Path, PathBuf};

pub fn crop_face_jpeg(input: &Path, model: &str, quality: u8) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(input)
        .map_err(|error| format!("Failed to read input image {}: {error}", input.display()))?;
    let frame = decode_jpeg_rgb(&bytes)?;
    let mut detector = FaceDetector::load(model)?;
    crop_largest_face_jpeg(&mut detector, &frame, quality)
}

pub fn capture_face_jpeg(
    camera: Option<&str>,
    model: &str,
    quality: u8,
    auto_optimize_camera: bool,
) -> Result<Vec<u8>, String> {
    let mut detector = FaceDetector::load(model)?;
    capture_face_jpeg_with_detector(camera, &mut detector, quality, auto_optimize_camera)
}

pub fn capture_face_jpeg_with_detector(
    camera: Option<&str>,
    detector: &mut FaceDetector,
    quality: u8,
    auto_optimize_camera: bool,
) -> Result<Vec<u8>, String> {
    let frame = capture_camera_frame(camera, auto_optimize_camera)?;
    crop_largest_face_jpeg(detector, &frame, quality)
}

pub fn capture_camera_frame(
    camera: Option<&str>,
    auto_optimize_camera: bool,
) -> Result<RgbFrame, String> {
    let request = CameraRequest {
        device_path: camera
            .filter(|camera| !camera.is_empty())
            .map(PathBuf::from),
        auto_optimize_camera,
        ..CameraRequest::default()
    };
    capture_rgb_frame(&request)
}

pub fn crop_largest_face_jpeg(
    detector: &mut FaceDetector,
    frame: &RgbFrame,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let face = detector
        .crop_largest_face(frame)?
        .ok_or_else(|| "No face detected".to_string())?;
    encode_jpeg(&face, quality)
}
