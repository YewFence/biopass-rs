use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "biopass-rs-helper")]
#[command(about = "BioPass authentication helper")]
pub struct Cli {
    /// Target username. Defaults to the current user (SUDO_USER → USER → USERNAME → LOGNAME).
    /// Ignored by commands that do not operate on a specific user (install, crop-face, completion).
    #[arg(short, long, global = true)]
    pub username: Option<String>,

    /// Override the config file path. Useful for development and testing
    /// without touching the user's real `~/.config/biopass-rs/config.yaml`.
    /// Sets `BIOPASS_CONFIG` for the rest of the helper.
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Override the data directory (faces / debugs). Sets `BIOPASS_DATA_DIR`
    /// for the rest of the helper.
    #[arg(short = 'd', long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authenticate a user
    Auth {
        /// Service name
        #[arg(short, long)]
        service: String,
        /// Temporarily override file log level for this authentication run
        #[arg(long, value_parser = ["debug", "info", "warn", "error"])]
        log_level: Option<String>,
    },
    /// Manage the user's config file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Install models and run setup
    Install,
    /// Download models only
    ModelDownload,
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
        #[arg(long)]
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
    /// Clean diagnostic files, logs, and auth history
    Clean {
        /// Limit cleanup to a specific data category. Defaults to debug frames
        /// only; pass `--target all` to also prune logs and auth history.
        #[arg(long, value_enum, default_value_t = CleanTargetArg::Debugs)]
        target: CleanTargetArg,
        /// Remove all selected entries instead of only entries past configured retention
        #[arg(long)]
        all: bool,
        /// Show what would be removed without deleting files
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CleanTargetArg {
    All,
    Debugs,
    Logs,
    AuthHistory,
}

#[derive(Args)]
pub struct CaptureArgs {
    /// Camera device path
    #[arg(long)]
    pub camera: Option<String>,
    /// Output image path
    #[arg(short, long)]
    pub output: PathBuf,
    /// Detection model path
    #[arg(short, long)]
    pub model: String,
    /// JPEG quality (1-100)
    #[arg(short, long, default_value = "90")]
    pub quality: u8,
}

#[derive(Subcommand)]
pub enum ConfigAction {
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

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
            Commands::Auth { service, log_level } => {
                assert_eq!(service, "sudo");
                assert!(log_level.is_none());
            }
            _ => panic!("expected auth command"),
        }
    }

    #[test]
    fn auth_accepts_log_level_override() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "auth",
            "--service",
            "sudo",
            "--log-level",
            "debug",
        ]);

        match cli.command {
            Commands::Auth { log_level, .. } => assert_eq!(log_level.as_deref(), Some("debug")),
            _ => panic!("expected auth command"),
        }
    }

    #[test]
    fn config_override_parses_short_and_long_flag() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "-c",
            "/tmp/dev-config.yaml",
            "config",
            "reset",
        ]);
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/dev-config.yaml"))
        );

        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "--config",
            "/tmp/dev-config.yaml",
            "auth",
            "--service",
            "sudo",
        ]);
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/dev-config.yaml"))
        );
    }

    #[test]
    fn data_dir_override_parses_short_and_long_flag() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "-d",
            "/tmp/dev-data",
            "auth",
            "--service",
            "sudo",
        ]);
        assert_eq!(
            cli.data_dir.as_deref(),
            Some(std::path::Path::new("/tmp/dev-data"))
        );
    }

    #[test]
    fn help_documents_config_and_data_dir_flags() {
        let mut command = Cli::command();
        let mut help = Vec::new();
        command.write_long_help(&mut help).unwrap();
        let help = String::from_utf8(help).unwrap();

        assert!(
            help.contains("--config"),
            "missing --config in help: {help}"
        );
        assert!(
            help.contains("--data-dir"),
            "missing --data-dir in help: {help}"
        );
    }

    #[test]
    fn clean_parses_with_global_username() {
        let cli = Cli::parse_from(["biopass-rs-helper", "--username", "yewfence", "clean"]);

        assert_eq!(cli.username.as_deref(), Some("yewfence"));
        match cli.command {
            Commands::Clean {
                target,
                all,
                dry_run,
            } => {
                assert_eq!(target, CleanTargetArg::Debugs);
                assert!(!all);
                assert!(!dry_run);
            }
            _ => panic!("expected clean command"),
        }
    }

    #[test]
    fn clean_accepts_target_all_and_dry_run() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "clean",
            "--target",
            "auth-history",
            "--all",
            "--dry-run",
        ]);

        match cli.command {
            Commands::Clean {
                target,
                all,
                dry_run,
            } => {
                assert_eq!(target, CleanTargetArg::AuthHistory);
                assert!(all);
                assert!(dry_run);
            }
            _ => panic!("expected clean command"),
        }
    }

    #[test]
    fn install_and_model_download_parse() {
        let cli = Cli::parse_from(["biopass-rs-helper", "install"]);
        assert!(matches!(cli.command, Commands::Install));

        let cli = Cli::parse_from(["biopass-rs-helper", "model-download"]);
        assert!(matches!(cli.command, Commands::ModelDownload));
    }

    #[test]
    fn crop_face_parses_paths_model_and_quality() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "crop-face",
            "--input",
            "/tmp/input.jpg",
            "--output",
            "/tmp/output.jpg",
            "--model",
            "/tmp/model.onnx",
            "--quality",
            "75",
        ]);

        match cli.command {
            Commands::CropFace {
                input,
                output,
                model,
                quality,
            } => {
                assert_eq!(input, std::path::Path::new("/tmp/input.jpg"));
                assert_eq!(output, std::path::Path::new("/tmp/output.jpg"));
                assert_eq!(model, "/tmp/model.onnx");
                assert_eq!(quality, 75);
            }
            _ => panic!("expected crop-face command"),
        }
    }

    #[test]
    fn capture_face_parses_nested_args_and_default_quality() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "capture-face",
            "--camera",
            "/dev/video2",
            "--output",
            "/tmp/output.jpg",
            "--model",
            "/tmp/model.onnx",
        ]);

        match cli.command {
            Commands::CaptureFace { capture } => {
                assert_eq!(capture.camera.as_deref(), Some("/dev/video2"));
                assert_eq!(capture.output, std::path::Path::new("/tmp/output.jpg"));
                assert_eq!(capture.model, "/tmp/model.onnx");
                assert_eq!(capture.quality, 90);
            }
            _ => panic!("expected capture-face command"),
        }
    }

    #[test]
    fn preview_session_parses_optional_camera_model_and_quality() {
        let cli = Cli::parse_from([
            "biopass-rs-helper",
            "preview-session",
            "--camera",
            "/dev/video4",
            "--model",
            "/tmp/model.onnx",
            "--quality",
            "55",
        ]);

        match cli.command {
            Commands::PreviewSession {
                camera,
                model,
                quality,
            } => {
                assert_eq!(camera.as_deref(), Some("/dev/video4"));
                assert_eq!(model.as_deref(), Some("/tmp/model.onnx"));
                assert_eq!(quality, 55);
            }
            _ => panic!("expected preview-session command"),
        }
    }

    #[test]
    fn completion_parses_shell() {
        let cli = Cli::parse_from(["biopass-rs-helper", "completion", "zsh"]);

        match cli.command {
            Commands::Completion { shell } => assert_eq!(shell, Shell::Zsh),
            _ => panic!("expected completion command"),
        }
    }
}
