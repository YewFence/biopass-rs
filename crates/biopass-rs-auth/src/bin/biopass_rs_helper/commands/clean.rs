use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use biopass_rs_auth::{user_data_dir, user_exists};
use std::path::Path;

pub(crate) fn run(username: &str) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }

    clean_debug_dir(username, &user_data_dir(username).join("debugs"))
}

fn clean_debug_dir(username: &str, debug_dir: &Path) -> u8 {
    let Ok(entries) = std::fs::read_dir(debug_dir) else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn format_bytes_uses_largest_binary_unit() {
        assert_eq!(format_bytes(42), "42 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.00 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GiB");
    }

    #[test]
    fn missing_debug_directory_is_successful_noop() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(
            clean_debug_dir("alice", &directory.path().join("debugs")),
            EXIT_SUCCESS
        );
    }

    #[test]
    fn removes_debug_files_and_directories() {
        let directory = tempfile::tempdir().unwrap();
        let debug_dir = directory.path().join("debugs");
        let nested_dir = debug_dir.join("nested");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(debug_dir.join("frame.jpg"), [1_u8, 2, 3]).unwrap();
        fs::write(nested_dir.join("trace.txt"), "debug").unwrap();

        assert_eq!(clean_debug_dir("alice", &debug_dir), EXIT_SUCCESS);
        assert_eq!(fs::read_dir(&debug_dir).unwrap().count(), 0);
    }
}
