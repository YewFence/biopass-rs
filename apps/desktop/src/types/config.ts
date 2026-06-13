export interface BiopassConfig {
  strategy: StrategyConfig;
  methods: MethodsConfig;
  models: ModelConfig[];
  appearance: string;
}

export interface StrategyConfig {
  debug: boolean;
  execution_mode: "sequential" | "parallel";
  order: string[];
  ignore_services: string[];
}

export interface MethodsConfig {
  face: FaceMethodConfig;
  fingerprint: FingerprintMethodConfig;
}

export interface VideoDeviceInfo {
  path: string;
  name: string;
  display_name: string;
}

export interface FaceMethodConfig {
  enable: boolean;
  retries: number;
  retry_delay: number;
  camera: string | null;
  detection: {
    model: string;
    threshold: number;
  };
  recognition: {
    model: string;
    threshold: number;
  };
  anti_spoofing: {
    rgb: {
      enable: boolean;
      retries: number;
      retry_delay_ms: number;
      model: {
        path: string;
        threshold: number;
      };
    };
    ir: {
      enable: boolean;
      retries: number;
      retry_delay_ms: number;
      camera: string | null;
      warmup_delay_ms: number;
      min_face_area_ratio: number;
      model: {
        path: string;
        threshold: number;
      };
    };
  };
  auto_optimize_camera: boolean;
}

export interface FingerprintMethodConfig {
  enable: boolean;
  retries: number;
  timeout: number;
  fingers: FingerConfig[];
}

export interface FingerConfig {
  name: string;
  created_at: number;
}

export interface ModelConfig {
  path: string;
  type: "detection" | "recognition" | "anti-spoofing";
}

export interface LoadConfigResult {
  config: BiopassConfig;
  /** True when the on-disk config was rewritten to the current schema. */
  migrated: boolean;
}
