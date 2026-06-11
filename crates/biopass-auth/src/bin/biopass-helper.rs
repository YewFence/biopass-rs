use biopass_auth::{
    capture_rgb_frame, config_exists, decode_jpeg_rgb, download_models, encode_jpeg,
    migrate_all_users, migrate_config_schema, read_config, run_ldconfig, user_exists, AuthManager,
    CameraRequest, FaceAuth, FaceDetector, FingerprintAuth, PamCode, RgbFrame,
};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_SUCCESS: u8 = 0;
const EXIT_AUTH_ERR: u8 = 1;
const EXIT_IGNORE: u8 = 2;

#[derive(Parser)]
#[command(name = "biopass-helper")]
#[command(about = "BioPass authentication helper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate user
    Auth {
        /// Service name
        #[arg(short, long)]
        service: String,
        /// Username to authenticate
        #[arg(short, long)]
        username: Option<String>,
    },
    /// Migrate user configuration
    Migrate {
        /// Username to migrate
        #[arg(short, long)]
        username: String,
    },
    /// Install models and run setup
    Install,
    /// Crop face from image
    CropFace {
        /// Input image path
        #[arg(short, long)]
        input: PathBuf,
        /// Output image path
        #[arg(short, long)]
        output: PathBuf,
        /// Detection model path
        #[arg(short, long)]
        model: String,
        /// JPEG quality (1-100)
        #[arg(short, long, default_value = "90")]
        quality: u8,
    },
    /// Capture face from camera
    CaptureFace {
        /// Camera device path
        #[arg(short, long)]
        camera: Option<String>,
        /// Output image path
        #[arg(short, long)]
        output: PathBuf,
        /// Detection model path
        #[arg(short, long)]
        model: String,
        /// JPEG quality (1-100)
        #[arg(short, long, default_value = "90")]
        quality: u8,
    },
    /// Start interactive preview session
    PreviewSession {
        /// Camera device path
        #[arg(short, long)]
        camera: Option<String>,
        /// Detection model path
        #[arg(short, long)]
        model: Option<String>,
        /// JPEG quality (1-100)
        #[arg(short, long, default_value = "70")]
        quality: u8,
    },
    /// Generate shell completion script
    Completion {
        /// Shell type
        #[arg(value_enum)]
        shell: Shell,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Auth { username, service } => authenticate(username.as_deref(), Some(&service)),
        Commands::Migrate { username } => migrate(&username),
        Commands::Install => install(),
        Commands::CropFace {
            input,
            output,
            model,
            quality,
        } => crop_face(&input, &output, &model, quality),
        Commands::CaptureFace {
            camera,
            output,
            model,
            quality,
        } => capture_face(camera.as_deref(), &output, &model, quality),
        Commands::PreviewSession {
            camera,
            model,
            quality,
        } => preview_session(camera.as_deref(), model.as_deref(), quality),
        Commands::Completion { shell } => {
            generate(
                shell,
                &mut Cli::command(),
                "biopass-helper",
                &mut io::stdout(),
            );
            return ExitCode::SUCCESS;
        }
    };
    ExitCode::from(code)
}

fn authenticate(_username: Option<&str>, service: Option<&str>) -> u8 {
    let Some(username) = _username
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(current_username)
    else {
        eprintln!("auth: no target user provided and none could be inferred from the environment");
        return EXIT_AUTH_ERR;
    };

    if !user_exists(&username) {
        return EXIT_IGNORE;
    }

    if !config_exists(&username) {
        return EXIT_IGNORE;
    }

    let config = match read_config(&username) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("auth: {error}");
            return EXIT_AUTH_ERR;
        }
    };
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

    let outcome = manager.authenticate(&username);
    match outcome.code {
        PamCode::Success => EXIT_SUCCESS,
        PamCode::Ignore => EXIT_IGNORE,
        PamCode::AuthError => {
            eprintln!("Authentication failed");
            EXIT_AUTH_ERR
        }
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

fn install() -> u8 {
    eprintln!("Running ldconfig...");
    if let Err(error) = run_ldconfig() {
        eprintln!("Warning: {error}");
    }

    eprintln!("Migrating configurations...");
    if let Err(error) = migrate_all_users() {
        eprintln!("Warning: {error}");
    }

    eprintln!("Downloading models...");
    match download_models() {
        Ok(_) => {
            eprintln!("Installation complete.");
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to download models: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn crop_face(input: &PathBuf, output: &PathBuf, model: &str, quality: u8) -> u8 {
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

fn capture_face(camera: Option<&str>, output: &PathBuf, model: &str, quality: u8) -> u8 {
    match capture_face_jpeg(camera, model, quality) {
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

fn preview_session(camera: Option<&str>, model: Option<&str>, quality: u8) -> u8 {
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

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return EXIT_AUTH_ERR;
        };

        if line == "QUIT" {
            return EXIT_SUCCESS;
        }

        if line == "FRAME" {
            match capture_camera_jpeg(camera, quality) {
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

            match capture_face_jpeg_with_detector(camera, detector, quality) {
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
    read_config(&user)
        .map(|config| config.methods.face.auto_optimize_camera)
        .unwrap_or(true)
}

fn current_username() -> Option<String> {
    for key in ["SUDO_USER", "USER", "USERNAME", "LOGNAME"] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty()
                && trimmed
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
            {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}
