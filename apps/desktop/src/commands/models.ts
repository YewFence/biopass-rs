import type { BuiltinModelInfo, ModelDownloadReport } from "@/types/config";
import { invokeCommand } from "./core";

function listBuiltin() {
  return invokeCommand<BuiltinModelInfo[]>("list_builtin_models");
}

function downloadBuiltin() {
  return invokeCommand<ModelDownloadReport>("download_builtin_models");
}

export const models = {
  listBuiltin,
  downloadBuiltin,
};
