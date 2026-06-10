import { toast } from "sonner";
import { create } from "zustand";
import { validateConfig } from "@/app/configuration/-components/validation";
import { cmd } from "@/commands";
import type {
  BiopassConfig,
  FaceMethodConfig,
  FingerprintMethodConfig,
  MethodsConfig,
  StrategyConfig,
} from "@/types/config";

interface ConfigurationStore {
  config: BiopassConfig | null;
  savedConfig: BiopassConfig | null;
  loading: boolean;
  saving: boolean;
  initializeConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  resetConfig: () => void;
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

  initializeConfig: async () => {
    try {
      set({ loading: true });
      const loadedConfig = await cmd.config.load();
      set({ config: loadedConfig, savedConfig: loadedConfig });
    } catch (err) {
      toast.error(`Failed to load config: ${err}`);
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
      toast.error(`Failed to save config: ${err}`);
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
