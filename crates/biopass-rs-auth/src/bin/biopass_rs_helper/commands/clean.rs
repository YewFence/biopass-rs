use super::auth::{EXIT_AUTH_ERR, EXIT_SUCCESS};
use crate::cli::CleanTargetArg;
use biopass_rs_auth::{
    cleanup_data_dir, config_path, read_config_from_path, user_data_dir, user_exists, CleanupMode,
    CleanupOptions, CleanupReport, CleanupSectionReport, CleanupTarget,
};

pub(crate) fn run(username: &str, target: CleanTargetArg, all: bool, dry_run: bool) -> u8 {
    if !user_exists(username) {
        eprintln!("User '{username}' not found");
        return EXIT_AUTH_ERR;
    }

    let config_path = config_path(username);
    let config = match read_config_from_path(&config_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("clean: {error}");
            return EXIT_AUTH_ERR;
        }
    };
    let data_dir = user_data_dir(username);
    let options = CleanupOptions {
        target: target.into(),
        mode: if all {
            CleanupMode::All
        } else {
            CleanupMode::Retention
        },
        dry_run,
    };

    let report = cleanup_data_dir(&data_dir, &config, options);
    print_report(&report, dry_run);
    if report.has_failures() {
        EXIT_AUTH_ERR
    } else {
        EXIT_SUCCESS
    }
}

impl From<CleanTargetArg> for CleanupTarget {
    fn from(value: CleanTargetArg) -> Self {
        match value {
            CleanTargetArg::All => CleanupTarget::All,
            CleanTargetArg::Debugs => CleanupTarget::Debugs,
            CleanTargetArg::Logs => CleanupTarget::Logs,
            CleanTargetArg::AuthHistory => CleanupTarget::AuthHistory,
        }
    }
}

fn print_report(report: &CleanupReport, dry_run: bool) {
    let action = if dry_run { "Would remove" } else { "Removed" };
    eprintln!(
        "{action} {} entr(y/ies) ({})",
        report.removed_entries(),
        format_bytes(report.freed_bytes())
    );

    for section in &report.sections {
        print_section(section, dry_run);
    }
}

fn print_section(section: &CleanupSectionReport, dry_run: bool) {
    if section.missing {
        eprintln!("{}: missing at {}", section.name, section.path.display());
        return;
    }

    let action = if dry_run { "would remove" } else { "removed" };
    eprintln!(
        "{}: scanned {}, {action} {} ({}) from {}",
        section.name,
        section.scanned_entries,
        section.removed_entries,
        format_bytes(section.freed_bytes),
        section.path.display()
    );

    for failure in &section.failures {
        eprintln!(
            "Failed to remove {}: {}",
            failure.path.display(),
            failure.error
        );
    }
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

    #[test]
    fn format_bytes_uses_largest_binary_unit() {
        assert_eq!(format_bytes(42), "42 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.00 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GiB");
    }

    #[test]
    fn maps_cli_target_to_cleanup_target() {
        assert_eq!(CleanupTarget::from(CleanTargetArg::All), CleanupTarget::All);
        assert_eq!(
            CleanupTarget::from(CleanTargetArg::Debugs),
            CleanupTarget::Debugs
        );
        assert_eq!(
            CleanupTarget::from(CleanTargetArg::Logs),
            CleanupTarget::Logs
        );
        assert_eq!(
            CleanupTarget::from(CleanTargetArg::AuthHistory),
            CleanupTarget::AuthHistory
        );
    }
}
