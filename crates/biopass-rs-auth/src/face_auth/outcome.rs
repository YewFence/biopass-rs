use crate::{
    AuthAttemptSummary, AuthMethodSummaryResult, AuthResult, FaceBestMatchSummary,
    IrLivenessSummary, MethodAuthOutcome,
};

#[derive(Debug, Clone)]
pub(super) struct FaceAuthAttemptOutcome {
    pub(super) result: AuthResult,
    pub(super) reason: Option<String>,
    pub(super) message: String,
    pub(super) best_match: Option<FaceBestMatchSummary>,
    pub(super) ir: Option<IrLivenessSummary>,
}

impl FaceAuthAttemptOutcome {
    pub(super) fn new(result: AuthResult, reason: &str, message: &str) -> Self {
        Self {
            result,
            reason: Some(reason.to_string()),
            message: message.to_string(),
            best_match: None,
            ir: None,
        }
    }

    pub(super) fn into_method_outcome(self) -> MethodAuthOutcome {
        let summary_result = method_summary_result(self.result);
        let attempt = AuthAttemptSummary {
            attempt: 1,
            result: summary_result.clone(),
            reason: self.reason.clone(),
            message: self.message.clone(),
            best_match: self.best_match,
            ir: self.ir.clone(),
        };
        MethodAuthOutcome {
            result: self.result,
            summary: crate::AuthMethodSummary {
                method: "face".to_string(),
                result: summary_result,
                attempts: vec![attempt],
                reason: self.reason,
                message: self.message,
                best_match: self.best_match,
                ir: self.ir,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AntiSpoofingCheckOutcome {
    pub(super) passed: bool,
    pub(super) reason: Option<String>,
    pub(super) message: Option<String>,
    pub(super) ir: Option<IrLivenessSummary>,
}

impl AntiSpoofingCheckOutcome {
    pub(super) fn passed() -> Self {
        Self {
            passed: true,
            reason: None,
            message: None,
            ir: None,
        }
    }

    pub(super) fn failed(reason: &str, message: &str) -> Self {
        Self {
            passed: false,
            reason: Some(reason.to_string()),
            message: Some(message.to_string()),
            ir: None,
        }
    }

    pub(super) fn with_ir(mut self, ir: IrLivenessSummary) -> Self {
        self.ir = Some(ir);
        self
    }
}

pub(super) fn method_summary_result(result: AuthResult) -> AuthMethodSummaryResult {
    match result {
        AuthResult::Success => AuthMethodSummaryResult::Success,
        AuthResult::Failure => AuthMethodSummaryResult::Failure,
        AuthResult::Retry => AuthMethodSummaryResult::Retry,
        AuthResult::Unavailable => AuthMethodSummaryResult::Unavailable,
    }
}
