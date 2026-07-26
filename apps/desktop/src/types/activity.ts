export type AuthSummaryResult = "success" | "failure" | "ignored";

export type AuthMethodSummaryResult = "success" | "failure" | "retry" | "unavailable" | "cancelled";

export interface AuthSessionSummary {
  id: string;
  started_at: string;
  finished_at: string;
  service: string | null;
  result: AuthSummaryResult;
  execution_mode: "sequential" | "parallel";
  methods: AuthMethodSummary[];
}

export interface AuthMethodSummary {
  method: string;
  result: AuthMethodSummaryResult;
  attempts: AuthAttemptSummary[];
  reason: string | null;
  message: string;
  best_match: FaceBestMatchSummary | null;
  ir: IrLivenessSummary | null;
}

export interface AuthAttemptSummary {
  attempt: number;
  result: AuthMethodSummaryResult;
  reason: string | null;
  message: string;
  best_match: FaceBestMatchSummary | null;
  ir: IrLivenessSummary | null;
}

export interface FaceBestMatchSummary {
  enrolled_index: number;
  similarity: number;
  threshold: number;
  similar: boolean;
}

export interface IrLivenessSummary {
  frames: number;
  passed_frames: number;
  required_passes: number;
  last_failure: string | null;
  highest_detection_confidence: number | null;
}

export type ActivityLogComponent = "auth" | "helper" | "desktop";

export type ActivityCleanupTarget = "all" | "debugs" | "logs" | "auth_history";

export type ActivityCleanupMode = "retention" | "all";

export interface CleanupFailure {
  path: string;
  error: string;
}

export interface CleanupSectionReport {
  name: string;
  path: string;
  scanned_entries: number;
  removed_entries: number;
  failed_entries: number;
  freed_bytes: number;
  missing: boolean;
  failures: CleanupFailure[];
}

export interface CleanupReport {
  sections: CleanupSectionReport[];
}

export type PamCode = "success" | "auth_error" | "ignore";

export type AuthTestResult =
  | {
      status: "completed";
      pam_code: PamCode;
    }
  | {
      status: "ignored";
    };
