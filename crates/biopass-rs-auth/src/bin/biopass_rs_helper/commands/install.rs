use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use super::config;
use crate::cli::ConfigAction;
use biopass_rs_auth::{
    current_username, download_models, import_legacy_faces_for_user, run_ldconfig,
};

pub(crate) fn install() -> u8 {
    eprintln!("Running ldconfig...");
    if let Err(error) = run_ldconfig() {
        eprintln!("Warning: {error}");
    }

    let username = match current_username() {
        Some(username) => username,
        None => {
            eprintln!("Cannot determine current user (set USER/SUDO_USER)");
            return EXIT_AUTH_ERR;
        }
    };

    eprintln!("Bootstrapping config for the current user...");
    // `install` 本质上是 `config init` + `model-download` 的打包，再补一次
    // 上游人脸数据导入。这里复用 `config init` 的实现，让配置路径走与
    // `config` 命令相同的解析链（CLI override → BIOPASS_CONFIG → 用户主目录）。
    let init_code = config::run(&username, ConfigAction::Init { force: false });
    if init_code != EXIT_SUCCESS {
        return init_code;
    }

    match import_legacy_faces_for_user(&username) {
        Ok(outcome) if outcome.copied > 0 => {
            eprintln!(
                "Imported {} face image(s) from upstream biopass for user '{username}'",
                outcome.copied
            );
        }
        Ok(_) => {}
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

pub(crate) fn model_download() -> u8 {
    eprintln!("Downloading models...");
    match download_models() {
        Ok(_) => {
            eprintln!("Models downloaded successfully.");
            EXIT_SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to download models: {error}");
            EXIT_AUTH_ERR
        }
    }
}
