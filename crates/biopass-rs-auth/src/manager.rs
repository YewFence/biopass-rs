use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use chrono::Local;

use crate::{
    emit_log, make_auth_summary_id, AuthAttemptSummary, AuthMethodSummary, AuthMethodSummaryResult,
    AuthSessionSummary, AuthSummaryResult, LogComponent, LogLevel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthResult {
    Success,
    Failure,
    Retry,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PamCode {
    Success,
    AuthError,
    Ignore,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthConfig {
    pub debug: bool,
    pub antispoof: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthOutcome {
    pub code: PamCode,
    pub attempted: bool,
    pub summary: AuthSessionSummary,
}

pub trait AuthMethod: Send {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn retries(&self) -> u32;
    fn retry_delay_ms(&self) -> u32;
    fn begin_authentication_session(&mut self) {}
    fn end_authentication_session(&mut self) {}
    fn authenticate(
        &mut self,
        username: &str,
        config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
    ) -> MethodAuthOutcome;
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodAuthOutcome {
    pub result: AuthResult,
    pub summary: AuthMethodSummary,
}

impl MethodAuthOutcome {
    pub fn basic(method: &str, result: AuthResult, message: impl Into<String>) -> Self {
        let summary_result = method_summary_result(result);
        let message = message.into();
        Self {
            result,
            summary: AuthMethodSummary {
                method: method.to_string(),
                result: summary_result.clone(),
                attempts: vec![AuthAttemptSummary {
                    attempt: 1,
                    result: summary_result,
                    reason: None,
                    message: message.clone(),
                    best_match: None,
                    ir: None,
                }],
                reason: None,
                message,
                best_match: None,
                ir: None,
            },
        }
    }
}

pub struct AuthManager {
    mode: ExecutionMode,
    config: AuthConfig,
    methods: Vec<Box<dyn AuthMethod>>,
}

impl Default for AuthManager {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::Parallel,
            config: AuthConfig::default(),
            methods: Vec::new(),
        }
    }
}

impl AuthManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_mode(&mut self, mode: ExecutionMode) {
        self.mode = mode;
    }

    pub fn set_config(&mut self, config: AuthConfig) {
        self.config = config;
    }

    pub fn add_method(&mut self, method: Box<dyn AuthMethod>) {
        self.methods.push(method);
    }

    pub fn authenticate(&mut self, username: &str) -> AuthOutcome {
        let started_at = Local::now();
        if self.methods.is_empty() {
            return AuthOutcome {
                code: PamCode::Ignore,
                attempted: false,
                summary: session_summary(
                    started_at,
                    AuthSummaryResult::Ignored,
                    self.mode,
                    Vec::new(),
                    None,
                ),
            };
        }

        match self.mode {
            ExecutionMode::Sequential => self.run_sequential(username, started_at),
            ExecutionMode::Parallel => self.run_parallel(username, started_at),
        }
    }

    fn run_sequential(
        &mut self,
        username: &str,
        started_at: chrono::DateTime<Local>,
    ) -> AuthOutcome {
        let mut any_attempted = false;
        let mut summaries = Vec::new();

        for method in &mut self.methods {
            if !method.is_available() {
                continue;
            }

            let method_name = auth_method_log_name(method.name()).to_string();
            emit_log(
                LogComponent::Auth,
                LogLevel::Debug,
                "AuthManager",
                &format!("Starting {method_name} authentication (sequential)"),
            );
            method.begin_authentication_session();
            let outcome = authenticate_with_retries(method.as_mut(), username, &self.config, None);
            method.end_authentication_session();
            emit_log(
                LogComponent::Auth,
                LogLevel::Debug,
                "AuthManager",
                &format!(
                    "{method_name} authentication finished with {:?} (sequential)",
                    outcome.result
                ),
            );

            let result = outcome.result;
            summaries.push(outcome.summary);
            match result {
                AuthResult::Success => {
                    return AuthOutcome {
                        code: PamCode::Success,
                        attempted: true,
                        summary: session_summary(
                            started_at,
                            AuthSummaryResult::Success,
                            self.mode,
                            summaries,
                            None,
                        ),
                    };
                }
                AuthResult::Unavailable => {}
                AuthResult::Failure | AuthResult::Retry => any_attempted = true,
            }
        }

        if any_attempted {
            AuthOutcome {
                code: PamCode::AuthError,
                attempted: true,
                summary: session_summary(
                    started_at,
                    AuthSummaryResult::Failure,
                    self.mode,
                    summaries,
                    None,
                ),
            }
        } else {
            AuthOutcome {
                code: PamCode::Ignore,
                attempted: false,
                summary: session_summary(
                    started_at,
                    AuthSummaryResult::Ignored,
                    self.mode,
                    summaries,
                    None,
                ),
            }
        }
    }

    fn run_parallel(&mut self, username: &str, started_at: chrono::DateTime<Local>) -> AuthOutcome {
        let cancel_signal = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();

        for mut method in self.methods.drain(..) {
            if !method.is_available() {
                continue;
            }

            let username = username.to_string();
            let config = self.config;
            let cancel_signal = Arc::clone(&cancel_signal);
            handles.push(thread::spawn(move || {
                let method_name = auth_method_log_name(method.name()).to_string();
                emit_log(
                    LogComponent::Auth,
                    LogLevel::Debug,
                    "AuthManager",
                    &format!("Starting {method_name} authentication (parallel)"),
                );
                method.begin_authentication_session();
                let outcome = authenticate_with_retries(
                    method.as_mut(),
                    &username,
                    &config,
                    Some(cancel_signal.as_ref()),
                );
                method.end_authentication_session();
                if outcome.result == AuthResult::Success {
                    cancel_signal.store(true, Ordering::SeqCst);
                }
                emit_log(
                    LogComponent::Auth,
                    LogLevel::Debug,
                    "AuthManager",
                    &format!(
                        "{method_name} authentication finished with {:?} (parallel)",
                        outcome.result
                    ),
                );
                outcome
            }));
        }

        let mut any_success = false;
        let mut any_attempted = false;
        let mut summaries = Vec::new();

        for handle in handles {
            let outcome = handle.join().unwrap_or_else(|_| {
                MethodAuthOutcome::basic(
                    "unknown",
                    AuthResult::Failure,
                    "authentication thread panicked",
                )
            });
            let result = outcome.result;
            if result == AuthResult::Success {
                any_success = true;
            } else if result != AuthResult::Unavailable {
                any_attempted = true;
            }
            summaries.push(outcome.summary);
        }

        if any_success {
            AuthOutcome {
                code: PamCode::Success,
                attempted: true,
                summary: session_summary(
                    started_at,
                    AuthSummaryResult::Success,
                    self.mode,
                    summaries,
                    None,
                ),
            }
        } else if any_attempted {
            AuthOutcome {
                code: PamCode::AuthError,
                attempted: true,
                summary: session_summary(
                    started_at,
                    AuthSummaryResult::Failure,
                    self.mode,
                    summaries,
                    None,
                ),
            }
        } else {
            AuthOutcome {
                code: PamCode::Ignore,
                attempted: false,
                summary: session_summary(
                    started_at,
                    AuthSummaryResult::Ignored,
                    self.mode,
                    summaries,
                    None,
                ),
            }
        }
    }
}

fn auth_method_log_name(name: &str) -> &str {
    match name {
        "face" => "Face",
        "fingerprint" => "Fingerprint",
        other => other,
    }
}

fn authenticate_with_retries(
    method: &mut dyn AuthMethod,
    username: &str,
    config: &AuthConfig,
    cancel_signal: Option<&AtomicBool>,
) -> MethodAuthOutcome {
    let max_attempts = method.retries().max(1);
    let mut attempts = 0;
    let mut merged_attempts = Vec::new();

    loop {
        if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
            let mut outcome = MethodAuthOutcome::basic(
                method.name(),
                AuthResult::Failure,
                "authentication cancelled",
            );
            outcome.summary.result = AuthMethodSummaryResult::Cancelled;
            outcome.summary.reason = Some("cancelled".to_string());
            outcome.summary.message = "authentication cancelled".to_string();
            outcome.summary.attempts = merged_attempts;
            return outcome;
        }

        if attempts > 0 {
            thread::sleep(Duration::from_millis(method.retry_delay_ms().into()));
        }

        let mut outcome = method.authenticate(username, config, cancel_signal);
        attempts += 1;
        if let Some(attempt) = outcome.summary.attempts.first_mut() {
            attempt.attempt = attempts;
        }
        merged_attempts.extend(outcome.summary.attempts.clone());

        if outcome.result != AuthResult::Retry || attempts >= max_attempts {
            outcome.summary.attempts = merged_attempts;
            return outcome;
        }
    }
}

fn session_summary(
    started_at: chrono::DateTime<Local>,
    result: AuthSummaryResult,
    mode: ExecutionMode,
    methods: Vec<AuthMethodSummary>,
    service: Option<String>,
) -> AuthSessionSummary {
    AuthSessionSummary {
        id: make_auth_summary_id(),
        started_at: started_at.to_rfc3339(),
        finished_at: Local::now().to_rfc3339(),
        service,
        result,
        execution_mode: match mode {
            ExecutionMode::Sequential => "sequential".to_string(),
            ExecutionMode::Parallel => "parallel".to_string(),
        },
        methods,
    }
}

fn method_summary_result(result: AuthResult) -> AuthMethodSummaryResult {
    match result {
        AuthResult::Success => AuthMethodSummaryResult::Success,
        AuthResult::Failure => AuthMethodSummaryResult::Failure,
        AuthResult::Retry => AuthMethodSummaryResult::Retry,
        AuthResult::Unavailable => AuthMethodSummaryResult::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FakeMethod {
        name: String,
        available: bool,
        retries: u32,
        retry_delay_ms: u32,
        results: Vec<AuthResult>,
        attempts: Arc<Mutex<Vec<String>>>,
    }

    impl FakeMethod {
        fn new(name: &str, results: Vec<AuthResult>, attempts: Arc<Mutex<Vec<String>>>) -> Self {
            Self {
                name: name.to_string(),
                available: true,
                retries: 1,
                retry_delay_ms: 0,
                results,
                attempts,
            }
        }
    }

    impl AuthMethod for FakeMethod {
        fn name(&self) -> &str {
            &self.name
        }

        fn is_available(&self) -> bool {
            self.available
        }

        fn retries(&self) -> u32 {
            self.retries
        }

        fn retry_delay_ms(&self) -> u32 {
            self.retry_delay_ms
        }

        fn authenticate(
            &mut self,
            _username: &str,
            _config: &AuthConfig,
            _cancel_signal: Option<&AtomicBool>,
        ) -> MethodAuthOutcome {
            self.attempts.lock().unwrap().push(self.name.clone());
            let result = if self.results.len() > 1 {
                self.results.remove(0)
            } else {
                self.results[0]
            };
            MethodAuthOutcome::basic(
                &self.name,
                result,
                format!("{} returned {:?}", self.name, result),
            )
        }
    }

    #[test]
    fn sequential_succeeds_on_first_successful_method() {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let mut manager = AuthManager::new();
        manager.set_mode(ExecutionMode::Sequential);
        manager.add_method(Box::new(FakeMethod::new(
            "face",
            vec![AuthResult::Failure],
            Arc::clone(&attempts),
        )));
        manager.add_method(Box::new(FakeMethod::new(
            "fingerprint",
            vec![AuthResult::Success],
            Arc::clone(&attempts),
        )));

        let outcome = manager.authenticate("alice");

        assert_eq!(outcome.code, PamCode::Success);
        assert_eq!(*attempts.lock().unwrap(), ["face", "fingerprint"]);
    }

    #[test]
    fn sequential_ignores_when_no_available_method_runs() {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let mut method = FakeMethod::new("face", vec![AuthResult::Success], attempts);
        method.available = false;
        let mut manager = AuthManager::new();
        manager.set_mode(ExecutionMode::Sequential);
        manager.add_method(Box::new(method));

        let outcome = manager.authenticate("alice");

        assert_eq!(outcome.code, PamCode::Ignore);
        assert!(!outcome.attempted);
    }

    #[test]
    fn retry_result_repeats_until_limit() {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let mut method = FakeMethod::new(
            "face",
            vec![AuthResult::Retry, AuthResult::Success],
            Arc::clone(&attempts),
        );
        method.retries = 2;
        let mut manager = AuthManager::new();
        manager.set_mode(ExecutionMode::Sequential);
        manager.add_method(Box::new(method));

        let outcome = manager.authenticate("alice");

        assert_eq!(outcome.code, PamCode::Success);
        assert_eq!(attempts.lock().unwrap().len(), 2);
    }

    #[test]
    fn parallel_succeeds_when_any_method_succeeds() {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let mut manager = AuthManager::new();
        manager.set_mode(ExecutionMode::Parallel);
        manager.add_method(Box::new(FakeMethod::new(
            "face",
            vec![AuthResult::Failure],
            Arc::clone(&attempts),
        )));
        manager.add_method(Box::new(FakeMethod::new(
            "fingerprint",
            vec![AuthResult::Success],
            Arc::clone(&attempts),
        )));

        let outcome = manager.authenticate("alice");

        assert_eq!(outcome.code, PamCode::Success);
        assert!(outcome.attempted);
    }
}
