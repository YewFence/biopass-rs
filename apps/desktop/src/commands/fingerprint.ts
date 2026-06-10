import { invokeCommand } from "./core";

function listEnrolled(username: string) {
  return invokeCommand<string[]>("list_enrolled_fingerprints", { username });
}

function enroll(username: string, fingerName: string) {
  return invokeCommand<void>("enroll_fingerprint", {
    username,
    fingerName,
  });
}

function remove(username: string, fingerName: string) {
  return invokeCommand<void>("remove_fingerprint", {
    username,
    fingerName,
  });
}

export const fingerprint = {
  listEnrolled,
  enroll,
  remove,
};
