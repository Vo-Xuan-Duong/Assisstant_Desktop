import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AssistantEvent,
  AudioLevel,
  RuntimeHealth,
  VoiceCapabilities,
  VoiceTurnResult,
} from "./types";

export function getRuntimeHealth(): Promise<RuntimeHealth> {
  return invoke<RuntimeHealth>("assistant_health");
}

export function submitPrompt(text: string): Promise<string> {
  return invoke<string>("assistant_submit", { text });
}

export function getVoiceCapabilities(): Promise<VoiceCapabilities> {
  return invoke<VoiceCapabilities>("assistant_voice_capabilities");
}

export function runVoiceTurn(): Promise<VoiceTurnResult> {
  return invoke<VoiceTurnResult>("assistant_voice_turn");
}

export function speakText(text: string): Promise<void> {
  return invoke<void>("assistant_speak", { text });
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

export function onVoiceLevel(
  handler: (level: AudioLevel) => void,
): Promise<UnlistenFn> {
  return listen<AudioLevel>("voice:level", ({ payload }) => handler(payload));
}
