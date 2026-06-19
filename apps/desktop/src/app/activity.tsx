import { createFileRoute } from "@tanstack/react-router";
import { FileText, RefreshCcw, ShieldCheck, Terminal } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { cmd } from "@/commands";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { ActivityLogComponent, AuthSessionSummary } from "@/types/activity";

export const Route = createFileRoute("/activity")({
  component: ActivityPage,
});

function ActivityPage() {
  const [history, setHistory] = useState<AuthSessionSummary[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [component, setComponent] = useState<ActivityLogComponent>("auth");
  const [loading, setLoading] = useState(false);

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

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-normal">Activity</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Authentication summaries and diagnostic logs
          </p>
        </div>
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

      <Tabs defaultValue="history" className="space-y-4">
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
            {logs.length ? logs.join("\n") : "No log lines found for today."}
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
            </div>
          ))}
        </div>
      )}
    </div>
  );
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

function resultClass(result: AuthSessionSummary["result"]) {
  const base = "rounded px-2 py-0.5 text-xs font-medium";
  if (result === "success") return `${base} bg-emerald-500/10 text-emerald-600`;
  if (result === "failure") return `${base} bg-red-500/10 text-red-600`;
  return `${base} bg-muted text-muted-foreground`;
}
