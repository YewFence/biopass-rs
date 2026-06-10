import { invokeCommand } from "./core";

function getCurrentUsername() {
  return invokeCommand<string>("get_current_username");
}

export const system = {
  getCurrentUsername,
};
