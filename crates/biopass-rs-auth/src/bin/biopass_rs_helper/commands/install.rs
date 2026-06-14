use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use super::config;
use crate::cli::ConfigAction;
use biopass_rs_auth::{current_username, download_models, run_ldconfig, user_data_dir};
use users::os::unix::UserExt;

/// Upstream TickLabVN `biopass` data directory, relative to a user's home.
const UPSTREAM_DATA_DIR: &str = ".local/share/com.ticklab.biopass";

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

    import_legacy_faces(&username);

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

/// Copy enrolled face images from an upstream `biopass` install.
///
/// Only the `faces/` directory is copied. The upstream **config** is ignored
/// because its schema drifts independently and chasing every version is
/// unsustainable; faces are plain image files with no schema, so copying them
/// is always safe.
///
/// Existing destination files are never overwritten, so the upstream directory
/// is left untouched and re-runs are idempotent. The destination resolves
/// through [`user_data_dir`], honouring `BIOPASS_DATA_DIR` and the `--data-dir`
/// override the same way model downloads do — so `mise run dev-helper install`
/// lands them under `dev-data/faces/` rather than the user's real home.
fn import_legacy_faces(username: &str) {
    let Some(home) = users::get_user_by_name(username).map(|user| user.home_dir().to_path_buf())
    else {
        return;
    };
    let src_faces = home.join(UPSTREAM_DATA_DIR).join("faces");
    let dest_faces = user_data_dir(username).join("faces");

    let Ok(entries) = std::fs::read_dir(&src_faces) else {
        // No upstream faces directory — nothing to import.
        return;
    };
    if let Err(error) = std::fs::create_dir_all(&dest_faces) {
        eprintln!("Warning: Failed to create faces dir for '{username}': {error}");
        return;
    }

    let mut copied = 0usize;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let dest = dest_faces.join(name);
        if dest.exists() {
            continue;
        }
        if std::fs::copy(entry.path(), &dest).is_ok() {
            copied += 1;
        }
    }

    if copied > 0 {
        eprintln!("Imported {copied} face image(s) from upstream biopass for user '{username}'");
    }
}
