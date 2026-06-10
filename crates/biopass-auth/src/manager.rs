use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthOutcome {
    pub code: PamCode,
    pub attempted: bool,
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
    ) -> AuthResult;
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
        if self.methods.is_empty() {
            return AuthOutcome {
                code: PamCode::Ignore,
                attempted: false,
            };
        }

        match self.mode {
            ExecutionMode::Sequential => self.run_sequential(username),
            ExecutionMode::Parallel => self.run_parallel(username),
        }
    }

    fn run_sequential(&mut self, username: &str) -> AuthOutcome {
        let mut any_attempted = false;

        for method in &mut self.methods {
            if !method.is_available() {
                continue;
            }

            method.begin_authentication_session();
            let result = authenticate_with_retries(method.as_mut(), username, &self.config, None);
            method.end_authentication_session();

            match result {
                AuthResult::Success => {
                    return AuthOutcome {
                        code: PamCode::Success,
                        attempted: true,
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
            }
        } else {
            AuthOutcome {
                code: PamCode::Ignore,
                attempted: false,
            }
        }
    }

    fn run_parallel(&mut self, username: &str) -> AuthOutcome {
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
                method.begin_authentication_session();
                let result = authenticate_with_retries(
                    method.as_mut(),
                    &username,
                    &config,
                    Some(cancel_signal.as_ref()),
                );
                method.end_authentication_session();
                if result == AuthResult::Success {
                    cancel_signal.store(true, Ordering::SeqCst);
                }
                result
            }));
        }

        let mut any_success = false;
        let mut any_attempted = false;

        for handle in handles {
            let result = handle.join().unwrap_or(AuthResult::Failure);
            if result == AuthResult::Success {
                any_success = true;
            } else if result != AuthResult::Unavailable {
                any_attempted = true;
            }
        }

        if any_success {
            AuthOutcome {
                code: PamCode::Success,
                attempted: true,
            }
        } else if any_attempted {
            AuthOutcome {
                code: PamCode::AuthError,
                attempted: true,
            }
        } else {
            AuthOutcome {
                code: PamCode::Ignore,
                attempted: false,
            }
        }
    }
}

fn authenticate_with_retries(
    method: &mut dyn AuthMethod,
    username: &str,
    config: &AuthConfig,
    cancel_signal: Option<&AtomicBool>,
) -> AuthResult {
    let max_attempts = method.retries().max(1);
    let mut attempts = 0;

    loop {
        if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
            return AuthResult::Failure;
        }

        if attempts > 0 {
            thread::sleep(Duration::from_millis(method.retry_delay_ms().into()));
        }

        let result = method.authenticate(username, config, cancel_signal);
        attempts += 1;

        if result != AuthResult::Retry || attempts >= max_attempts {
            return result;
        }
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
        ) -> AuthResult {
            self.attempts.lock().unwrap().push(self.name.clone());
            if self.results.len() > 1 {
                self.results.remove(0)
            } else {
                self.results[0]
            }
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
