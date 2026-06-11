use biopass_auth::{
    capture_rgb_frame, config_exists, decode_jpeg_rgb, encode_jpeg, migrate_config_schema,
    read_config, user_exists, AuthManager, CameraRequest, FaceAuth, FaceDetector, FingerprintAuth,
    PamCode, RgbFrame,
};
use std::env;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_SUCCESS: u8 = 0;
const EXIT_AUTH_ERR: u8 = 1;
const EXIT_IGNORE: u8 = 2;
const EXIT_USAGE: u8 = 64;

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match run(args) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn run(args: Vec<String>) -> Result<u8, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(help());
    };

    match command {
        "auth" => {
            let options = AuthOptions::parse(&args[1..])?;
            Ok(authenticate(&options.username, options.service.as_deref()))
        }
        "migrate" => {
            let options = UsernameOptions::parse(&args[1..])?;
            Ok(migrate(&options.username))
        }
        "capture-face" => {
            let options = CaptureFaceOptions::parse(&args[1..])?;
            Ok(capture_face(&options))
        }
        "preview-session" => {
            let options = PreviewSessionOptions::parse(&args[1..])?;
            Ok(preview_session(&options))
        }
        "crop-face" => {
            let options = CropFaceOptions::parse(&args[1..])?;
            Ok(crop_face(&options))
        }
        _ => Err(help()),
    }
}

fn authenticate(username: &str, service: Option<&str>) -> u8 {
    if !user_exists(username) {
        return EXIT_IGNORE;
    }

    if !config_exists(username) {
        return EXIT_IGNORE;
    }

    let config = read_config(username);
    if service.is_some_and(|service| config.ignores_service(service)) {
        return EXIT_IGNORE;
    }

    let methods = config.auth_methods();
    if methods.is_empty() {
        return EXIT_IGNORE;
    }

    let mut manager = AuthManager::new();
    manager.set_mode(config.execution_mode());
    manager.set_config(config.runtime_auth_config());
    for method in methods {
        match method.name.as_str() {
            "face" => manager.add_method(Box::new(FaceAuth::new(config.methods.face.clone()))),
            "fingerprint" => manager.add_method(Box::new(FingerprintAuth::new(
                config.methods.fingerprint.clone(),
            ))),
            _ => {}
        }
    }

    match manager.authenticate(username).code {
        PamCode::Success => EXIT_SUCCESS,
        PamCode::Ignore => EXIT_IGNORE,
        PamCode::AuthError => EXIT_AUTH_ERR,
    }
}

fn migrate(username: &str) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }

    match migrate_config_schema(username) {
        Ok(_) => EXIT_SUCCESS,
        Err(error) => {
            eprintln!("Failed to migrate config schema: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn crop_face(options: &CropFaceOptions) -> u8 {
    match crop_face_jpeg(&options.input, &options.model, options.quality) {
        Ok(jpeg) => match std::fs::write(&options.output, jpeg) {
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

fn capture_face(options: &CaptureFaceOptions) -> u8 {
    match capture_face_jpeg(options.camera.as_deref(), &options.model, options.quality) {
        Ok(jpeg) => match std::fs::write(&options.output, jpeg) {
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

fn preview_session(options: &PreviewSessionOptions) -> u8 {
    let mut detector = match options.model.as_deref().filter(|model| !model.is_empty()) {
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

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return EXIT_AUTH_ERR;
        };

        if line == "QUIT" {
            return EXIT_SUCCESS;
        }

        if line == "FRAME" {
            match capture_camera_jpeg(options.camera.as_deref(), options.quality) {
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

            match capture_face_jpeg_with_detector(
                options.camera.as_deref(),
                detector,
                options.quality,
            ) {
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

fn capture_face_jpeg(camera: Option<&str>, model: &str, quality: u8) -> Result<Vec<u8>, String> {
    let mut detector = FaceDetector::load(model)?;
    capture_face_jpeg_with_detector(camera, &mut detector, quality)
}

fn capture_face_jpeg_with_detector(
    camera: Option<&str>,
    detector: &mut FaceDetector,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let frame = capture_camera_frame(camera)?;
    let face = detector
        .crop_largest_face(&frame)?
        .ok_or_else(|| "No face detected".to_string())?;
    encode_jpeg(&face, quality)
}

fn capture_camera_jpeg(camera: Option<&str>, quality: u8) -> Result<Vec<u8>, String> {
    encode_jpeg(&capture_camera_frame(camera)?, quality)
}

fn capture_camera_frame(camera: Option<&str>) -> Result<RgbFrame, String> {
    let request = CameraRequest {
        device_path: camera
            .filter(|camera| !camera.is_empty())
            .map(PathBuf::from),
        auto_optimize_camera: helper_auto_optimize_camera(),
        ..CameraRequest::default()
    };
    capture_rgb_frame(&request)
}

fn helper_auto_optimize_camera() -> bool {
    let user = env::var("SUDO_USER")
        .ok()
        .or_else(|| env::var("USER").ok())
        .or_else(|| env::var("USERNAME").ok());
    let Some(user) = user else {
        return true;
    };
    read_config(&user).methods.face.auto_optimize_camera
}

#[derive(Debug, PartialEq, Eq)]
struct UsernameOptions {
    username: String,
}

impl UsernameOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut username = None;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--username" | "-u" => {
                    index += 1;
                    username = args.get(index).cloned();
                }
                unknown => return Err(format!("Unknown option {unknown}")),
            }
            index += 1;
        }

        Ok(Self {
            username: username.ok_or_else(help)?,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct AuthOptions {
    username: String,
    service: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CropFaceOptions {
    input: PathBuf,
    output: PathBuf,
    model: String,
    quality: u8,
}

impl CropFaceOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut input = None;
        let mut output = None;
        let mut model = None;
        let mut quality = 90;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--input" | "-i" => {
                    index += 1;
                    input = args.get(index).map(PathBuf::from);
                }
                "--output" | "-o" => {
                    index += 1;
                    output = args.get(index).map(PathBuf::from);
                }
                "--model" | "-m" => {
                    index += 1;
                    model = args.get(index).cloned();
                }
                "--quality" | "-q" => {
                    index += 1;
                    quality = parse_quality(args.get(index))?;
                }
                unknown => return Err(format!("Unknown option {unknown}")),
            }
            index += 1;
        }

        Ok(Self {
            input: input.ok_or_else(help)?,
            output: output.ok_or_else(help)?,
            model: model.ok_or_else(help)?,
            quality,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CaptureFaceOptions {
    camera: Option<String>,
    output: PathBuf,
    model: String,
    quality: u8,
}

impl CaptureFaceOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut camera = None;
        let mut output = None;
        let mut model = None;
        let mut quality = 90;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--camera" | "-c" => {
                    index += 1;
                    camera = args.get(index).cloned();
                }
                "--output" | "-o" => {
                    index += 1;
                    output = args.get(index).map(PathBuf::from);
                }
                "--model" | "-m" => {
                    index += 1;
                    model = args.get(index).cloned();
                }
                "--quality" | "-q" => {
                    index += 1;
                    quality = parse_quality(args.get(index))?;
                }
                unknown => return Err(format!("Unknown option {unknown}")),
            }
            index += 1;
        }

        Ok(Self {
            camera,
            output: output.ok_or_else(help)?,
            model: model.ok_or_else(help)?,
            quality,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PreviewSessionOptions {
    camera: Option<String>,
    model: Option<String>,
    quality: u8,
}

impl PreviewSessionOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut camera = None;
        let mut model = None;
        let mut quality = 70;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--camera" | "-c" => {
                    index += 1;
                    camera = args.get(index).cloned();
                }
                "--model" | "-m" => {
                    index += 1;
                    model = args.get(index).cloned();
                }
                "--quality" | "-q" => {
                    index += 1;
                    quality = parse_quality(args.get(index))?;
                }
                unknown => return Err(format!("Unknown option {unknown}")),
            }
            index += 1;
        }

        Ok(Self {
            camera,
            model,
            quality,
        })
    }
}

fn parse_quality(value: Option<&String>) -> Result<u8, String> {
    let value = value.ok_or_else(help)?;
    let quality = value
        .parse::<u8>()
        .map_err(|_| format!("Invalid JPEG quality {value}"))?;
    if (1..=100).contains(&quality) {
        Ok(quality)
    } else {
        Err(format!(
            "JPEG quality must be between 1 and 100, got {quality}"
        ))
    }
}

impl AuthOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut username = None;
        let mut service = None;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--username" | "-u" => {
                    index += 1;
                    username = args.get(index).cloned();
                }
                "--service" | "-s" => {
                    index += 1;
                    service = args.get(index).cloned();
                }
                unknown => return Err(format!("Unknown option {unknown}")),
            }
            index += 1;
        }

        Ok(Self {
            username: username.ok_or_else(help)?,
            service,
        })
    }
}

fn help() -> String {
    "Usage: biopass-helper auth --username <name> [--service <service>] | migrate --username <name> | crop-face --input <path> --output <path> --model <path> | capture-face --output <path> --model <path> [--camera <path>] | preview-session --model <path> [--camera <path>]"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_auth_options() {
        let options =
            AuthOptions::parse(&args(&["--username", "alice", "--service", "sudo"])).unwrap();

        assert_eq!(
            options,
            AuthOptions {
                username: "alice".to_string(),
                service: Some("sudo".to_string())
            }
        );
    }

    #[test]
    fn parses_short_username_option() {
        let options = UsernameOptions::parse(&args(&["-u", "alice"])).unwrap();

        assert_eq!(
            options,
            UsernameOptions {
                username: "alice".to_string()
            }
        );
    }

    #[test]
    fn rejects_unknown_options() {
        let error = AuthOptions::parse(&args(&["--wat"])).unwrap_err();

        assert_eq!(error, "Unknown option --wat");
    }

    #[test]
    fn missing_config_is_pam_ignore() {
        assert_eq!(
            authenticate("__biopass_missing_user_for_test__", None),
            EXIT_IGNORE
        );
    }

    #[test]
    fn parses_crop_face_options() {
        let options = CropFaceOptions::parse(&args(&[
            "--input",
            "/tmp/input.jpg",
            "--output",
            "/tmp/face.jpg",
            "--model",
            "model.onnx",
            "--quality",
            "80",
        ]))
        .unwrap();

        assert_eq!(options.input, PathBuf::from("/tmp/input.jpg"));
        assert_eq!(options.output, PathBuf::from("/tmp/face.jpg"));
        assert_eq!(options.model, "model.onnx");
        assert_eq!(options.quality, 80);
    }

    #[test]
    fn parses_capture_face_options() {
        let options = CaptureFaceOptions::parse(&args(&[
            "--camera",
            "/dev/video2",
            "--output",
            "/tmp/face.jpg",
            "--model",
            "model.onnx",
            "--quality",
            "80",
        ]))
        .unwrap();

        assert_eq!(options.camera.as_deref(), Some("/dev/video2"));
        assert_eq!(options.output, PathBuf::from("/tmp/face.jpg"));
        assert_eq!(options.model, "model.onnx");
        assert_eq!(options.quality, 80);
    }

    #[test]
    fn parses_preview_session_options() {
        let options = PreviewSessionOptions::parse(&args(&[
            "--model",
            "model.onnx",
            "--camera",
            "/dev/video0",
            "--quality",
            "55",
        ]))
        .unwrap();

        assert_eq!(options.model.as_deref(), Some("model.onnx"));
        assert_eq!(options.camera.as_deref(), Some("/dev/video0"));
        assert_eq!(options.quality, 55);
    }

    #[test]
    fn rejects_invalid_quality() {
        let error = parse_quality(Some(&"0".to_string())).unwrap_err();

        assert_eq!(error, "JPEG quality must be between 1 and 100, got 0");
    }
}
