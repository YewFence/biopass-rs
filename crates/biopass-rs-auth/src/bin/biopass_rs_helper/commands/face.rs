use super::auth::{EXIT_AUTH_ERR, EXIT_IGNORE, EXIT_SUCCESS};
use crate::utils::helper_auto_optimize_camera;
use biopass_rs_auth::{
    capture_rgb_frame, decode_jpeg_rgb, encode_jpeg, CameraRequest, FaceDetector, RgbFrame,
};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

pub(crate) fn crop_face(input: &PathBuf, output: &PathBuf, model: &str, quality: u8) -> u8 {
    match crop_face_jpeg(input, model, quality) {
        Ok(jpeg) => match std::fs::write(output, jpeg) {
            Ok(()) => EXIT_SUCCESS,
            Err(error) => {
                eprintln!("Failed to save cropped face image: {error}");
                EXIT_AUTH_ERR
            }
        },
        Err(error) if error == "No face detected" => {
            eprintln!("{error}");
            EXIT_IGNORE
        }
        Err(error) => {
            eprintln!("{error}");
            EXIT_AUTH_ERR
        }
    }
}

pub(crate) fn capture_face(
    camera: Option<&str>,
    output: &PathBuf,
    model: &str,
    quality: u8,
    username: Option<&str>,
) -> u8 {
    match capture_face_jpeg(camera, model, quality, username) {
        Ok(jpeg) => match std::fs::write(output, jpeg) {
            Ok(()) => EXIT_SUCCESS,
            Err(error) => {
                eprintln!("Failed to save captured face crop: {error}");
                EXIT_AUTH_ERR
            }
        },
        Err(error) if error == "No face detected" => {
            eprintln!("{error}");
            EXIT_IGNORE
        }
        Err(error) => {
            eprintln!("{error}");
            EXIT_AUTH_ERR
        }
    }
}

pub(crate) fn preview_session(
    camera: Option<&str>,
    model: Option<&str>,
    quality: u8,
    username: Option<&str>,
) -> u8 {
    let mut detector = match model.filter(|model| !model.is_empty()) {
        Some(model) => match FaceDetector::load(model) {
            Ok(detector) => Some(detector),
            Err(error) => {
                println!("ERR failed to load detection model: {error}");
                let _ = std::io::stdout().flush();
                return EXIT_AUTH_ERR;
            }
        },
        None => None,
    };

    println!("READY");
    let _ = std::io::stdout().flush();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return EXIT_AUTH_ERR;
        };

        if line == "QUIT" {
            return EXIT_SUCCESS;
        }

        if line == "FRAME" {
            match capture_camera_jpeg(camera, quality, username) {
                Ok(jpeg) => {
                    println!("OK {}", jpeg.len());
                    if std::io::stdout().write_all(&jpeg).is_err()
                        || std::io::stdout().flush().is_err()
                    {
                        return EXIT_AUTH_ERR;
                    }
                }
                Err(error) => {
                    println!("ERR {error}");
                    let _ = std::io::stdout().flush();
                }
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("CAPTURE ") {
            let Some(detector) = detector.as_mut() else {
                println!("ERR detection model not loaded");
                let _ = std::io::stdout().flush();
                continue;
            };

            match capture_face_jpeg_with_detector(camera, detector, quality, username) {
                Ok(jpeg) => match std::fs::write(path, jpeg) {
                    Ok(()) => println!("OK"),
                    Err(error) => println!("ERR save failed: {error}"),
                },
                Err(error) if error == "No face detected" => println!("NO_FACE"),
                Err(error) => println!("ERR {error}"),
            }
            let _ = std::io::stdout().flush();
            continue;
        }

        println!("ERR unknown command");
        let _ = std::io::stdout().flush();
    }

    EXIT_SUCCESS
}

fn crop_face_jpeg(input: &PathBuf, model: &str, quality: u8) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(input)
        .map_err(|error| format!("Failed to read input image {}: {error}", input.display()))?;
    let frame = decode_jpeg_rgb(&bytes)?;
    let mut detector = FaceDetector::load(model)?;
    let face = detector
        .crop_largest_face(&frame)?
        .ok_or_else(|| "No face detected".to_string())?;
    encode_jpeg(&face, quality)
}

fn capture_face_jpeg(
    camera: Option<&str>,
    model: &str,
    quality: u8,
    username: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut detector = FaceDetector::load(model)?;
    capture_face_jpeg_with_detector(camera, &mut detector, quality, username)
}

fn capture_face_jpeg_with_detector(
    camera: Option<&str>,
    detector: &mut FaceDetector,
    quality: u8,
    username: Option<&str>,
) -> Result<Vec<u8>, String> {
    let frame = capture_camera_frame(camera, username)?;
    let face = detector
        .crop_largest_face(&frame)?
        .ok_or_else(|| "No face detected".to_string())?;
    encode_jpeg(&face, quality)
}

fn capture_camera_jpeg(
    camera: Option<&str>,
    quality: u8,
    username: Option<&str>,
) -> Result<Vec<u8>, String> {
    encode_jpeg(&capture_camera_frame(camera, username)?, quality)
}

fn capture_camera_frame(camera: Option<&str>, username: Option<&str>) -> Result<RgbFrame, String> {
    let request = CameraRequest {
        device_path: camera
            .filter(|camera| !camera.is_empty())
            .map(PathBuf::from),
        auto_optimize_camera: helper_auto_optimize_camera(username),
        ..CameraRequest::default()
    };
    capture_rgb_frame(&request)
}
