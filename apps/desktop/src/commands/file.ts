import { invokeCommand } from "./core";

function exists(path: string) {
  return invokeCommand<boolean>("path_exists", { path });
}

export const file = {
  exists,
};
