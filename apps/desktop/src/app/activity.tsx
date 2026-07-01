import { createFileRoute } from "@tanstack/react-router";
import { FileText, Loader2, Play, RefreshCcw, ShieldCheck, Sparkles, Terminal } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { cmd } from "@/commands";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type {
  ActivityCleanupMode,
  ActivityCleanupTarget,
  ActivityLogComponent,
  AuthSessionSummary,
  CleanupReport,
  IrLivenessSummary,
} from "@/types/activity";
import type { LogLevelName } from "@/types/config";

export const Route = createFileRoute("/activity")({
  component: ActivityPage,
});

function ActivityPage() {
  const [history, setHistory] = useState<AuthSessionSummary[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [component, setComponent] = useState<ActivityLogComponent>("auth");
  const [loading, setLoading] = useState(false);
  const [testingAuth, setTestingAuth] = useState(false);
  const [testService, setTestService] = useState("sudo");
  const [testLogLevel, setTestLogLevel] = useState<LogLevelName>("debug");
  const [activeTab, setActiveTab] = useState("history");
  const [cleanDialogOpen, setCleanDialogOpen] = useState(false);
  const [cleanTarget, setCleanTarget] = useState<ActivityCleanupTarget>("all");
  const [cleanMode, setCleanMode] = useState<ActivityCleanupMode>("retention");
  const [cleaning, setCleaning] = useState(false);
  const [lastCleanupReport, setLastCleanupReport] = useState<CleanupReport | null>(null);

  async function loadHistory() {
    setLoading(true);
    try {
      setHistory(await cmd.activity.listAuthHistory(100));
    } catch (error) {
      toast.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  async function loadLogs(nextComponent = component) {
    setLoading(true);
    try {
      setLogs(await cmd.activity.readLogTail(nextComponent, 500));
    } catch (error) {
      toast.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadHistory();
  }, []);

  useEffect(() => {
    void loadLogs(component);
  }, [component]);

  useEffect(() => {
    if (!testingAuth) return;
    const interval = window.setInterval(() => {
      void cmd.activity
        .readLogTail("auth", 500)
        .then(setLogs)
        .catch((error) => console.debug("Live auth log refresh skipped:", error));
    }, 1000);
    return () => window.clearInterval(interval);
  }, [testingAuth]);

  async function testAuthFlow() {
    setTestingAuth(true);
    setComponent("auth");
    setActiveTab("logs");
    try {
      const service = testService.trim() || "sudo";
      const result = await cmd.activity.testAuthFlow(service, testLogLevel);
      if (result.status === "ignored") {
        toast.info("Authentication was ignored by the current configuration.");
      } else if (result.pam_code === "success") {
        toast.success("Authentication succeeded.");
      } else {
        toast.error("Authentication failed.");
      }
      await loadHistory();
      await loadLogs("auth");
    } catch (error) {
      toast.error(String(error));
    } finally {
      setTestingAuth(false);
    }
  }

  async function cleanActivityData() {
    setCleaning(true);
    try {
      const report = await cmd.activity.cleanActivityData(cleanTarget, cleanMode, false);
      setLastCleanupReport(report);
      const failures = cleanupFailedEntries(report);
      const removed = cleanupRemovedEntries(report);
      if (failures > 0) {
        toast.error(`Cleanup finished with ${failures} failed entr(y/ies).`);
      } else {
        toast.success(
          `Removed ${removed} entr(y/ies), freed ${formatBytes(cleanupFreedBytes(report))}.`,
        );
        setCleanDialogOpen(false);
      }
      await loadHistory();
      await loadLogs();
    } catch (error) {
      toast.error(String(error));
    } finally {
      setCleaning(false);
    }
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-normal">Activity</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Authentication summaries and diagnostic logs
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Dialog open={cleanDialogOpen} onOpenChange={setCleanDialogOpen}>
            <DialogTrigger asChild>
              <Button variant="outline" size="sm" disabled={cleaning || loading}>
                <Sparkles className="w-4 h-4" />
                Clean
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Clean Activity Data</DialogTitle>
                <DialogDescription>
                  Remove diagnostic files, logs, or authentication history using the shared cleanup
                  rules.
                </DialogDescription>
              </DialogHeader>

              <div className="grid gap-4">
                <div className="grid gap-1.5">
                  <Label htmlFor="cleanup-target">Target</Label>
                  <Select
                    value={cleanTarget}
                    onValueChange={(value) => {
                      setCleanTarget(value as ActivityCleanupTarget);
                      setLastCleanupReport(null);
                    }}
                    disabled={cleaning}
                  >
                    <SelectTrigger id="cleanup-target">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">All activity data</SelectItem>
                      <SelectItem value="debugs">Debug frames</SelectItem>
                      <SelectItem value="logs">Logs</SelectItem>
                      <SelectItem value="auth_history">Authentication history</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                <div className="grid gap-1.5">
                  <Label htmlFor="cleanup-mode">Mode</Label>
                  <Select
                    value={cleanMode}
                    onValueChange={(value) => {
                      setCleanMode(value as ActivityCleanupMode);
                      setLastCleanupReport(null);
                    }}
                    disabled={cleaning}
                  >
                    <SelectTrigger id="cleanup-mode">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="retention">Configured retention</SelectItem>
                      <SelectItem value="all">Everything in target</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                {lastCleanupReport && (
                  <div className="rounded-md border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
                    Removed {cleanupRemovedEntries(lastCleanupReport)} entr(y/ies), freed{" "}
                    {formatBytes(cleanupFreedBytes(lastCleanupReport))}
                    {cleanupFailedEntries(lastCleanupReport) > 0 &&
                      `, ${cleanupFailedEntries(lastCleanupReport)} failed`}
                  </div>
                )}
              </div>

              <DialogFooter>
                <DialogClose asChild>
                  <Button variant="outline" disabled={cleaning}>
                    Cancel
                  </Button>
                </DialogClose>
                <Button onClick={() => void cleanActivityData()} disabled={cleaning}>
                  {cleaning ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Sparkles className="w-4 h-4" />
                  )}
                  {cleaning ? "Cleaning..." : "Confirm Clean"}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              void loadHistory();
              void loadLogs();
            }}
            disabled={loading}
          >
            <RefreshCcw className="w-4 h-4" />
            Refresh
          </Button>
        </div>
      </div>

      <div className="rounded-lg border border-border bg-card p-4">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <h2 className="text-base font-medium">Authentication Test</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Start a real authentication run with temporary logging settings.
            </p>
          </div>
          <div className="grid gap-3 sm:grid-cols-[minmax(0,12rem)_10rem_auto] sm:items-end">
            <div className="grid gap-1.5">
              <Label htmlFor="auth-test-service">Service name</Label>
              <Input
                id="auth-test-service"
                value={testService}
                onChange={(event) => setTestService(event.target.value)}
                placeholder="sudo"
                disabled={testingAuth}
                className="h-8"
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="auth-test-log-level">Log level</Label>
              <Select
                value={testLogLevel}
                onValueChange={(value) => setTestLogLevel(value as LogLevelName)}
                disabled={testingAuth}
              >
                <SelectTrigger id="auth-test-log-level" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="debug">Debug</SelectItem>
                  <SelectItem value="info">Info</SelectItem>
                  <SelectItem value="warn">Warn</SelectItem>
                  <SelectItem value="error">Error</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Button
              size="sm"
              onClick={() => void testAuthFlow()}
              disabled={testingAuth}
              className="min-w-32"
            >
              {testingAuth ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Play className="w-4 h-4" />
              )}
              {testingAuth ? "Working..." : "Start Test"}
            </Button>
          </div>
        </div>
      </div>

      <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-4">
        <TabsList>
          <TabsTrigger value="history">
            <ShieldCheck className="w-4 h-4" />
            Authentication
          </TabsTrigger>
          <TabsTrigger value="logs">
            <Terminal className="w-4 h-4" />
            Logs
          </TabsTrigger>
        </TabsList>

        <TabsContent value="history" className="space-y-3">
          {history.length === 0 ? (
            <div className="border border-border rounded-lg p-8 text-center text-sm text-muted-foreground">
              No authentication summaries have been recorded yet.
            </div>
          ) : (
            history.map((entry) => <AuthSummaryRow key={entry.id} entry={entry} />)
          )}
        </TabsContent>

        <TabsContent value="logs" className="space-y-3">
          <div className="flex items-center gap-2">
            {(["auth", "helper", "desktop"] as ActivityLogComponent[]).map((item) => (
              <Button
                key={item}
                variant={component === item ? "default" : "outline"}
                size="sm"
                onClick={() => setComponent(item)}
              >
                <FileText className="w-4 h-4" />
                {labelForLogComponent(item)}
              </Button>
            ))}
          </div>
          <pre className="min-h-96 max-h-[60vh] overflow-auto rounded-lg border border-border bg-muted/30 p-4 text-xs leading-relaxed">
            {logs.length ? logs.join("\n") : emptyStateForLogComponent(component)}
          </pre>
        </TabsContent>
      </Tabs>
    </div>
  );
}

function AuthSummaryRow({ entry }: { entry: AuthSessionSummary }) {
  const primaryMethod = entry.methods[0];
  const best = useMemo(
    () => entry.methods.find((method) => method.best_match)?.best_match ?? null,
    [entry.methods],
  );
  const ir = useMemo(() => entry.methods.find((method) => method.ir)?.ir ?? null, [entry.methods]);

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <span className="font-medium">{entry.service || "Unknown service"}</span>
            <span className={resultClass(entry.result)}>{entry.result}</span>
          </div>
          <p className="text-sm text-muted-foreground">
            {primaryMethod?.message || "No method details recorded"}
          </p>
          {best && (
            <p className="text-xs text-muted-foreground">
              Best match #{best.enrolled_index}, similarity {best.similarity.toFixed(4)}, threshold{" "}
              {best.threshold.toFixed(4)}
            </p>
          )}
          {ir && <IrSummaryLine ir={ir} />}
        </div>
        <time className="shrink-0 text-xs text-muted-foreground">
          {new Date(entry.started_at).toLocaleString()}
        </time>
      </div>

      {entry.methods.length > 0 && (
        <div className="mt-4 grid gap-2">
          {entry.methods.map((method) => (
            <div key={method.method} className="rounded-md bg-muted/40 px-3 py-2 text-xs">
              <span className="font-medium capitalize">{method.method}</span>
              <span className="mx-2 text-muted-foreground">{method.result}</span>
              <span className="text-muted-foreground">{method.attempts.length} attempt(s)</span>
              {method.ir && (
                <span className="ml-2 text-muted-foreground">{formatIrSummary(method.ir)}</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function IrSummaryLine({ ir }: { ir: IrLivenessSummary }) {
  return <p className="text-xs text-muted-foreground">{formatIrSummary(ir)}</p>;
}

function formatIrSummary(ir: IrLivenessSummary) {
  const parts = [`IR ${ir.passed_frames}/${ir.frames} frame(s) passed`];
  parts.push(`required ${ir.required_passes}`);
  if (ir.last_failure) parts.push(`reason ${ir.last_failure}`);
  if (ir.highest_detection_confidence !== null) {
    parts.push(`highest confidence ${ir.highest_detection_confidence.toFixed(4)}`);
  }
  return parts.join(", ");
}

function labelForLogComponent(component: ActivityLogComponent) {
  switch (component) {
    case "auth":
      return "Auth";
    case "helper":
      return "Helper";
    case "desktop":
      return "Desktop";
  }
}

function emptyStateForLogComponent(component: ActivityLogComponent) {
  switch (component) {
    case "auth":
      return "No log lines found for today.";
    case "desktop":
      return "No desktop events have been logged yet.";
    case "helper":
      return "The helper is a CLI tool and does not write file logs, so this view stays empty.";
  }
}

function cleanupRemovedEntries(report: CleanupReport) {
  return report.sections.reduce((total, section) => total + section.removed_entries, 0);
}

function cleanupFailedEntries(report: CleanupReport) {
  return report.sections.reduce((total, section) => total + section.failed_entries, 0);
}

function cleanupFreedBytes(report: CleanupReport) {
  return report.sections.reduce((total, section) => total + section.freed_bytes, 0);
}

function formatBytes(bytes: number) {
  const units = [
    ["GiB", 1024 * 1024 * 1024],
    ["MiB", 1024 * 1024],
    ["KiB", 1024],
  ] as const;
  const unit = units.find(([, factor]) => bytes >= factor);
  if (!unit) return `${bytes} B`;
  return `${(bytes / unit[1]).toFixed(2)} ${unit[0]}`;
}

function resultClass(result: AuthSessionSummary["result"]) {
  const base = "rounded px-2 py-0.5 text-xs font-medium";
  if (result === "success") return `${base} bg-emerald-500/10 text-emerald-600`;
  if (result === "failure") return `${base} bg-red-500/10 text-red-600`;
  return `${base} bg-muted text-muted-foreground`;
}
