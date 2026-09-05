import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getRuntimeHealth,
  getVoiceCapabilities,
  getWakeStatus,
  onAssistantEvent,
  onVoiceLevel,
  onWakeEvent,
  resetConversation,
  restartRuntime,
  runVoiceTurn,
  setWakeEnabled,
  speakText,
  submitPrompt,
} from "./api";
import { onPermissionRequest, submitPermissionDecision } from "./permissionApi";
import AntigravitySettingsPanel from "./AntigravitySettingsPanel";
import PermissionPolicyPanel from "./PermissionPolicyPanel";
import ReadinessPanel from "./ReadinessPanel";
import ResourceSetupPanel from "./ResourceSetupPanel";
import type {
  AssistantState,
  ChatMessage,
  PermissionRequest,
  RuntimeHealth,
  VoiceCapabilities,
  WakeStatus,
} from "./types";

const quickPrompts = [
  "Âm lượng hiện tại bao nhiêu?",
  "Ứng dụng nào đang active?",
  "Máy đang dùng bao nhiêu RAM?",
];

const WAKE_TO_COMMAND_DELAY_MS = 180;
const PERMISSION_UI_TIMEOUT_SECONDS = 29;

type VoiceTurnOrigin = "manual" | "wake";

function message(role: ChatMessage["role"], text: string): ChatMessage {
  return { id: crypto.randomUUID(), role, text };
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function formatPermissionArguments(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "Không thể hiển thị arguments.";
  }
}

export default function App() {
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");
  const [health, setHealth] = useState<RuntimeHealth | null>(null);
  const [voice, setVoice] = useState<VoiceCapabilities | null>(null);
  const [wake, setWake] = useState<WakeStatus | null>(null);
  const [voiceLevel, setVoiceLevel] = useState(0);
  const [messages, setMessages] = useState<ChatMessage[]>([
    message("system", "Desktop runtime đã sẵn sàng. Antigravity sẽ được khởi tạo khi cần."),
  ]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [wakeBusy, setWakeBusy] = useState(false);
  const [readinessOpen, setReadinessOpen] = useState(false);
  const [resourcesOpen, setResourcesOpen] = useState(false);
  const [permissionsOpen, setPermissionsOpen] = useState(false);
  const [aiSettingsOpen, setAiSettingsOpen] = useState(false);
  const [isCheckingHealth, setIsCheckingHealth] = useState(false);
  const [isRestarting, setIsRestarting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [permissionQueue, setPermissionQueue] = useState<PermissionRequest[]>([]);
  const [permissionRemaining, setPermissionRemaining] = useState(PERMISSION_UI_TIMEOUT_SECONDS);
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const busyRef = useRef(false);
  const voiceReadyRef = useRef(false);
  const permissionRespondingRef = useRef(false);

  const activePermission = permissionQueue[0] ?? null;

  const refreshHealth = useCallback(async () => {
    setIsCheckingHealth(true);
    try {
      setHealth(await getRuntimeHealth());
    } catch (error) {
      setHealth({ state: "unhealthy", detail: String(error) });
    } finally {
      setIsCheckingHealth(false);
    }
  }, []);

  const refreshVoice = useCallback(async () => {
    try {
      setVoice(await getVoiceCapabilities());
    } catch {
      setVoice(null);
    }
  }, []);

  const refreshWake = useCallback(async () => {
    try {
      setWake(await getWakeStatus());
    } catch (error) {
      setWake({
        compiled: false,
        available: false,
        enabled: false,
        state: "unavailable",
        detail: String(error),
      });
    }
  }, []);

  const voiceReady = Boolean(voice?.whisper_compiled && voice.model_available);
  const voiceHint = useMemo(() => {
    if (!voice) return "Đang kiểm tra voice runtime";
    if (!voice.whisper_compiled) return "Build chưa bật feature voice-whisper";
    if (!voice.model_available) return "Thiếu Whisper model (bấm Tài nguyên để tải)";
    return "Voice local sẵn sàng";
  }, [voice]);

  const wakeHint = useMemo(() => {
    if (!wake) return "Đang kiểm tra wake runtime";
    if (!wake.compiled) return "Build chưa bật feature wake-word";
    if (!wake.available) {
      if (wake.detail && (wake.detail.includes("missing") || wake.detail.includes("encoder"))) {
        return "Chưa có Wake-word model (Sherpa ONNX)";
      }
      return wake.detail || "Wake-word model chưa sẵn sàng";
    }
    if (wake.state === "listening") return voiceReady ? "Wake word đang nghe nền · auto voice turn" : "Wake word đang nghe nền";
    if (wake.state === "suspended") return "Wake word tạm dừng khi Assistant dùng microphone";
    if (wake.state === "cooldown") return "Wake word đang cooldown";
    if (wake.state === "error") return wake.detail || "Wake runtime gặp lỗi";
    return wake.enabled ? "Wake word đang khởi tạo" : "Wake word đang tắt (bấm để bật)";
  }, [voiceReady, wake]);

  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);

  useEffect(() => {
    voiceReadyRef.current = voiceReady;
  }, [voiceReady]);

  const resolvePermission = useCallback(async (approved: boolean) => {
    const request = activePermission;
    if (!request || permissionRespondingRef.current) return;

    permissionRespondingRef.current = true;
    try {
      await submitPermissionDecision(request.request_id, approved);
    } catch (error) {
      // The broker may have already timed out. Do not include the permission
      // arguments in chat/log output; only report the request id/tool name.
      setMessages((current) => [
        ...current,
        message(
          "system",
          `Không thể gửi quyết định cho ${request.tool_name}: ${String(error)}`,
        ),
      ]);
    } finally {
      setPermissionQueue((current) =>
        current.filter((item) => item.request_id !== request.request_id),
      );
      permissionRespondingRef.current = false;
    }
  }, [activePermission]);

  useEffect(() => {
    if (!activePermission) {
      setPermissionRemaining(PERMISSION_UI_TIMEOUT_SECONDS);
      return;
    }

    setPermissionRemaining(PERMISSION_UI_TIMEOUT_SECONDS);
    const startedAt = Date.now();
    const interval = window.setInterval(() => {
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      setPermissionRemaining(Math.max(0, PERMISSION_UI_TIMEOUT_SECONDS - elapsed));
    }, 250);
    const timeoutId = window.setTimeout(() => {
      void resolvePermission(false);
    }, PERMISSION_UI_TIMEOUT_SECONDS * 1000);

    return () => {
      window.clearInterval(interval);
      window.clearTimeout(timeoutId);
    };
  }, [activePermission?.request_id, resolvePermission]);

  const performVoiceTurn = useCallback(
    async (origin: VoiceTurnOrigin) => {
      if (busyRef.current) return;
      if (!voiceReadyRef.current) {
        if (origin === "wake") {
          setMessages((current) => [
            ...current,
            message("system", "Wake word đã kích hoạt nhưng Whisper voice chưa sẵn sàng."),
          ]);
        }
        return;
      }

      busyRef.current = true;
      setBusy(true);
      setVoiceLevel(0);

      if (origin === "wake") {
        await delay(WAKE_TO_COMMAND_DELAY_MS);
      }

      try {
        const result = await runVoiceTurn();
        setMessages((current) => [
          ...current,
          message("user", result.transcript),
          message("assistant", result.response),
          ...(result.tts_error
            ? [message("system", `TTS không thể phát phản hồi: ${result.tts_error}`)]
            : []),
        ]);
      } catch (error) {
        setMessages((current) => [
          ...current,
          message(
            "system",
            `${origin === "wake" ? "Wake voice turn" : "Voice turn"} thất bại: ${String(error)}`,
          ),
        ]);
      } finally {
        setVoiceLevel(0);
        await refreshHealth();
        await refreshVoice();
        await refreshWake();
        busyRef.current = false;
        setBusy(false);
      }
    },
    [refreshHealth, refreshVoice, refreshWake],
  );

  useEffect(() => {
    void refreshHealth();
    void refreshVoice();
    void refreshWake();
  }, [refreshHealth, refreshVoice, refreshWake]);

  useEffect(() => {
    let disposed = false;
    let unsubscribeAssistant: (() => void) | undefined;
    let unsubscribeLevel: (() => void) | undefined;
    let unsubscribeWake: (() => void) | undefined;
    let unsubscribePermission: (() => void) | undefined;

    void onAssistantEvent((event) => {
      if (event.type === "state_changed") {
        setAssistantState(event.to);
        if (event.to !== "listening") setVoiceLevel(0);
      }
      if (event.type === "error") {
        setAssistantState("error");
        setVoiceLevel(0);
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unsubscribeAssistant = unlisten;
    });

    void onVoiceLevel((level) => {
      const normalized = Math.max(0, Math.min(1, level.rms * 8));
      setVoiceLevel(normalized);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unsubscribeLevel = unlisten;
    });

    void onWakeEvent((event) => {
      if (event.type === "state_changed") {
        setWake((current) =>
          current
            ? {
                ...current,
                state: event.to,
                enabled: !["disabled", "stopped"].includes(event.to),
              }
            : current,
        );
      }
      if (event.type === "detected") {
        // The hidden main WebView remains alive while the app is in the tray, so
        // it can start the exact same backend voice-turn command used by the Mic
        // button. No second wake-specific STT/AI pipeline is introduced.
        void performVoiceTurn("wake");
      }
      if (event.type === "error") {
        setWake((current) => (current ? { ...current, state: "error", detail: event.message } : current));
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unsubscribeWake = unlisten;
    });

    void onPermissionRequest((request) => {
      setPermissionQueue((current) => {
        if (current.some((item) => item.request_id === request.request_id)) {
          return current;
        }
        return [...current, request];
      });
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unsubscribePermission = unlisten;
    });

    return () => {
      disposed = true;
      unsubscribeAssistant?.();
      unsubscribeLevel?.();
      unsubscribeWake?.();
      unsubscribePermission?.();
    };
  }, [performVoiceTurn]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const statusLabel = useMemo(() => {
    if (activePermission) return "Đang chờ xác nhận";
    if (!health) return "Đang kiểm tra runtime";
    if (health.state === "missing") return "Không tìm thấy Antigravity CLI";
    if (health.state === "unhealthy") return "Antigravity CLI có vấn đề";
    if (assistantState === "listening") return "Đang nghe";
    if (assistantState === "processing") return "Đang suy luận";
    if (assistantState === "executing") return "Đang thực thi tool";
    if (assistantState === "speaking") return "Đang trả lời";
    if (assistantState === "error") return "Cần khôi phục runtime";
    return "Sẵn sàng";
  }, [activePermission, assistantState, health]);

  const send = useCallback(
    async (text: string) => {
      const prompt = text.trim();
      if (!prompt || busyRef.current) return;

      setMessages((current) => [...current, message("user", prompt)]);
      setInput("");
      busyRef.current = true;
      setBusy(true);

      try {
        const response = await submitPrompt(prompt);
        setMessages((current) => [...current, message("assistant", response)]);
        await refreshHealth();
      } catch (error) {
        setMessages((current) => [
          ...current,
          message("assistant", `Không thể hoàn thành yêu cầu: ${String(error)}`),
        ]);
        await refreshHealth();
      } finally {
        busyRef.current = false;
        setBusy(false);
      }
    },
    [refreshHealth],
  );

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    await send(input);
  }

  const handleVoiceTurnClick = useCallback(() => {
    if (busyRef.current) return;
    if (!voiceReadyRef.current) {
      setResourcesOpen(true);
      setMessages((current) => [
        ...current,
        message(
          "system",
          `Microphone / Whisper chưa sẵn sàng (${voiceHint}). Đang mở bảng Tài nguyên để tải và cài đặt mô hình Whisper.`,
        ),
      ]);
      return;
    }
    void performVoiceTurn("manual");
  }, [performVoiceTurn, voiceHint]);

  async function onWakeToggle() {
    if (wakeBusy) return;
    if (!wake?.available) {
      setResourcesOpen(true);
      setMessages((current) => [
        ...current,
        message(
          "system",
          `Wake word chưa sẵn sàng (${wakeHint}). Đang mở bảng Tài nguyên để cấu hình mô hình hoặc wake phrase.`,
        ),
      ]);
      return;
    }
    setWakeBusy(true);
    try {
      setWake(await setWakeEnabled(!wake.enabled));
    } catch (error) {
      setMessages((current) => [
        ...current,
        message("system", `Không thể thay đổi wake word: ${String(error)}`),
      ]);
      await refreshWake();
    } finally {
      setWakeBusy(false);
    }
  }

  async function onSpeakLast() {
    if (busyRef.current || !voice?.tts_available) return;
    const lastAssistant = [...messages].reverse().find((item) => item.role === "assistant");
    if (!lastAssistant) return;

    busyRef.current = true;
    setBusy(true);
    try {
      await speakText(lastAssistant.text);
    } catch (error) {
      setMessages((current) => [
        ...current,
        message("system", `Không thể đọc phản hồi: ${String(error)}`),
      ]);
    } finally {
      await refreshWake();
      busyRef.current = false;
      setBusy(false);
    }
  }

  async function onRestart() {
    if (busyRef.current || isRestarting) return;
    setIsRestarting(true);
    busyRef.current = true;
    setBusy(true);
    try {
      await restartRuntime();
      setAssistantState("idle");
      setMessages((current) => [...current, message("system", "Antigravity runtime đã được khởi động lại.")]);
    } catch (error) {
      setMessages((current) => [...current, message("system", `Restart thất bại: ${String(error)}`)]);
    } finally {
      await refreshHealth();
      busyRef.current = false;
      setBusy(false);
      setIsRestarting(false);
    }
  }

  async function onNewConversation() {
    if (busyRef.current || isResetting) return;
    setIsResetting(true);
    busyRef.current = true;
    setBusy(true);
    try {
      await resetConversation();
      setAssistantState("idle");
      setMessages([message("system", "Đã tạo phiên hội thoại mới.")]);
    } finally {
      await refreshHealth();
      busyRef.current = false;
      setBusy(false);
      setIsResetting(false);
    }
  }

  return (
    <main className={`app-shell state-${assistantState}`}>
      <header className="topbar">
        <div>
          <p className="eyebrow">ASSISSTANT DESKTOP</p>
          <h1>Antigravity Assistant</h1>
        </div>
        <div className={`runtime-pill runtime-${health?.state ?? "checking"}`}>
          <span className="status-dot" />
          <span>{statusLabel}</span>
        </div>
      </header>

      <section className="runtime-card">
        <div>
          <span className="section-label">Runtime</span>
          <strong>{health?.detail || "Antigravity CLI"}</strong>
          <small>{health?.conversation_id ? `Conversation ${health.conversation_id}` : "Chưa có conversation active"}</small>
          <small className={voiceReady ? "voice-ready" : "voice-warning"}>{voiceHint}</small>
          <small className={wake?.available ? "wake-ready" : "wake-warning"}>{wakeHint}</small>
        </div>
        <div className="runtime-actions">
          <button
            type="button"
            className={`wake-toggle ${wake?.enabled ? "wake-toggle-on" : ""} ${!wake?.available ? "wake-toggle-unready" : ""}`}
            disabled={wakeBusy}
            title={wake?.available ? wakeHint : `${wakeHint} - Bấm để mở bảng Tài nguyên`}
            onClick={() => void onWakeToggle()}
          >
            {wakeBusy ? "Wake..." : wake?.available ? (wake.enabled ? "Wake ON" : "Wake OFF") : "Wake (setup)"}
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => setReadinessOpen(true)}
          >
            Readiness
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => setResourcesOpen(true)}
          >
            Tài nguyên
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => setPermissionsOpen(true)}
          >
            Quyền hạn
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => setAiSettingsOpen(true)}
          >
            Cài đặt AI
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy || isCheckingHealth}
            onClick={() => void refreshHealth()}
          >
            {isCheckingHealth ? "Đang kiểm tra..." : "Kiểm tra"}
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy || isRestarting}
            onClick={() => void onRestart()}
          >
            {isRestarting ? "Đang restart..." : "Restart"}
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy || isResetting}
            onClick={() => void onNewConversation()}
          >
            {isResetting ? "Đang tạo..." : "Phiên mới"}
          </button>
        </div>
      </section>

      <section className="conversation" aria-live="polite">
        {messages.map((item) => (
          <article key={item.id} className={`message message-${item.role}`}>
            <span>{item.role === "user" ? "Bạn" : item.role === "assistant" ? "Assistant" : "System"}</span>
            <p>{item.text}</p>
          </article>
        ))}
        {busy && assistantState !== "listening" && assistantState !== "speaking" && (
          <article className="message message-assistant pending">
            <span>Assistant</span>
            <p>Đang xử lý<span className="thinking-dots">...</span></p>
          </article>
        )}
        <div ref={bottomRef} />
      </section>

      <section className="quick-prompts">
        {quickPrompts.map((prompt) => (
          <button key={prompt} type="button" disabled={busy} onClick={() => void send(prompt)}>
            {prompt}
          </button>
        ))}
      </section>

      <div className="voice-strip" aria-live="polite">
        <div className="voice-meter" aria-hidden="true">
          <span style={{ transform: `scaleX(${Math.max(0.025, voiceLevel)})` }} />
        </div>
        <span>{assistantState === "listening" ? "Hãy nói..." : voiceHint}</span>
      </div>

      <form className="composer" onSubmit={onSubmit}>
        <textarea
          value={input}
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void send(input);
            }
          }}
          placeholder="Nhập yêu cầu cho Assistant..."
          rows={2}
          disabled={busy}
        />
        <div className="composer-actions">
          <button
            type="button"
            className={`voice-button ${!voiceReady ? "voice-button-unready" : ""}`}
            title={voiceReady ? "Bắt đầu thu âm giọng nói" : `${voiceHint} - Bấm để mở cài đặt`}
            aria-label="Bắt đầu voice turn"
            disabled={busy}
            onClick={handleVoiceTurnClick}
          >
            {voiceReady ? "Mic" : "Cài Mic"}
          </button>
          <button type="submit" className="primary" disabled={busy || !input.trim()}>
            Gửi
          </button>
        </div>
      </form>

      <footer>
        <span>Alt + Space hoặc wake word để gọi Assistant</span>
        <button
          type="button"
          className="footer-action"
          disabled={busy || !voice?.tts_available || !messages.some((item) => item.role === "assistant")}
          onClick={() => void onSpeakLast()}
        >
          Đọc lại phản hồi cuối
        </button>
      </footer>

      {activePermission && (
        <div className="permission-backdrop" role="presentation">
          <section
            className="permission-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="permission-title"
          >
            <div className="permission-heading">
              <div>
                <p className="eyebrow">SENSITIVE TOOL</p>
                <h2 id="permission-title">Xác nhận hành động</h2>
              </div>
              <span className="risk-badge risk-sensitive">{activePermission.risk}</span>
            </div>

            <p className="permission-copy">
              Assistant muốn thực thi <strong>{activePermission.tool_name}</strong>. Hành động chỉ
              tiếp tục nếu bạn cho phép lần này.
            </p>

            <div className="permission-meta">
              <span>Request</span>
              <code>{activePermission.request_id}</code>
            </div>

            <div className="permission-arguments">
              <span>Arguments</span>
              <pre>{formatPermissionArguments(activePermission.arguments)}</pre>
            </div>

            <div className="permission-timeout">
              Tự động từ chối sau <strong>{permissionRemaining}s</strong>
              {permissionQueue.length > 1 && ` · ${permissionQueue.length - 1} yêu cầu đang chờ`}
            </div>

            <div className="permission-actions">
              <button
                type="button"
                className="permission-deny"
                onClick={() => void resolvePermission(false)}
              >
                Từ chối
              </button>
              <button
                type="button"
                className="permission-allow"
                onClick={() => void resolvePermission(true)}
              >
                Cho phép một lần
              </button>
            </div>
          </section>
        </div>
      )}

      <ReadinessPanel
        open={readinessOpen}
        onClose={() => setReadinessOpen(false)}
        showTrigger={false}
        onWakeChanged={refreshWake}
      />

      <ResourceSetupPanel
        open={resourcesOpen}
        onClose={() => setResourcesOpen(false)}
        showTrigger={false}
        onResourcesChanged={async () => {
          await refreshVoice();
          await refreshWake();
          await refreshHealth();
        }}
      />

      <PermissionPolicyPanel
        open={permissionsOpen}
        onClose={() => setPermissionsOpen(false)}
        showTrigger={false}
      />

      <AntigravitySettingsPanel
        open={aiSettingsOpen}
        onClose={() => setAiSettingsOpen(false)}
        showTrigger={false}
        onSettingsSaved={async () => {
          await refreshHealth();
        }}
      />
    </main>
  );
}
