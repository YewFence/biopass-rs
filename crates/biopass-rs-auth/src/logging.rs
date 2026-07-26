use crate::{LogLevel, LoggingConfig};
use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

static RUNTIME_LOGGING: OnceLock<RwLock<RuntimeLoggingConfig>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogComponent {
    Auth,
    Helper,
    Desktop,
}

impl LogComponent {
    pub fn as_dir(self) -> &'static str {
        match self {
            LogComponent::Auth => "auth",
            LogComponent::Helper => "helper",
            LogComponent::Desktop => "desktop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeLoggingConfig {
    pub data_dir: PathBuf,
    pub file_enabled: bool,
    pub file_level: LogLevel,
    pub console_enabled: bool,
    pub console_level: LogLevel,
    pub max_size_bytes: u64,
    pub max_files_per_day: u32,
    pub save_failed_frames: bool,
    pub auth_history_enabled: bool,
}

impl RuntimeLoggingConfig {
    pub fn from_config(data_dir: PathBuf, config: &LoggingConfig) -> Self {
        Self {
            data_dir,
            file_enabled: config.file.enabled,
            file_level: parse_log_level(&config.file.level).unwrap_or(LogLevel::Info),
            console_enabled: config.console.enabled,
            console_level: parse_log_level(&config.console.level).unwrap_or(LogLevel::Warn),
            max_size_bytes: u64::from(config.file.rotation.max_size_mb.max(1)) * 1024 * 1024,
            max_files_per_day: config.file.rotation.max_files_per_day.max(1),
            save_failed_frames: config.diagnostics.save_failed_frames,
            auth_history_enabled: config.auth_history.enabled,
        }
    }
}

impl Default for RuntimeLoggingConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("."),
            file_enabled: false,
            file_level: LogLevel::Info,
            console_enabled: false,
            console_level: LogLevel::Warn,
            max_size_bytes: 10 * 1024 * 1024,
            max_files_per_day: 5,
            save_failed_frames: false,
            auth_history_enabled: false,
        }
    }
}

pub fn set_runtime_logging(config: RuntimeLoggingConfig) {
    let lock = RUNTIME_LOGGING.get_or_init(|| RwLock::new(RuntimeLoggingConfig::default()));
    if let Ok(mut guard) = lock.write() {
        *guard = config;
    }
}

pub fn runtime_logging() -> RuntimeLoggingConfig {
    RUNTIME_LOGGING
        .get_or_init(|| RwLock::new(RuntimeLoggingConfig::default()))
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn emit_log(component: LogComponent, level: LogLevel, scope: &str, message: &str) {
    let config = runtime_logging();
    let timestamp = Local::now();
    if config.console_enabled && level >= config.console_level {
        eprintln!(
            "[{}] [biopass-rs] [{}] {}: {}",
            timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            level.as_prefix(),
            scope,
            message
        );
    }
    if config.file_enabled && level >= config.file_level {
        let line = format!(
            "{} {} {} {}\n",
            timestamp.to_rfc3339(),
            level.as_prefix(),
            scope,
            message.replace('\n', "\\n")
        );
        if let Ok(path) = log_file_path_with_rotation(&config, component, timestamp) {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}

pub fn save_failed_frames_enabled() -> bool {
    runtime_logging().save_failed_frames
}

pub fn log_file_path(component: LogComponent) -> PathBuf {
    let config = runtime_logging();
    let date = Local::now().format("%Y-%m-%d").to_string();
    config
        .data_dir
        .join("logs")
        .join(component.as_dir())
        .join(format!("{date}.log"))
}

pub fn read_log_tail(component: LogComponent, max_lines: usize) -> Result<Vec<String>, String> {
    let path = log_file_path(component);
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to read {}: {error}", path.display())),
    };
    let reader = BufReader::new(file);
    let mut lines = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    if lines.len() > max_lines {
        lines.drain(0..lines.len() - max_lines);
    }
    Ok(lines)
}

fn log_file_path_with_rotation(
    config: &RuntimeLoggingConfig,
    component: LogComponent,
    timestamp: DateTime<Local>,
) -> Result<PathBuf, String> {
    let date = timestamp.format("%Y-%m-%d").to_string();
    let dir = config.data_dir.join("logs").join(component.as_dir());
    let base = dir.join(format!("{date}.log"));
    if !file_exceeds_size(&base, config.max_size_bytes) {
        return Ok(base);
    }

    for index in 1..config.max_files_per_day {
        let path = dir.join(format!("{date}.{index}.log"));
        if !file_exceeds_size(&path, config.max_size_bytes) {
            return Ok(path);
        }
    }
    Ok(dir.join(format!(
        "{date}.{}.log",
        config.max_files_per_day.saturating_sub(1)
    )))
}

fn file_exceeds_size(path: &Path, max_size_bytes: u64) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.len() >= max_size_bytes)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthSessionSummary {
    pub id: String,
    pub started_at: String,
    pub finished_at: String,
    pub service: Option<String>,
    pub result: AuthSummaryResult,
    pub execution_mode: String,
    pub methods: Vec<AuthMethodSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthSummaryResult {
    Success,
    Failure,
    Ignored,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthMethodSummary {
    pub method: String,
    pub result: AuthMethodSummaryResult,
    pub attempts: Vec<AuthAttemptSummary>,
    pub reason: Option<String>,
    pub message: String,
    pub best_match: Option<FaceBestMatchSummary>,
    pub ir: Option<IrLivenessSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethodSummaryResult {
    Success,
    Failure,
    Retry,
    Unavailable,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthAttemptSummary {
    pub attempt: u32,
    pub result: AuthMethodSummaryResult,
    pub reason: Option<String>,
    pub message: String,
    pub best_match: Option<FaceBestMatchSummary>,
    pub ir: Option<IrLivenessSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FaceBestMatchSummary {
    pub enrolled_index: usize,
    pub similarity: f32,
    pub threshold: f32,
    pub similar: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IrLivenessSummary {
    pub frames: usize,
    pub passed_frames: usize,
    pub required_passes: usize,
    pub last_failure: Option<String>,
    pub highest_detection_confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuthSummary {
    #[serde(flatten)]
    pub summary: AuthSessionSummary,
}

pub fn write_auth_summary(summary: &AuthSessionSummary) -> Result<(), String> {
    let config = runtime_logging();
    if !config.auth_history_enabled {
        return Ok(());
    }
    let path = auth_history_file_path_for_now(&config.data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }
    let line = serde_json::to_string(summary)
        .map_err(|error| format!("Failed to serialize auth summary: {error}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Failed to open {}: {error}", path.display()))?;
    writeln!(file, "{line}").map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

pub fn read_auth_history(limit: usize) -> Result<Vec<AuthSessionSummary>, String> {
    let config = runtime_logging();
    let dir = config.data_dir.join("auth-history");
    let mut files = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to read {}: {error}", dir.display())),
    };
    files.sort_by(|a, b| compare_modified_desc(a, b));

    let mut summaries = Vec::new();
    for path in files {
        let file = fs::File::open(&path)
            .map_err(|error| format!("Failed to open {}: {error}", path.display()))?;
        let reader = BufReader::new(file);
        let mut file_summaries = Vec::new();
        for line in reader.lines() {
            let line =
                line.map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(summary) = serde_json::from_str::<AuthSessionSummary>(&line) {
                file_summaries.push(summary);
            }
        }
        file_summaries.reverse();
        summaries.extend(file_summaries);
        if summaries.len() >= limit {
            summaries.truncate(limit);
            break;
        }
    }
    Ok(summaries)
}

pub fn auth_history_dir() -> PathBuf {
    runtime_logging().data_dir.join("auth-history")
}

pub fn logs_dir() -> PathBuf {
    runtime_logging().data_dir.join("logs")
}

pub fn make_auth_summary_id() -> String {
    let now = Local::now();
    format!("{}-{}", now.format("%Y%m%dT%H%M%S%3f"), std::process::id())
}

fn auth_history_file_path_for_now(data_dir: &Path) -> PathBuf {
    let now = Local::now();
    data_dir
        .join("auth-history")
        .join(format!("{:04}-{:02}.jsonl", now.year(), now.month()))
}

fn compare_modified_desc(a: &Path, b: &Path) -> Ordering {
    let a_modified = fs::metadata(a)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let b_modified = fs::metadata(b)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    b_modified.cmp(&a_modified)
}

fn parse_log_level(level: &str) -> Option<LogLevel> {
    match level {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}
