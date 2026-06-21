use crate::{BiopassConfig, LogComponent};
use chrono::{Duration, Local, NaiveDate};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupTarget {
    All,
    Debugs,
    Logs,
    AuthHistory,
}

impl CleanupTarget {
    fn includes_debugs(self) -> bool {
        matches!(self, CleanupTarget::All | CleanupTarget::Debugs)
    }

    fn includes_logs(self) -> bool {
        matches!(self, CleanupTarget::All | CleanupTarget::Logs)
    }

    fn includes_auth_history(self) -> bool {
        matches!(self, CleanupTarget::All | CleanupTarget::AuthHistory)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupMode {
    Retention,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupOptions {
    pub target: CleanupTarget,
    pub mode: CleanupMode,
    pub dry_run: bool,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            target: CleanupTarget::All,
            mode: CleanupMode::Retention,
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub sections: Vec<CleanupSectionReport>,
}

impl CleanupReport {
    pub fn removed_entries(&self) -> usize {
        self.sections
            .iter()
            .map(|section| section.removed_entries)
            .sum()
    }

    pub fn failed_entries(&self) -> usize {
        self.sections
            .iter()
            .map(|section| section.failed_entries)
            .sum()
    }

    pub fn freed_bytes(&self) -> u64 {
        self.sections
            .iter()
            .map(|section| section.freed_bytes)
            .sum()
    }

    pub fn has_failures(&self) -> bool {
        self.failed_entries() > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupSectionReport {
    pub name: &'static str,
    pub path: PathBuf,
    pub scanned_entries: usize,
    pub removed_entries: usize,
    pub failed_entries: usize,
    pub freed_bytes: u64,
    pub missing: bool,
    pub failures: Vec<CleanupFailure>,
}

impl CleanupSectionReport {
    fn new(name: &'static str, path: PathBuf) -> Self {
        Self {
            name,
            path,
            scanned_entries: 0,
            removed_entries: 0,
            failed_entries: 0,
            freed_bytes: 0,
            missing: false,
            failures: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupFailure {
    pub path: PathBuf,
    pub error: String,
}

pub fn cleanup_data_dir(
    data_dir: &Path,
    config: &BiopassConfig,
    options: CleanupOptions,
) -> CleanupReport {
    let mut sections = Vec::new();
    if options.target.includes_debugs() {
        sections.push(clean_debugs(data_dir, config, options));
    }
    if options.target.includes_logs() {
        sections.extend(clean_logs(data_dir, config, options));
    }
    if options.target.includes_auth_history() {
        sections.push(clean_auth_history(data_dir, config, options));
    }
    CleanupReport { sections }
}

fn clean_debugs(
    data_dir: &Path,
    config: &BiopassConfig,
    options: CleanupOptions,
) -> CleanupSectionReport {
    clean_directory_entries(
        "debugs",
        data_dir.join("debugs"),
        options,
        Some(config.logging.diagnostics.retention_days),
        |path| path.is_file() || path.is_dir(),
    )
}

fn clean_logs(
    data_dir: &Path,
    config: &BiopassConfig,
    options: CleanupOptions,
) -> Vec<CleanupSectionReport> {
    [
        (
            "logs/auth",
            LogComponent::Auth,
            config.logging.file.retention.auth_days,
        ),
        (
            "logs/helper",
            LogComponent::Helper,
            config.logging.file.retention.helper_days,
        ),
        (
            "logs/desktop",
            LogComponent::Desktop,
            config.logging.file.retention.desktop_days,
        ),
    ]
    .into_iter()
    .map(|(name, component, retention_days)| {
        clean_directory_entries(
            name,
            data_dir.join("logs").join(component.as_dir()),
            options,
            Some(retention_days),
            is_log_file,
        )
    })
    .collect()
}

fn clean_auth_history(
    data_dir: &Path,
    config: &BiopassConfig,
    options: CleanupOptions,
) -> CleanupSectionReport {
    clean_directory_entries(
        "auth-history",
        data_dir.join("auth-history"),
        options,
        Some(config.logging.auth_history.retention_days),
        |path| path.extension().and_then(|ext| ext.to_str()) == Some("jsonl"),
    )
}

fn clean_directory_entries(
    name: &'static str,
    dir: PathBuf,
    options: CleanupOptions,
    retention_days: Option<u32>,
    should_consider: impl Fn(&Path) -> bool,
) -> CleanupSectionReport {
    let mut report = CleanupSectionReport::new(name, dir.clone());
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.missing = true;
            return report;
        }
        Err(error) => {
            report.failed_entries = 1;
            report.failures.push(CleanupFailure {
                path: dir,
                error: error.to_string(),
            });
            return report;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.failed_entries += 1;
                report.failures.push(CleanupFailure {
                    path: dir.clone(),
                    error: error.to_string(),
                });
                continue;
            }
        };
        let path = entry.path();
        if !should_consider(&path) {
            continue;
        }
        report.scanned_entries += 1;
        if !should_remove(&path, options.mode, retention_days) {
            continue;
        }
        let size = entry_size(&path);
        if options.dry_run {
            report.removed_entries += 1;
            report.freed_bytes += size;
            continue;
        }
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        match result {
            Ok(()) => {
                report.removed_entries += 1;
                report.freed_bytes += size;
            }
            Err(error) => {
                report.failed_entries += 1;
                report.failures.push(CleanupFailure {
                    path,
                    error: error.to_string(),
                });
            }
        }
    }

    report
}

fn should_remove(path: &Path, mode: CleanupMode, retention_days: Option<u32>) -> bool {
    match mode {
        CleanupMode::All => true,
        CleanupMode::Retention => retention_days
            .map(|days| is_older_than_retention(path, days))
            .unwrap_or(false),
    }
}

fn is_older_than_retention(path: &Path, retention_days: u32) -> bool {
    let Some(cutoff) = retention_cutoff(retention_days) else {
        return false;
    };
    entry_modified(path)
        .map(|modified| modified < cutoff)
        .unwrap_or(false)
}

fn retention_cutoff(retention_days: u32) -> Option<SystemTime> {
    let cutoff = Local::now()
        .checked_sub_signed(Duration::days(i64::from(retention_days)))?
        .into();
    Some(cutoff)
}

fn entry_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn entry_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };
    if metadata.is_dir() {
        return directory_size(path);
    }
    metadata.len()
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry_size(&entry.path()))
        .sum()
}

fn is_log_file(path: &Path) -> bool {
    if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
        return false;
    }
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(date_part) = file_name.split('.').next() else {
        return false;
    };
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthHistoryConfig, ConsoleLoggingConfig, DiagnosticsLoggingConfig, FileLoggingConfig,
        LogRetentionConfig, LogRotationConfig, LoggingConfig,
    };
    use std::io::Write;
    use std::time::{Duration as StdDuration, SystemTime};

    #[cfg(unix)]
    fn set_mtime(path: &Path, modified: SystemTime) {
        use std::os::unix::ffi::OsStrExt;
        use std::process::Command;

        let duration = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("test time should be after epoch");
        let spec = format!("@{}", duration.as_secs());
        let status = Command::new("touch")
            .arg("-d")
            .arg(spec)
            .arg(path)
            .status()
            .expect("touch should run");
        assert!(
            status.success(),
            "touch failed for {}",
            String::from_utf8_lossy(path.as_os_str().as_bytes())
        );
    }

    #[cfg(not(unix))]
    fn set_mtime(_path: &Path, _modified: SystemTime) {}

    fn config() -> BiopassConfig {
        BiopassConfig {
            logging: LoggingConfig {
                file: FileLoggingConfig {
                    enabled: true,
                    level: "info".to_string(),
                    rotation: LogRotationConfig::default(),
                    retention: LogRetentionConfig {
                        auth_days: 10,
                        helper_days: 20,
                        desktop_days: 30,
                    },
                },
                console: ConsoleLoggingConfig::default(),
                diagnostics: DiagnosticsLoggingConfig {
                    save_failed_frames: true,
                    retention_days: 7,
                },
                auth_history: AuthHistoryConfig {
                    enabled: true,
                    retention_days: 60,
                },
            },
            ..BiopassConfig::default_for_data_dir(Path::new("/tmp/biopass-rs-cleanup-test"))
        }
    }

    #[test]
    fn retention_cleanup_removes_only_old_matching_entries() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let debug_dir = data_dir.join("debugs");
        let auth_log_dir = data_dir.join("logs/auth");
        let history_dir = data_dir.join("auth-history");
        fs::create_dir_all(&debug_dir).unwrap();
        fs::create_dir_all(&auth_log_dir).unwrap();
        fs::create_dir_all(&history_dir).unwrap();

        let old = SystemTime::now() - StdDuration::from_secs(90 * 24 * 60 * 60);
        let recent = SystemTime::now();
        let old_debug = debug_dir.join("not_similar.old.jpg");
        let recent_debug = debug_dir.join("not_similar.new.jpg");
        let old_log = auth_log_dir.join("2026-01-01.log");
        let ignored_log = auth_log_dir.join("notes.txt");
        let old_history = history_dir.join("2026-01.jsonl");
        fs::write(&old_debug, [1, 2, 3]).unwrap();
        fs::write(&recent_debug, [4, 5, 6]).unwrap();
        fs::write(&old_log, [7, 8]).unwrap();
        fs::write(&ignored_log, [9]).unwrap();
        fs::write(&old_history, [10]).unwrap();
        set_mtime(&old_debug, old);
        set_mtime(&recent_debug, recent);
        set_mtime(&old_log, old);
        set_mtime(&ignored_log, old);
        set_mtime(&old_history, old);

        let report = cleanup_data_dir(data_dir, &config(), CleanupOptions::default());

        assert_eq!(report.failed_entries(), 0);
        assert!(!old_debug.exists());
        assert!(recent_debug.exists());
        assert!(!old_log.exists());
        assert!(ignored_log.exists());
        assert!(!old_history.exists());
    }

    #[test]
    fn dry_run_reports_without_removing() {
        let directory = tempfile::tempdir().unwrap();
        let debug_dir = directory.path().join("debugs");
        fs::create_dir_all(&debug_dir).unwrap();
        let old_debug = debug_dir.join("old.jpg");
        fs::write(&old_debug, [1, 2, 3, 4]).unwrap();
        set_mtime(
            &old_debug,
            SystemTime::now() - StdDuration::from_secs(30 * 24 * 60 * 60),
        );

        let report = cleanup_data_dir(
            directory.path(),
            &config(),
            CleanupOptions {
                target: CleanupTarget::Debugs,
                mode: CleanupMode::Retention,
                dry_run: true,
            },
        );

        assert_eq!(report.removed_entries(), 1);
        assert_eq!(report.freed_bytes(), 4);
        assert!(old_debug.exists());
    }

    #[test]
    fn all_mode_removes_nested_debug_directory_and_counts_file_sizes() {
        let directory = tempfile::tempdir().unwrap();
        let debug_dir = directory.path().join("debugs");
        let nested_dir = debug_dir.join("nested");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(debug_dir.join("frame.jpg"), [1, 2, 3]).unwrap();
        let mut file = fs::File::create(nested_dir.join("trace.txt")).unwrap();
        file.write_all(&[4, 5]).unwrap();

        let report = cleanup_data_dir(
            directory.path(),
            &config(),
            CleanupOptions {
                target: CleanupTarget::Debugs,
                mode: CleanupMode::All,
                dry_run: false,
            },
        );

        assert_eq!(report.removed_entries(), 2);
        assert_eq!(report.freed_bytes(), 5);
        assert_eq!(fs::read_dir(&debug_dir).unwrap().count(), 0);
    }
}
