use super::auth::{EXIT_AUTH_ERR, EXIT_IGNORE, EXIT_SUCCESS};
use crate::utils::helper_auto_optimize_camera;
use biopass_rs_auth::{
    capture_face_jpeg as capture_face_jpeg_data, crop_face_jpeg as crop_face_jpeg_data,
    crop_largest_face_jpeg, encode_jpeg, CameraRequest, CameraSession, FaceDetector,
};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub(crate) fn crop_face(input: &Path, output: &Path, model: &str, quality: u8) -> u8 {
    match crop_face_jpeg_data(input, model, quality) {
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
    output: &Path,
    model: &str,
    quality: u8,
    username: Option<&str>,
) -> u8 {
    let auto_optimize_camera = helper_auto_optimize_camera(username);
    match capture_face_jpeg_data(camera, model, quality, auto_optimize_camera) {
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

    let request = CameraRequest {
        device_path: camera.filter(|c| !c.is_empty()).map(PathBuf::from),
        auto_optimize_camera: helper_auto_optimize_camera(username),
        ..CameraRequest::default()
    };
    let mut session = match CameraSession::open(&request) {
        Ok(session) => session,
        Err(error) => {
            println!("ERR failed to open camera: {error}");
            let _ = std::io::stdout().flush();
            return EXIT_AUTH_ERR;
        }
    };
    if let Err(error) = session.warmup(request.warmup_frames) {
        println!("ERR warmup failed: {error}");
        let _ = std::io::stdout().flush();
        return EXIT_AUTH_ERR;
    }

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
            match session
                .next_frame()
                .and_then(|frame| encode_jpeg(&frame, quality))
            {
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

            match session.next_frame() {
                Ok(frame) => match crop_largest_face_jpeg(detector, &frame, quality) {
                    Ok(jpeg) => match std::fs::write(path, jpeg) {
                        Ok(()) => println!("OK"),
                        Err(error) => println!("ERR save failed: {error}"),
                    },
                    Err(error) if error == "No face detected" => println!("NO_FACE"),
                    Err(error) => println!("ERR {error}"),
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
