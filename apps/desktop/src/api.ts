import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AssistantEvent, RuntimeHealth } from "./types";

export function getRuntimeHealth(): Promise<RuntimeHealth> {
  return invoke<RuntimeHealth>("assistant_health");
}

export function submitPrompt(text: string): Promise<string> {
  return invoke<string>("assistant_submit", { text });
}

export function restartRuntime(): Promise<void> {
  return invoke<void>("assistant_restart");
}

export function resetConversation(): Promise<void> {
  return invoke<void>("assistant_reset");
}

export function onAssistantEvent(
  handler: (event: AssistantEvent) => void,
): Promise<UnlistenFn> {
  return listen<AssistantEvent>("assistant:event", ({ payload }) => handler(payload));
}
