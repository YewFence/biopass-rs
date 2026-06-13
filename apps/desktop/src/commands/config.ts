import type { BiopassConfig, LoadConfigResult } from "@/types/config";
import { invokeCommand } from "./core";

function load() {
  return invokeCommand<LoadConfigResult>("load_config");
}

function save(config: BiopassConfig) {
  return invokeCommand<void>("save_config", { config });
}

function reset() {
  return invokeCommand<LoadConfigResult>("reset_config");
}

function filePath() {
  return invokeCommand<string>("config_file_path");
}

export const config = {
  load,
  save,
  reset,
  filePath,
};
