mod cli;
mod commands;
mod utils;

use biopass_rs_auth::{set_config_path_override, set_data_dir_override};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, Commands};
use commands::auth::{authenticate, EXIT_AUTH_ERR};
use std::io;
use std::process::ExitCode;
use utils::resolve_username;

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(path) = cli.config.clone() {
        set_config_path_override(path);
    }
    if let Some(path) = cli.data_dir.clone() {
        set_data_dir_override(path);
    }

    let username = cli.username;
    let code = match cli.command {
        Commands::Auth { service } => {
            let target = resolve_username(username.as_deref());
            authenticate(target.as_deref(), Some(&service))
        }
        Commands::Config { action } => match resolve_username(username.as_deref()) {
            Some(name) => commands::config::run(&name, action),
            None => {
                eprintln!(
                    "config: no target user provided and none could be inferred from the environment"
                );
                EXIT_AUTH_ERR
            }
        },
        Commands::Install => commands::install::install(),
        Commands::ModelDownload => commands::install::model_download(),
        Commands::CropFace {
            input,
            output,
            model,
            quality,
        } => commands::face::crop_face(&input, &output, &model, quality),
        Commands::CaptureFace { capture } => commands::face::capture_face(
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
        } => commands::face::preview_session(
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
