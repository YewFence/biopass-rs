use clap::{Args, Parser, Subcommand};
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
pub struct CaptureArgs {
    /// Camera device path
    #[arg(short, long)]
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
            Commands::Auth { service } => assert_eq!(service, "sudo"),
            _ => panic!("expected auth command"),
        }
    }
}
