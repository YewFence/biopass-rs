import { openPath } from "@tauri-apps/plugin-opener";
import { AlertTriangle, Copy, FileEdit, RotateCcw } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { formatError } from "@/lib/utils";

interface BrokenConfigOverlayProps {
  path: string;
  message: string;
  onReset: () => Promise<void> | void;
}

export function BrokenConfigOverlay({ path, message, onReset }: BrokenConfigOverlayProps) {
  const [resetting, setResetting] = useState(false);

  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(path);
      toast.success("Config path copied to clipboard");
    } catch (err) {
      toast.error(`Failed to copy path: ${formatError(err)}`);
    }
  };

  const handleOpenInEditor = async () => {
    try {
      await openPath(path);
    } catch (err) {
      toast.error(`Failed to open config file: ${formatError(err)}`);
    }
  };

  const handleReset = async () => {
    const confirmed = window.confirm(
      "Reset the configuration file to defaults? Your current settings will be lost.",
    );
    if (!confirmed) return;
    setResetting(true);
    try {
      await onReset();
    } finally {
      setResetting(false);
    }
  };

  return (
    <div className="flex flex-col gap-6 w-full max-w-3xl mx-auto p-6">
      <div className="flex items-start gap-3 rounded-lg border border-destructive/40 bg-destructive/5 p-5">
        <AlertTriangle className="size-6 text-destructive shrink-0 mt-0.5" />
        <div className="flex flex-col gap-2 min-w-0">
          <h1 className="text-lg font-semibold text-destructive">
            Configuration file could not be parsed
          </h1>
          <p className="text-sm text-muted-foreground">
            BioPass found a config file on disk, but it does not match the expected schema. You can
            edit it manually with your preferred editor, or reset it to the built-in defaults.
          </p>
          <div className="mt-1 rounded-md bg-background/60 border border-border px-3 py-2">
            <p className="text-[11px] uppercase tracking-wider text-muted-foreground mb-1">
              Config file
            </p>
            <p className="font-mono text-xs break-all">{path}</p>
          </div>
          <details className="mt-2">
            <summary className="text-xs text-muted-foreground cursor-pointer select-none">
              Show parser error
            </summary>
            <pre className="mt-2 rounded-md bg-muted/50 p-3 text-xs whitespace-pre-wrap font-mono break-all">
              {message}
            </pre>
          </details>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button variant="outline" onClick={handleCopyPath} className="flex items-center gap-2">
          <Copy className="size-4" />
          Copy path
        </Button>
        <Button variant="outline" onClick={handleOpenInEditor} className="flex items-center gap-2">
          <FileEdit className="size-4" />
          Open in editor
        </Button>
        <Button
          variant="destructive"
          onClick={handleReset}
          disabled={resetting}
          className="flex items-center gap-2"
        >
          <RotateCcw className="size-4" />
          {resetting ? "Resetting..." : "Reset to defaults"}
        </Button>
      </div>
    </div>
  );
}
