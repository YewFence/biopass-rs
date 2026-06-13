use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use biopass_rs_auth::{user_data_dir, user_exists};

pub(crate) fn run(username: &str) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }

    let debug_dir = user_data_dir(username).join("debugs");
    let Ok(entries) = std::fs::read_dir(&debug_dir) else {
        eprintln!(
            "No debug cache found for user '{username}' at {}",
            debug_dir.display()
        );
        return EXIT_SUCCESS;
    };

    let mut removed = 0usize;
    let mut failed = 0usize;
    let mut freed: u64 = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let is_dir = path.is_dir();
        let result = if is_dir {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match result {
            Ok(()) => {
                removed += 1;
                freed += size;
            }
            Err(error) => {
                eprintln!("Failed to remove {}: {error}", path.display());
                failed += 1;
            }
        }
    }

    eprintln!(
        "Removed {removed} debug frame(s) ({}) from {}",
        format_bytes(freed),
        debug_dir.display()
    );
    if failed > 0 {
        eprintln!("{failed} entr(y/ies) could not be removed");
        return EXIT_AUTH_ERR;
    }
    EXIT_SUCCESS
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[(&str, u64)] = &[
        ("GiB", 1024 * 1024 * 1024),
        ("MiB", 1024 * 1024),
        ("KiB", 1024),
    ];
    for (unit, factor) in UNITS {
        if bytes >= *factor {
            return format!("{:.2} {unit}", bytes as f64 / *factor as f64);
        }
    }
    format!("{bytes} B")
}
