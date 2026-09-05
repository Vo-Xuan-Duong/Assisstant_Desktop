import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AntigravitySettingsView,
  AssistantEvent,
  AudioLevel,
  ResourceInstallManifest,
  ResourceInstallProgress,
  ResourceInstallResult,
  RuntimeHealth,
  RuntimeReadinessReport,
  RuntimeResourceSnapshot,
  VoiceCapabilities,
  VoiceTurnResult,
  WakeRuntimeEvent,
  WakeStatus,
} from "./types";

export function getRuntimeHealth(): Promise<RuntimeHealth> {
  return invoke<RuntimeHealth>("assistant_health");
}

export function getRuntimeReadiness(): Promise<RuntimeReadinessReport> {
  return invoke<RuntimeReadinessReport>("assistant_readiness");
}

export function getRuntimeResources(): Promise<RuntimeResourceSnapshot> {
  return invoke<RuntimeResourceSnapshot>("assistant_resources");
}

export function getResourceCatalog(): Promise<ResourceInstallManifest[]> {
  return invoke<ResourceInstallManifest[]>("assistant_resource_catalog");
}

export function installResource(resourceId: string, phrase?: string): Promise<ResourceInstallResult> {
  return invoke<ResourceInstallResult>("assistant_resource_install", { resourceId, phrase });
}

export function onResourceInstallProgress(
  handler: (progress: ResourceInstallProgress) => void,
): Promise<UnlistenFn> {
  return listen<ResourceInstallProgress>("resource:install_progress", ({ payload }) => handler(payload));
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

export function getWakeStatus(): Promise<WakeStatus> {
  return invoke<WakeStatus>("assistant_wake_status");
}

export function setWakeEnabled(enabled: boolean): Promise<WakeStatus> {
  return invoke<WakeStatus>("assistant_wake_set_enabled", { enabled });
}

export function restartRuntime(): Promise<void> {
  return invoke<void>("assistant_restart");
}

export function resetConversation(): Promise<void> {
  return invoke<void>("assistant_reset");
}

export function hideQuickAssistant(): Promise<void> {
  return invoke<void>("assistant_quick_hide");
}

export function openFullAssistant(): Promise<void> {
  return invoke<void>("assistant_quick_expand");
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

export function onWakeEvent(
  handler: (event: WakeRuntimeEvent) => void,
): Promise<UnlistenFn> {
  return listen<WakeRuntimeEvent>("wake:event", ({ payload }) => handler(payload));
}

export function getAntigravitySettings(): Promise<AntigravitySettingsView> {
  return invoke<AntigravitySettingsView>("assistant_get_antigravity_settings");
}

export function saveAntigravitySettings(payload: {
  model?: string | null;
  effort?: string | null;
}): Promise<AntigravitySettingsView> {
  return invoke<AntigravitySettingsView>("assistant_save_antigravity_settings", { payload });
}

export function launchAntigravityAuth(): Promise<void> {
  return invoke<void>("assistant_launch_antigravity_auth");
}
