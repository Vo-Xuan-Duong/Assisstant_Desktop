import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getRuntimeHealth,
  getVoiceCapabilities,
  onAssistantEvent,
  onVoiceLevel,
  resetConversation,
  restartRuntime,
  runVoiceTurn,
  speakText,
  submitPrompt,
} from "./api";
import type {
  AssistantState,
  ChatMessage,
  RuntimeHealth,
  VoiceCapabilities,
} from "./types";

const quickPrompts = [
  "Âm lượng hiện tại bao nhiêu?",
  "Ứng dụng nào đang active?",
  "Máy đang dùng bao nhiêu RAM?",
];

function message(role: ChatMessage["role"], text: string): ChatMessage {
  return { id: crypto.randomUUID(), role, text };
}

export default function App() {
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");
  const [health, setHealth] = useState<RuntimeHealth | null>(null);
  const [voice, setVoice] = useState<VoiceCapabilities | null>(null);
  const [voiceLevel, setVoiceLevel] = useState(0);
  const [messages, setMessages] = useState<ChatMessage[]>([
    message("system", "Desktop runtime đã sẵn sàng. Antigravity sẽ được khởi tạo khi cần."),
  ]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const refreshHealth = useCallback(async () => {
    try {
      setHealth(await getRuntimeHealth());
    } catch (error) {
      setHealth({ state: "unhealthy", detail: String(error) });
    }
  }, []);

  const refreshVoice = useCallback(async () => {
    try {
      setVoice(await getVoiceCapabilities());
    } catch {
      setVoice(null);
    }
  }, []);

  useEffect(() => {
    void refreshHealth();
    void refreshVoice();
    let disposed = false;
    let unsubscribeAssistant: (() => void) | undefined;
    let unsubscribeLevel: (() => void) | undefined;

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

    return () => {
      disposed = true;
      unsubscribeAssistant?.();
      unsubscribeLevel?.();
    };
  }, [refreshHealth, refreshVoice]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const statusLabel = useMemo(() => {
    if (!health) return "Đang kiểm tra runtime";
    if (health.state === "missing") return "Không tìm thấy Antigravity CLI";
    if (health.state === "unhealthy") return "Antigravity CLI có vấn đề";
    if (assistantState === "listening") return "Đang nghe";
    if (assistantState === "processing") return "Đang suy luận";
    if (assistantState === "executing") return "Đang thực thi tool";
    if (assistantState === "speaking") return "Đang trả lời";
    if (assistantState === "error") return "Cần khôi phục runtime";
    return "Sẵn sàng";
  }, [assistantState, health]);

  const voiceReady = Boolean(voice?.whisper_compiled && voice.model_available);
  const voiceHint = useMemo(() => {
    if (!voice) return "Đang kiểm tra voice runtime";
    if (!voice.whisper_compiled) return "Build chưa bật feature voice-whisper";
    if (!voice.model_available) return `Thiếu Whisper model${voice.model_path ? `: ${voice.model_path}` : ""}`;
    return "Voice local sẵn sàng";
  }, [voice]);

  const send = useCallback(
    async (text: string) => {
      const prompt = text.trim();
      if (!prompt || busy) return;

      setMessages((current) => [...current, message("user", prompt)]);
      setInput("");
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
        setBusy(false);
      }
    },
    [busy, refreshHealth],
  );

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    await send(input);
  }

  async function onVoiceTurn() {
    if (busy || !voiceReady) return;
    setBusy(true);
    setVoiceLevel(0);

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
        message("system", `Voice turn thất bại: ${String(error)}`),
      ]);
    } finally {
      setVoiceLevel(0);
      await refreshHealth();
      await refreshVoice();
      setBusy(false);
    }
  }

  async function onSpeakLast() {
    if (busy || !voice?.tts_available) return;
    const lastAssistant = [...messages].reverse().find((item) => item.role === "assistant");
    if (!lastAssistant) return;

    setBusy(true);
    try {
      await speakText(lastAssistant.text);
    } catch (error) {
      setMessages((current) => [
        ...current,
        message("system", `Không thể đọc phản hồi: ${String(error)}`),
      ]);
    } finally {
      setBusy(false);
    }
  }

  async function onRestart() {
    setBusy(true);
    try {
      await restartRuntime();
      setAssistantState("idle");
      setMessages((current) => [...current, message("system", "Antigravity runtime đã được khởi động lại.")]);
    } catch (error) {
      setMessages((current) => [...current, message("system", `Restart thất bại: ${String(error)}`)]);
    } finally {
      await refreshHealth();
      setBusy(false);
    }
  }

  async function onNewConversation() {
    setBusy(true);
    try {
      await resetConversation();
      setAssistantState("idle");
      setMessages([message("system", "Đã tạo phiên hội thoại mới.")]);
    } finally {
      await refreshHealth();
      setBusy(false);
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
        </div>
        <div className="runtime-actions">
          <button type="button" className="secondary" disabled={busy} onClick={() => void refreshHealth()}>
            Kiểm tra
          </button>
          <button type="button" className="secondary" disabled={busy} onClick={() => void onRestart()}>
            Restart
          </button>
          <button type="button" className="secondary" disabled={busy} onClick={() => void onNewConversation()}>
            Phiên mới
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
            className="voice-button"
            title={voiceHint}
            aria-label="Bắt đầu voice turn"
            disabled={busy || !voiceReady}
            onClick={() => void onVoiceTurn()}
          >
            Mic
          </button>
          <button type="submit" className="primary" disabled={busy || !input.trim()}>
            Gửi
          </button>
        </div>
      </form>

      <footer>
        <span>Alt + Space để mở Assistant</span>
        <button
          type="button"
          className="footer-action"
          disabled={busy || !voice?.tts_available || !messages.some((item) => item.role === "assistant")}
          onClick={() => void onSpeakLast()}
        >
          Đọc lại phản hồi cuối
        </button>
      </footer>
    </main>
  );
}
