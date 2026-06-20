mod cli;
mod commands;
mod utils;

use biopass_rs_auth::{set_config_path_override, set_data_dir_override};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, Commands};
use commands::auth::{authenticate, EXIT_AUTH_ERR};
use std::io::{self, Write};
use std::process::ExitCode;
use utils::resolve_username;

fn main() -> ExitCode {
    let cli = Cli::parse();

    run_cli(cli)
}

fn run_cli(cli: Cli) -> ExitCode {
    run_cli_with_username_resolver(cli, resolve_username, &mut io::stdout())
}

fn run_cli_with_username_resolver(
    cli: Cli,
    username_resolver: impl Fn(Option<&str>) -> Option<String>,
    completion_writer: &mut impl Write,
) -> ExitCode {
    if let Some(path) = cli.config.clone() {
        set_config_path_override(path);
    }
    if let Some(path) = cli.data_dir.clone() {
        set_data_dir_override(path);
    }

    let username = cli.username;
    let code = match cli.command {
        Commands::Auth { service, log_level } => {
            let target = username_resolver(username.as_deref());
            authenticate(target.as_deref(), Some(&service), log_level.as_deref())
        }
        Commands::Config { action } => match username_resolver(username.as_deref()) {
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
                completion_writer,
            );
            return ExitCode::SUCCESS;
        }
        Commands::Clean => match username_resolver(username.as_deref()) {
            Some(name) => commands::clean::run(&name),
            None => {
                eprintln!(
                    "clean: no target user provided and none could be inferred from the environment"
                );
                EXIT_AUTH_ERR
            }
        },
    };
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap_complete::Shell;

    fn no_username(_: Option<&str>) -> Option<String> {
        None
    }

    #[test]
    fn completion_exits_successfully() {
        let mut completion = Vec::new();
        let cli = Cli {
            username: None,
            config: None,
            data_dir: None,
            command: Commands::Completion { shell: Shell::Bash },
        };

        assert_eq!(
            run_cli_with_username_resolver(cli, no_username, &mut completion),
            ExitCode::SUCCESS
        );
        assert!(!completion.is_empty());
    }

    #[test]
    fn config_requires_resolved_username() {
        let cli = Cli {
            username: Some(String::new()),
            config: None,
            data_dir: None,
            command: Commands::Config {
                action: cli::ConfigAction::Reset,
            },
        };

        assert_eq!(
            run_cli_with_username_resolver(cli, no_username, &mut Vec::new()),
            ExitCode::from(EXIT_AUTH_ERR)
        );
    }

    #[test]
    fn clean_requires_resolved_username() {
        let cli = Cli {
            username: Some(String::new()),
            config: None,
            data_dir: None,
            command: Commands::Clean,
        };

        assert_eq!(
            run_cli_with_username_resolver(cli, no_username, &mut Vec::new()),
            ExitCode::from(EXIT_AUTH_ERR)
        );
    }

    #[test]
    fn auth_rejects_empty_username() {
        let cli = Cli {
            username: Some(String::new()),
            config: None,
            data_dir: None,
            command: Commands::Auth {
                service: "sudo".to_string(),
                log_level: None,
            },
        };

        assert_eq!(
            run_cli_with_username_resolver(cli, no_username, &mut Vec::new()),
            ExitCode::from(EXIT_AUTH_ERR)
        );
    }
}
