import { invoke } from "@tauri-apps/api/core";

type CommandArgs = Record<string, unknown>;

export function invokeCommand<T>(
  command: string,
  args?: CommandArgs,
): Promise<T> {
  return invoke<T>(command, args);
}
