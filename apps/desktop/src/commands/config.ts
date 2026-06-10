import type { BiopassConfig } from "@/types/config";
import { invokeCommand } from "./core";

function load() {
  return invokeCommand<BiopassConfig>("load_config");
}

function save(config: BiopassConfig) {
  return invokeCommand<void>("save_config", { config });
}

export const config = {
  load,
  save,
};
