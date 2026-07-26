import { Fingerprint, ScanFace, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { cmd } from "@/commands";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useConfigurationStore } from "../-stores/configuration-store";
import { FingerprintSetting } from "./FingerprintSetting";
import { FaceSetting } from "./face/FaceSetting";
import { MethodCard } from "./MethodCard";

export function MethodConfig() {
  const faceConfig = useConfigurationStore((state) => state.config?.methods.face);
  const fingerprintConfig = useConfigurationStore((state) => state.config?.methods.fingerprint);
  const setFaceConfig = useConfigurationStore((state) => state.setFaceConfig);
  const setFingerprintConfig = useConfigurationStore((state) => state.setFingerprintConfig);
  const [expandedMethod, setExpandedMethod] = useState<string | null>("face");
  // null = not probed yet, or the probe itself failed. A failed probe must not
  // be reported as "hardware absent".
  const [faceAvailable, setFaceAvailable] = useState<boolean | null>(null);
  const [fingerprintAvailable, setFingerprintAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    let canceled = false;

    const loadHardwareState = async () => {
      try {
        const devices = await cmd.face.listVideoDevices();
        if (!canceled) {
          setFaceAvailable(devices.length > 0);
        }
      } catch (err) {
        console.error("Failed to check camera availability:", err);
      }

      try {
        const available = await cmd.fingerprint.isAvailable();
        if (!canceled) {
          setFingerprintAvailable(available);
        }
      } catch (err) {
        console.error("Failed to check fingerprint availability:", err);
      }
    };

    void loadHardwareState();

    return () => {
      canceled = true;
    };
  }, []);

  if (!faceConfig || !fingerprintConfig) return null;

  const methodIcons: Record<string, React.ReactNode> = {
    face: <ScanFace className="w-5 h-5 text-white" />,
    fingerprint: <Fingerprint className="w-5 h-5 text-white" />,
  };

  const methodColors: Record<string, string> = {
    face: "from-violet-500 to-purple-500",
    fingerprint: "from-emerald-500 to-teal-500",
  };
  const faceUnavailable = faceAvailable === false;
  const fingerprintUnavailable = fingerprintAvailable === false;

  return (
    <div className="rounded-xl border border-border/50 bg-card/50 backdrop-blur-sm p-6 shadow-lg">
      <h2 className="text-2xl sm:text-3xl font-bold tracking-tight mb-6 flex items-center gap-3">
        <span className="w-10 h-10 rounded-xl bg-linear-to-br from-purple-500 to-pink-500 flex items-center justify-center shadow-sm">
          <ShieldCheck className="w-5 h-5 text-white" />
        </span>
        Authentication Methods
      </h2>

      <div className="grid gap-4">
        {/* Face Authentication */}
        <MethodCard
          title="Face Recognition"
          icon={methodIcons.face}
          color={methodColors.face}
          enabled={faceConfig.enable}
          unavailable={faceUnavailable}
          unavailableMessage="No camera is available. Authentication may fail, and face enrollment is unavailable."
          onToggle={(enable) => {
            setFaceConfig({ ...faceConfig, enable });
          }}
          expanded={expandedMethod === "face"}
          onExpand={() => setExpandedMethod(expandedMethod === "face" ? null : "face")}
        >
          <FaceSetting enrollmentUnavailable={faceUnavailable} />
        </MethodCard>

        {/* Fingerprint Authentication */}
        <MethodCard
          title="Fingerprint"
          icon={methodIcons.fingerprint}
          color={methodColors.fingerprint}
          enabled={fingerprintConfig.enable}
          unavailable={fingerprintUnavailable}
          unavailableMessage="No fingerprint reader is available. Authentication may fail, and fingerprint enrollment is unavailable."
          onToggle={(enable) => {
            setFingerprintConfig({ ...fingerprintConfig, enable });
          }}
          expanded={expandedMethod === "fingerprint"}
          onExpand={() =>
            setExpandedMethod(expandedMethod === "fingerprint" ? null : "fingerprint")
          }
        >
          <div className="grid gap-4 pt-4">
            {/* Retries and Delay */}
            <div className="grid grid-cols-2 gap-6 p-4 rounded-lg bg-muted/50 border border-border/50">
              <div className="grid gap-2">
                <Label
                  htmlFor="fingerprint-max-retries"
                  className="text-sm font-medium text-muted-foreground"
                >
                  Max Retries
                </Label>
                <Input
                  id="fingerprint-max-retries"
                  type="number"
                  min="0"
                  max="10"
                  value={fingerprintConfig.retries}
                  onChange={(e) =>
                    setFingerprintConfig({
                      ...fingerprintConfig,
                      retries: parseInt(e.target.value, 10) || 0,
                    })
                  }
                  className="h-10"
                />
              </div>
              <div className="grid gap-2">
                <Label
                  htmlFor="fingerprint-timeout"
                  className="text-sm font-medium text-muted-foreground"
                >
                  Timeout (ms)
                </Label>
                <Input
                  id="fingerprint-timeout"
                  type="number"
                  min="0"
                  max="5000"
                  step="100"
                  value={fingerprintConfig.timeout}
                  onChange={(e) =>
                    setFingerprintConfig({
                      ...fingerprintConfig,
                      timeout: parseInt(e.target.value, 10) || 0,
                    })
                  }
                  className="h-10"
                />
              </div>
            </div>

            <div className="overflow-hidden">
              <FingerprintSetting enrollmentUnavailable={fingerprintUnavailable} />
            </div>
          </div>
        </MethodCard>
      </div>
    </div>
  );
}
