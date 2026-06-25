import type {
  ActivityCleanupMode,
  ActivityCleanupTarget,
  ActivityLogComponent,
  AuthSessionSummary,
  AuthTestResult,
  CleanupReport,
} from "@/types/activity";
import type { LogLevelName } from "@/types/config";
import { invokeCommand } from "./core";

function listAuthHistory(limit = 100) {
  return invokeCommand<AuthSessionSummary[]>("list_auth_history", { limit });
}

function readLogTail(component: ActivityLogComponent, maxLines = 500) {
  return invokeCommand<string[]>("read_activity_log_tail", {
    component,
    maxLines,
  });
}

function logFilePath(component: ActivityLogComponent) {
  return invokeCommand<string>("activity_log_file_path", { component });
}

function logsDir() {
  return invokeCommand<string>("activity_logs_dir");
}

function authHistoryDir() {
  return invokeCommand<string>("auth_history_dir_path");
}

function testAuthFlow(service = "sudo", logLevel: LogLevelName = "debug") {
  return invokeCommand<AuthTestResult>("test_auth_flow", { service, logLevel });
}

function cleanActivityData(
  target: ActivityCleanupTarget,
  mode: ActivityCleanupMode,
  dryRun = false,
) {
  return invokeCommand<CleanupReport>("clean_activity_data", {
    target,
    mode,
    dryRun,
  });
}

export const activity = {
  listAuthHistory,
  readLogTail,
  logFilePath,
  logsDir,
  authHistoryDir,
  testAuthFlow,
  cleanActivityData,
};
