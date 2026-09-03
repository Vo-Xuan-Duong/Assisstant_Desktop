import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PermissionRequest } from "./types";

export function submitPermissionDecision(
  requestId: string,
  approved: boolean,
): Promise<void> {
  return invoke<void>("assistant_permission_respond", {
    requestId,
    allow: approved,
  });
}

export function onPermissionRequest(
  handler: (request: PermissionRequest) => void,
): Promise<UnlistenFn> {
  return listen<PermissionRequest>("permission:request", ({ payload }) => handler(payload));
}
