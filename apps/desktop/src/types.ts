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

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  text: string;
}
