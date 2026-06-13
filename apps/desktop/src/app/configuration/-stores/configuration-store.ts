import { toast } from "sonner";
import { create } from "zustand";
import { validateConfig } from "@/app/configuration/-components/validation";
import { cmd } from "@/commands";
import { formatError } from "@/lib/utils";
import type {
  BiopassConfig,
  FaceMethodConfig,
  FingerprintMethodConfig,
  MethodsConfig,
  StrategyConfig,
} from "@/types/config";

export interface BrokenConfigInfo {
  path: string;
  message: string;
}

interface ConfigurationStore {
  config: BiopassConfig | null;
  savedConfig: BiopassConfig | null;
  loading: boolean;
  saving: boolean;
  /** Set when the on-disk config cannot be parsed. The configuration page
   *  renders a recovery overlay (copy path / open / reset) while this is set. */
  brokenConfig: BrokenConfigInfo | null;
  initializeConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  /** Reset the *unsaved* edits back to the last value loaded from disk. */
  resetConfig: () => void;
  /** Rewrite the on-disk config file with the built-in defaults. Used by the
   *  "Reset to defaults" recovery button when the file is broken. */
  resetToDefaults: () => Promise<void>;
  setStrategy: (strategy: StrategyConfig) => void;
  setMethods: (methods: MethodsConfig) => void;
  setFaceConfig: (face: FaceMethodConfig) => void;
  setFingerprintConfig: (fingerprint: FingerprintMethodConfig) => void;
}

export const useConfigurationStore = create<ConfigurationStore>((set, get) => ({
  config: null,
  savedConfig: null,
  loading: true,
  saving: false,
  brokenConfig: null,

  initializeConfig: async () => {
    try {
      set({ loading: true });
      const result = await cmd.config.load();
      if (result.status === "broken") {
        set({
          config: null,
          savedConfig: null,
          brokenConfig: { path: result.path, message: result.message },
        });
        toast.error(`Failed to parse config at ${result.path}`);
        return;
      }
      set({
        config: result.config,
        savedConfig: result.config,
        brokenConfig: null,
      });
      if (result.initialized) {
        toast.info("Initialized default configuration");
      } else if (result.migrated) {
        toast.success("Configuration file was migrated to the latest schema");
      }
    } catch (err) {
      toast.error(`Failed to load config: ${formatError(err)}`);
    } finally {
      set({ loading: false });
    }
  },

  saveConfig: async () => {
    const config = get().config;
    if (!config) return;

    const isValid = await validateConfig(config);
    if (!isValid) return;

    try {
      set({ saving: true });
      await cmd.config.save(config);
      set({ savedConfig: config });
      toast.success("Settings saved successfully!");
    } catch (err) {
      console.error("Failed to save config:", err);
      toast.error(`Failed to save config: ${formatError(err)}`);
    } finally {
      set({ saving: false });
    }
  },

  resetConfig: () => {
    const savedConfig = get().savedConfig;
    if (!savedConfig) return;

    set({ config: savedConfig });
    toast.info("Configuration reset to last saved state");
  },

  resetToDefaults: async () => {
    try {
      set({ loading: true });
      const result = await cmd.config.reset();
      if (result.status === "broken") {
        // Shouldn't happen — reset_config always writes a parseable file —
        // but if it does, surface it instead of falling through silently.
        set({
          brokenConfig: { path: result.path, message: result.message },
          config: null,
          savedConfig: null,
        });
        toast.error("Reset wrote a config the backend could not parse — please report this");
        return;
      }
      set({
        config: result.config,
        savedConfig: result.config,
        brokenConfig: null,
      });
      toast.success("Configuration reset to defaults");
    } catch (err) {
      toast.error(`Failed to reset config: ${formatError(err)}`);
    } finally {
      set({ loading: false });
    }
  },

  setStrategy: (strategy) => {
    set((state) => {
      if (!state.config) return state;
      return { config: { ...state.config, strategy } };
    });
  },

  setMethods: (methods) => {
    set((state) => {
      if (!state.config) return state;
      return { config: { ...state.config, methods } };
    });
  },

  setFaceConfig: (face) => {
    set((state) => {
      if (!state.config) return state;
      return {
        config: {
          ...state.config,
          methods: { ...state.config.methods, face },
        },
      };
    });
  },

  setFingerprintConfig: (fingerprint) => {
    set((state) => {
      if (!state.config) return state;
      return {
        config: {
          ...state.config,
          methods: { ...state.config.methods, fingerprint },
        },
      };
    });
  },
}));
