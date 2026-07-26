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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_attempt_outcome_builds_method_summary() {
        let outcome =
            FaceAuthAttemptOutcome::new(AuthResult::Success, "face_matched", "face matched");

        let method_outcome = outcome.into_method_outcome();

        assert_eq!(method_outcome.result, AuthResult::Success);
        assert_eq!(method_outcome.summary.method, "face");
        assert_eq!(
            method_outcome.summary.result,
            AuthMethodSummaryResult::Success
        );
        assert_eq!(
            method_outcome.summary.reason.as_deref(),
            Some("face_matched")
        );
        assert_eq!(method_outcome.summary.message, "face matched");
        assert_eq!(method_outcome.summary.attempts.len(), 1);
        assert_eq!(method_outcome.summary.attempts[0].attempt, 1);
        assert_eq!(
            method_outcome.summary.attempts[0].result,
            AuthMethodSummaryResult::Success
        );
    }

    #[test]
    fn anti_spoofing_outcome_constructors_record_state() {
        let passed = AntiSpoofingCheckOutcome::passed();

        assert!(passed.passed);
        assert!(passed.reason.is_none());
        assert!(passed.message.is_none());
        assert!(passed.ir.is_none());

        let ir = IrLivenessSummary {
            frames: 3,
            passed_frames: 1,
            required_passes: 2,
            last_failure: Some("ir_face_mismatch".to_string()),
            highest_detection_confidence: Some(0.8),
        };
        let failed = AntiSpoofingCheckOutcome::failed("spoof", "spoof detected").with_ir(ir);

        assert!(!failed.passed);
        assert_eq!(failed.reason.as_deref(), Some("spoof"));
        assert_eq!(failed.message.as_deref(), Some("spoof detected"));
        assert_eq!(failed.ir.unwrap().passed_frames, 1);
    }

    #[test]
    fn method_summary_result_maps_all_auth_results() {
        assert_eq!(
            method_summary_result(AuthResult::Success),
            AuthMethodSummaryResult::Success
        );
        assert_eq!(
            method_summary_result(AuthResult::Failure),
            AuthMethodSummaryResult::Failure
        );
        assert_eq!(
            method_summary_result(AuthResult::Retry),
            AuthMethodSummaryResult::Retry
        );
        assert_eq!(
            method_summary_result(AuthResult::Unavailable),
            AuthMethodSummaryResult::Unavailable
        );
    }
}
