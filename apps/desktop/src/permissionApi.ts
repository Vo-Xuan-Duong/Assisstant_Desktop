import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  PermissionDecision,
  PermissionPolicyView,
  PermissionRequest,
} from "./types";

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

export function requestPermissionPolicy(): Promise<void> {
  return emit("permission:policy_get");
}

export function setPermissionPolicy(
  toolName: string,
  decision: PermissionDecision | null,
): Promise<void> {
  return emit("permission:policy_set", {
    tool_name: toolName,
    decision,
  });
}

export function onPermissionPolicy(
  handler: (view: PermissionPolicyView) => void,
): Promise<UnlistenFn> {
  return listen<PermissionPolicyView>("permission:policy_snapshot", ({ payload }) =>
    handler(payload),
  );
}

export function onPermissionPolicyError(
  handler: (message: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("permission:policy_error", ({ payload }) => handler(payload));
}
