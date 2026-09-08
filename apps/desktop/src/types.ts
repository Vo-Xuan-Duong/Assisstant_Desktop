export type AssistantState =
  | "idle"
  | "listening"
  | "processing"
  | "executing"
  | "speaking"
  | "confirming"
  | "error";

export type AssistantEvent =
  | { type: "state_changed"; from: AssistantState; to: AssistantState }
  | { type: "text_delta"; text: string }
  | { type: "response_completed"; text: string }
  | { type: "tool_started"; name: string }
  | { type: "tool_finished"; name: string; success: boolean }
  | { type: "error"; code: string; message: string };

export interface VoiceCapabilities {
  tts_available: boolean;
  /** Compatibility field name: this now means the primary local STT runtime is compiled. */
  whisper_compiled: boolean;
  model_path?: string | null;
  model_available: boolean;
}

export interface VoiceTurnResult {
  transcript: string;
  response: string;
  tts_error?: string | null;
}

export interface VoiceTranscriptEvent {
  text: string;
  is_final: boolean;
}

export interface AudioLevel {
  rms: number;
  peak: number;
}

export type ReadinessLevel = "ready" | "optional_missing" | "blocking";

export interface ReadinessCheck {
  id: string;
  label: string;
  level: ReadinessLevel;
  detail: string;
  path?: string | null;
}

export interface SatelliteDeviceSnapshot {
  id: string;
  name: string;
  first_seen_unix: number;
  last_seen_unix: number;
  revoked: boolean;
}

export interface SatelliteReadinessSnapshot {
  enabled: boolean;
  paired: boolean;
  bind: string;
  credential_storage: string;
  environment_token_override: boolean;
  remote_managed: boolean;
  remote_port?: number | null;
  trusted_devices: number;
  revoked_devices: number;
  devices: SatelliteDeviceSnapshot[];
  warnings: string[];
}

export interface RuntimeReadinessReport {
  overall: ReadinessLevel;
  checks: ReadinessCheck[];
  satellite: SatelliteReadinessSnapshot;
}

export type ToolRisk = "safe" | "moderate" | "sensitive" | "blocked";

export interface PermissionRequest {
  request_id: string;
  tool_name: string;
  risk: ToolRisk;
  arguments: unknown;
}
