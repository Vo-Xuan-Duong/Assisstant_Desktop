import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AssistantEvent,
  AudioLevel,
  VoiceCapabilities,
  VoiceTranscriptEvent,
  VoiceTurnResult,
} from "./types";

export function submitPrompt(text: string): Promise<string> {
  return invoke<string>("assistant_submit", { text });
}

export function getVoiceCapabilities(): Promise<VoiceCapabilities> {
  return invoke<VoiceCapabilities>("assistant_voice_capabilities");
}

export function runVoiceTurn(): Promise<VoiceTurnResult> {
  return invoke<VoiceTurnResult>("assistant_voice_turn");
}

export function speakResponse(text: string): Promise<void> {
  return invoke<void>("assistant_speak", { text });
}

export function hideQuickAssistant(): Promise<void> {
  return invoke<void>("assistant_quick_hide");
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

export function onVoiceTranscript(
  handler: (transcript: VoiceTranscriptEvent) => void,
): Promise<UnlistenFn> {
  return listen<VoiceTranscriptEvent>("voice:transcript", ({ payload }) => handler(payload));
}
