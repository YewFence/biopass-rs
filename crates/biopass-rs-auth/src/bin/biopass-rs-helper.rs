use biopass_rs_auth::{
    bootstrap_config_at, capture_rgb_frame, config_exists, config_path, decode_jpeg_rgb,
    download_models, encode_jpeg, migrate_config_schema, read_config, reset_config, run_ldconfig,
    user_exists, AuthManager, BiopassConfig, BootstrapOutcome, CameraRequest, FaceAuth,
    FaceDetector, FingerprintAuth, PamCode, RgbFrame,
};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_SUCCESS: u8 = 0;
const EXIT_AUTH_ERR: u8 = 1;
const EXIT_IGNORE: u8 = 2;

#[derive(Parser)]
#[command(name = "biopass-rs-helper")]
#[command(about = "BioPass authentication helper")]
struct Cli {
    /// Target username. Defaults to the current user (SUDO_USER → USER → USERNAME → LOGNAME).
    /// Ignored by commands that do not operate on a specific user (install, crop-face, completion).
    #[arg(short, long, global = true)]
    username: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate a user
    Auth {
        /// Service name
        #[arg(short, long)]
        service: String,
    },
    /// Manage the user's config file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
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
        #[command(flatten)]
        capture: CaptureArgs,
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

#[derive(Args)]
struct CaptureArgs {
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
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Write the default config if none exists
    Init {
        /// Overwrite an existing config file
        #[arg(long)]
        force: bool,
    },
    /// Restore the config file to its built-in defaults
    Reset,
    /// Migrate the config file to the current schema
    Migrate,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let username = cli.username;
    let code = match cli.command {
        Commands::Auth { service } => {
            let target = resolve_username(username.as_deref());
            authenticate(target.as_deref(), Some(&service))
        }
        Commands::Config { action } => {
            let target = resolve_username(username.as_deref());
            match target {
                Some(name) => run_config_action(&name, action),
                None => {
                    eprintln!(
                        "config: no target user provided and none could be inferred from the environment"
                    );
                    EXIT_AUTH_ERR
                }
            }
        }
        Commands::Install => install(),
        Commands::CropFace {
            input,
            output,
            model,
            quality,
        } => crop_face(&input, &output, &model, quality),
        Commands::CaptureFace { capture } => capture_face(
            capture.camera.as_deref(),
            &capture.output,
            &capture.model,
            capture.quality,
            username.as_deref(),
        ),
        Commands::PreviewSession {
            camera,
            model,
            quality,
        } => preview_session(
            camera.as_deref(),
            model.as_deref(),
            quality,
            username.as_deref(),
        ),
        Commands::Completion { shell } => {
            generate(
                shell,
                &mut Cli::command(),
                "biopass-rs-helper",
                &mut io::stdout(),
            );
            return ExitCode::SUCCESS;
        }
    };
    ExitCode::from(code)
}

fn authenticate(_username: Option<&str>, service: Option<&str>) -> u8 {
    let Some(username) = _username.filter(|name| !name.is_empty()).map(str::to_owned) else {
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

fn run_config_action(username: &str, action: ConfigAction) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }
    match action {
        ConfigAction::Init { force } => init(username, force),
        ConfigAction::Reset => reset(username),
        ConfigAction::Migrate => migrate(username),
    }
}

fn init(username: &str, force: bool) -> u8 {
    let path = config_path(username);
    if force {
        if let Err(error) = biopass_rs_auth::write_config_to_path(&path, &BiopassConfig::default())
        {
            eprintln!("Failed to initialize config: {error}");
            return EXIT_AUTH_ERR;
        }
        eprintln!(
            "Wrote default config (forced) for user '{username}' at {}",
            path.display()
        );
        return EXIT_SUCCESS;
    }

    let home = home_dir_for_username(username);
    match bootstrap_config_at(&path, home.as_deref(), BiopassConfig::default) {
        Ok(BootstrapOutcome::AlreadyPresent) => {
            eprintln!(
                "Config already exists for user '{username}' at {} (use --force to overwrite)",
                path.display()
            );
            EXIT_SUCCESS
        }
        Ok(BootstrapOutcome::ImportedFromUpstream) => {
            eprintln!(
                "Imported upstream biopass config for user '{username}' into {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Ok(BootstrapOutcome::WroteDefaults) => {
            eprintln!(
                "Wrote default config for user '{username}' at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to initialize config: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn reset(username: &str) -> u8 {
    let path = config_path(username);
    match reset_config(username) {
        Ok(()) => {
            eprintln!(
                "Reset config for user '{username}' to defaults at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to reset config: {error}");
            EXIT_AUTH_ERR
        }
    }
}

fn migrate(username: &str) -> u8 {
    let path = config_path(username);
    if !path.is_file() {
        eprintln!(
            "No config found for user '{username}' at {}",
            path.display()
        );
        return EXIT_SUCCESS;
    }

    match migrate_config_schema(username) {
        Ok(true) => {
            eprintln!(
                "Migrated config schema for user '{username}' at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
        Ok(false) => {
            eprintln!(
                "Config schema already current for user '{username}' at {}",
                path.display()
            );
            EXIT_SUCCESS
        }
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

    eprintln!("Bootstrapping config for the current user...");
    match bootstrap_current_user() {
        Ok(message) => {
            if let Some(message) = message {
                eprintln!("{message}");
            }
        }
        Err(error) => eprintln!("Warning: {error}"),
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

const NEW_CONFIG_PATH: &str = ".config/biopass-rs/config.yaml";
const OLD_DATA_DIR: &str = ".local/share/com.ticklab.biopass";
const NEW_DATA_DIR: &str = ".local/share/biopass-rs";

fn bootstrap_current_user() -> Result<Option<String>, String> {
    let username = current_username()
        .ok_or_else(|| "Cannot determine current user (set USER/SUDO_USER)".to_string())?;
    let home = home_dir_for_username(&username)
        .ok_or_else(|| format!("Cannot determine home directory for user '{username}'"))?;

    let new_config = home.join(NEW_CONFIG_PATH);
    let outcome_message =
        match bootstrap_config_at(&new_config, Some(&home), BiopassConfig::default) {
            Ok(BootstrapOutcome::AlreadyPresent) => format!(
                "Skipping config bootstrap for user '{username}': config already exists at {}",
                new_config.display()
            ),
            Ok(BootstrapOutcome::ImportedFromUpstream) => format!(
                "Imported upstream config for user '{username}' into {}",
                new_config.display()
            ),
            Ok(BootstrapOutcome::WroteDefaults) => format!(
                "Wrote default config for user '{username}' at {}",
                new_config.display()
            ),
            Err(error) => {
                return Err(format!(
                    "failed to bootstrap config for '{username}': {error}"
                ))
            }
        };

    let old_data = home.join(OLD_DATA_DIR);
    let new_data = home.join(NEW_DATA_DIR);
    if old_data.exists() && !new_data.exists() {
        eprintln!("Migrating data directory for user '{username}'...");
        if let Err(error) = std::fs::rename(&old_data, &new_data) {
            eprintln!("Warning: Failed to migrate data dir for '{username}': {error}");
        }
    }

    Ok(Some(outcome_message))
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

fn capture_face(
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

fn preview_session(
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

    let stdin = std::io::stdin();
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

fn helper_auto_optimize_camera(username: Option<&str>) -> bool {
    let user = username
        .filter(|user| !user.is_empty())
        .map(str::to_owned)
        .or_else(current_username);
    let Some(user) = user else {
        return true;
    };
    read_config(&user)
        .map(|config| config.methods.face.auto_optimize_camera)
        .unwrap_or(true)
}

fn resolve_username(explicit: Option<&str>) -> Option<String> {
    explicit
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(current_username)
}

fn home_dir_for_username(username: &str) -> Option<PathBuf> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        if name != username {
            return None;
        }
        let home = parts.nth(4)?;
        if home.is_empty() {
            None
        } else {
            Some(PathBuf::from(home))
        }
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_uses_config_command_tree() {
        let mut command = Cli::command();
        let mut help = Vec::new();
        command.write_long_help(&mut help).unwrap();
        let help = String::from_utf8(help).unwrap();

        assert!(help.contains("config"));
        assert!(help.contains("--username"));
        // The standalone `migrate-config` subcommand was replaced by `config migrate`.
        assert!(!help.contains("migrate-config"));
    }

    #[test]
    fn config_migrate_accepts_global_username() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "--username",
            "yewfence",
            "config",
            "migrate",
        ]);

        assert_eq!(cli.username.as_deref(), Some("yewfence"));
        match cli.command {
            Commands::Config {
                action: ConfigAction::Migrate,
            } => {}
            _ => panic!("expected `config migrate`"),
        }
    }

    #[test]
    fn config_init_force_flag_parses() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "-u",
            "alice",
            "config",
            "init",
            "--force",
        ]);

        assert_eq!(cli.username.as_deref(), Some("alice"));
        match cli.command {
            Commands::Config {
                action: ConfigAction::Init { force },
            } => assert!(force),
            _ => panic!("expected `config init`"),
        }
    }

    #[test]
    fn config_reset_parses() {
        let cli = Cli::parse_from(["biopass-rs-helper", "-u", "bob", "config", "reset"]);
        match cli.command {
            Commands::Config {
                action: ConfigAction::Reset,
            } => {}
            _ => panic!("expected `config reset`"),
        }
    }

    #[test]
    fn auth_accepts_global_username_after_service_flag() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "auth",
            "--service",
            "sudo",
            "--username",
            "carol",
        ]);

        assert_eq!(cli.username.as_deref(), Some("carol"));
        match cli.command {
            Commands::Auth { service } => assert_eq!(service, "sudo"),
            _ => panic!("expected auth command"),
        }
    }
}
