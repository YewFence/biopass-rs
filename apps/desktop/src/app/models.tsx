import { createFileRoute } from "@tanstack/react-router";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Copy, Cpu, Download, FolderOpen } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { cmd } from "@/commands";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { formatError } from "@/lib/utils";
import type { BuiltinModelInfo } from "@/types/config";
import { ModelStatus, type ModelStatusType } from "./-components/ModelStatus";

interface ModelCardProps {
  model: BuiltinModelInfo;
  status: ModelStatusType;
  onCopy: (path: string) => void;
}

function ModelFileFolderButton({ path }: { path: string }) {
  const handleOpenFileFolder = async (path: string) => {
    try {
      await revealItemInDir(path);
    } catch (err) {
      console.error("Failed to open file location:", err);
      toast.error(`Failed to open file location: ${formatError(err)}`);
    }
  };
  return (
    <TooltipProvider>
      <Tooltip delayDuration={300}>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={() => handleOpenFileFolder(path)}
            className="inline-flex items-center gap-1 text-[10px] font-mono text-muted-foreground opacity-70 truncate max-w-80 bg-muted/50 px-1.5 py-0.5 rounded hover:opacity-100 hover:bg-primary/10 hover:text-primary transition-all cursor-pointer"
          >
            <FolderOpen className="size-3" />
            <span className="truncate">{path}</span>
          </button>
        </TooltipTrigger>
        <TooltipContent side="bottom" align="end" className="max-w-75 break-all">
          <p className="font-mono text-xs">{path}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

function ModelCard({ model, status, onCopy }: ModelCardProps) {
  const filename = model.path.split(/[/]/).pop() || model.path;

  return (
    <div className="group relative flex flex-col gap-4 p-5 rounded-xl border border-border bg-linear-to-b from-card to-muted/20 shadow-sm hover:border-primary/30 hover:shadow-md transition-all duration-300">
      <div className="flex sm:flex-row sm:items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-linear-to-br from-blue-500/10 to-indigo-500/10 flex items-center justify-center border border-blue-500/10 group-hover:border-blue-500/30 transition-colors">
            <Cpu className="w-5 h-5 text-blue-600 dark:text-blue-400" />
          </div>
          <div>
            <div className="flex items-center gap-4">
              <h3 className="font-semibold leading-none truncate max-w-100" title={filename}>
                {filename}
              </h3>
              <p className="text-xs text-muted-foreground capitalize mt-1 block">{model.type}</p>
            </div>
            <div className="mt-1 flex items-center gap-2">
              <ModelFileFolderButton path={model.path} />
              <TooltipProvider>
                <Tooltip delayDuration={300}>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => onCopy(model.path)}
                      aria-label={`Copy path for ${filename}`}
                    >
                      <Copy className="size-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Copy path</TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-1">
          <ModelStatus status={status} />
        </div>
      </div>
    </div>
  );
}

function ModelsRouteComponent() {
  const [models, setModels] = useState<BuiltinModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [downloading, setDownloading] = useState(false);
  const [statusMap, setStatusMap] = useState<
    Record<string, "checking" | "available" | "missing" | "inuse">
  >({});

  const hasMissing = Object.values(statusMap).some((s) => s === "missing");

  const checkModelsStatus = useCallback(async (modelList: BuiltinModelInfo[]) => {
    const newStatuses: Record<string, "checking" | "available" | "missing" | "inuse"> = {};

    for (const model of modelList) newStatuses[model.path] = "checking";
    setStatusMap({ ...newStatuses });

    try {
      const result = await cmd.config.load();
      if (result.status !== "loaded") {
        modelList.forEach((model) => {
          newStatuses[model.path] = model.present ? "available" : "missing";
        });
        setStatusMap({ ...newStatuses });
        return;
      }
      const { config } = result;
      const inUsePaths = new Set<string>();

      if (config.methods.face.detection.model) {
        inUsePaths.add(config.methods.face.detection.model);
      }
      if (config.methods.face.recognition.model) {
        inUsePaths.add(config.methods.face.recognition.model);
      }
      if (
        config.methods.face.anti_spoofing.rgb.enable &&
        config.methods.face.anti_spoofing.rgb.model.path
      ) {
        inUsePaths.add(config.methods.face.anti_spoofing.rgb.model.path);
      }
      if (
        config.methods.face.anti_spoofing.ir.enable &&
        config.methods.face.anti_spoofing.ir.model.path
      ) {
        inUsePaths.add(config.methods.face.anti_spoofing.ir.model.path);
      }
      modelList.forEach((model) => {
        if (!model.present) {
          newStatuses[model.path] = "missing";
        } else if (inUsePaths.has(model.path)) {
          newStatuses[model.path] = "inuse";
        } else {
          newStatuses[model.path] = "available";
        }
      });

      setStatusMap({ ...newStatuses });
    } catch (err) {
      console.error("Failed to check model usage:", err);
    }
  }, []);

  const loadModels = useCallback(async () => {
    try {
      setLoading(true);
      const loadedModels = await cmd.models.listBuiltin();
      setModels(loadedModels);
      await checkModelsStatus(loadedModels);
    } catch (err) {
      console.error("Failed to load models:", err);
      toast.error("Failed to load models");
    } finally {
      setLoading(false);
    }
  }, [checkModelsStatus]);

  const downloadModels = useCallback(async () => {
    setDownloading(true);
    try {
      const report = await cmd.models.downloadBuiltin();
      setModels(report.models);
      await checkModelsStatus(report.models);
      if (report.downloaded > 0) {
        toast.success(`Downloaded ${report.downloaded} model file(s).`);
      } else {
        toast.info("All model files are already present.");
      }
    } catch (err) {
      console.error("Failed to download models:", err);
      toast.error(`Failed to download models: ${formatError(err)}`);
    } finally {
      setDownloading(false);
    }
  }, [checkModelsStatus]);

  const copyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      toast.success("Model path copied.");
    } catch (err) {
      console.error("Failed to copy path:", err);
      toast.error(`Failed to copy path: ${formatError(err)}`);
    }
  }, []);

  useEffect(() => {
    void loadModels();
  }, [loadModels]);

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 w-full max-width-4xl mx-auto p-6">
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold bg-linear-to-r from-primary to-purple-500 bg-clip-text text-transparent">
            AI Model Management
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Download built-in authentication models and copy their paths into configuration.
          </p>
        </div>
        <Button onClick={() => void downloadModels()} disabled={downloading}>
          <Download className="w-4 h-4" />
          {downloading ? "Downloading..." : "Download Models"}
        </Button>
      </div>

      {hasMissing && (
        <div className="flex gap-3 p-4 rounded-lg border border-amber-500/30 bg-amber-500/5">
          <Download className="w-5 h-5 text-amber-600 dark:text-amber-400 shrink-0 mt-0.5" />
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-amber-900 dark:text-amber-200">
              Some models are missing
            </p>
            <p className="text-xs text-amber-800/80 dark:text-amber-300/80 mt-1">
              Use the download button to fetch every built-in model into the shared biopass-rs data
              directory.
            </p>
          </div>
        </div>
      )}

      <div className="flex flex-col gap-4">
        {models.length === 0 ? (
          <div className="col-span-full flex flex-col items-center justify-center p-12 text-center rounded-xl border border-dashed border-border/50 bg-card/50">
            <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mb-4">
              <Cpu className="w-6 h-6 text-muted-foreground" />
            </div>
            <h3 className="font-semibold text-lg">No models registered</h3>
          </div>
        ) : (
          models.map((model) => (
            <ModelCard
              key={model.path}
              model={model}
              status={statusMap[model.path]}
              onCopy={copyPath}
            />
          ))
        )}
      </div>
    </div>
  );
}

export const Route = createFileRoute("/models")({
  component: ModelsRouteComponent,
});
