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

export interface RuntimeHealth {
  state: "available" | "missing" | "unhealthy";
  detail?: string | null;
  conversation_id?: string | null;
}

export type ReadinessLevel = "ready" | "optional_missing" | "blocking";

export interface ReadinessCheck {
  id: string;
  label: string;
  level: ReadinessLevel;
  detail: string;
  path?: string | null;
}

export interface RuntimeReadinessReport {
  overall: ReadinessLevel;
  checks: ReadinessCheck[];
}

export type ResourceState = "ready" | "missing" | "incomplete" | "not_compiled";

export interface ResourceFileStatus {
  name: string;
  path: string;
  exists: boolean;
}

export interface RuntimeResourceStatus {
  id: string;
  label: string;
  state: ResourceState;
  compiled: boolean;
  root_path: string;
  detail: string;
  files: ResourceFileStatus[];
}

export interface RuntimeResourceSnapshot {
  resources: RuntimeResourceStatus[];
}

export type ResourcePackageKind = "single_file" | "tar_bz2";

export interface ResourceInstallManifest {
  id: string;
  version: string;
  package_kind: ResourcePackageKind;
  installable: boolean;
  source_url: string;
  source_page: string;
  license: string;
  expected_bytes: number;
  sha256?: string | null;
  note: string;
}

export type ResourceInstallStage = "starting" | "downloading" | "verified" | "installed" | "failed";

export interface ResourceInstallProgress {
  resource_id: string;
  stage: ResourceInstallStage;
  downloaded_bytes: number;
  total_bytes: number;
  message: string;
}

export interface ResourceInstallResult {
  resource_id: string;
  path: string;
  bytes: number;
  sha256: string;
}

export interface VoiceCapabilities {
  tts_available: boolean;
  whisper_compiled: boolean;
  model_path?: string | null;
  model_available: boolean;
}

export interface VoiceTurnResult {
  transcript: string;
  response: string;
  tts_error?: string | null;
}

export interface AudioLevel {
  rms: number;
  peak: number;
}

export type WakeRuntimeState =
  | "not_compiled"
  | "unavailable"
  | "disabled"
  | "starting"
  | "listening"
  | "suspended"
  | "cooldown"
  | "error"
  | "stopped";

export interface WakeStatus {
  compiled: boolean;
  available: boolean;
  enabled: boolean;
  state: WakeRuntimeState;
  model_dir?: string | null;
  keywords_path?: string | null;
  detail?: string | null;
}

export interface WakeDetection {
  keyword: string;
  start_time_seconds: number;
}

export type WakeRuntimeEvent =
  | { type: "state_changed"; from: WakeRuntimeState; to: WakeRuntimeState }
  | { type: "detected"; detection: WakeDetection }
  | { type: "error"; message: string };

export type ToolRisk = "safe" | "moderate" | "sensitive" | "blocked";
export type PermissionDecision = "allow" | "ask" | "deny";

export interface PermissionRequest {
  request_id: string;
  tool_name: string;
  risk: ToolRisk;
  arguments: unknown;
}

export interface PermissionPolicyTool {
  name: string;
  description: string;
  default_decision: PermissionDecision;
  override_decision?: PermissionDecision | null;
}

export interface PermissionPolicyView {
  revision: number;
  load_error?: string | null;
  tools: PermissionPolicyTool[];
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  text: string;
}
