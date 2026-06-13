import { useEffect, useState } from "react";
import { ModelStatus } from "@/app/-components/ModelStatus";
import { cmd } from "@/commands";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";

interface Props {
  label: string;
  value: string;
  error: boolean;
  onChange: (value: string) => void;
}

export function ModelSelect({ label, value, error, onChange }: Props) {
  const [exists, setExists] = useState<boolean | undefined>(undefined);

  useEffect(() => {
    if (!value) {
      setExists(false);
      return;
    }

    let cancelled = false;
    setExists(undefined);

    const checkModel = async () => {
      try {
        const result = await cmd.file.exists(value);
        if (!cancelled) setExists(result);
      } catch {
        if (!cancelled) setExists(false);
      }
    };

    void checkModel();
    return () => {
      cancelled = true;
    };
  }, [value]);

  return (
    <div className="grid gap-2">
      <div className="flex items-center justify-between gap-3">
        {label ? <Label className="text-xs text-muted-foreground">{label}</Label> : <span />}
        <ModelStatus status={exists} size="sm" className="h-4" />
      </div>
      <Input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="/path/to/model.onnx"
        className={cn(
          "h-9 font-mono text-sm",
          error && !exists && "border-destructive ring-destructive/20",
        )}
      />
    </div>
  );
}
