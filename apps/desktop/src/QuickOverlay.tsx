import {
  type CSSProperties,
  type FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getVoiceCapabilities,
  hideQuickAssistant,
  onAssistantEvent,
  onVoiceLevel,
  openFullAssistant,
  runVoiceTurn,
  submitPrompt,
} from "./api";
import type { AssistantState, VoiceCapabilities } from "./types";
import "./quick.css";

interface QuickShownPayload {
  reason: "shortcut" | "wake" | string;
}

function SparkIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 1.8c.55 4.8 3.35 7.6 8.2 8.2-4.85.58-7.65 3.4-8.2 8.2-.58-4.8-3.4-7.62-8.2-8.2 4.8-.6 7.62-3.4 8.2-8.2Z" />
      <path d="M19.2 15.2c.22 1.9 1.3 3 3.2 3.2-1.9.22-2.98 1.3-3.2 3.2-.22-1.9-1.3-2.98-3.2-3.2 1.9-.2 2.98-1.3 3.2-3.2Z" />
    </svg>
  );
}

function MicIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 14.4a3.35 3.35 0 0 0 3.35-3.35V6.4a3.35 3.35 0 1 0-6.7 0v4.65A3.35 3.35 0 0 0 12 14.4Z" />
      <path d="M6.55 10.75a5.45 5.45 0 0 0 10.9 0M12 16.2v4.1M8.8 20.3h6.4" />
    </svg>
  );
}

function SendIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M5 12h13M13 6l6 6-6 6" />
    </svg>
  );
}

function ExpandIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M8.5 4H4v4.5M15.5 4H20v4.5M8.5 20H4v-4.5M15.5 20H20v-4.5" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m7 7 10 10M17 7 7 17" />
    </svg>
  );
}

export default function QuickOverlay() {
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");
  const [voice, setVoice] = useState<VoiceCapabilities | null>(null);
  const [voiceLevel, setVoiceLevel] = useState(0);
  const [input, setInput] = useState("");
  const [response, setResponse] = useState<string | null>(null);
  const [streamingText, setStreamingText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const busyRef = useRef(false);

  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);

  const refreshVoice = useCallback(async () => {
    try {
      setVoice(await getVoiceCapabilities());
    } catch {
      setVoice(null);
    }
  }, []);

  useEffect(() => {
    void refreshVoice();
  }, [refreshVoice]);

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];

    void listen<QuickShownPayload>("quick:shown", ({ payload }) => {
      setError(null);
      setStreamingText("");
      // A shortcut invocation behaves like Gemini's fresh overlay. Wake keeps
      // the surface clean as well; the response will arrive through core events.
      setResponse(null);
      window.setTimeout(() => inputRef.current?.focus(), payload.reason === "wake" ? 90 : 20);
      void refreshVoice();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void onAssistantEvent((event) => {
      if (event.type === "state_changed") {
        setAssistantState(event.to);
        if (event.to !== "listening") setVoiceLevel(0);
        if (event.to === "idle") {
          setBusy(false);
          setStreamingText("");
        }
      } else if (event.type === "text_delta") {
        setStreamingText((current) => current + event.text);
      } else if (event.type === "response_completed") {
        setResponse(event.text);
        setStreamingText("");
        setBusy(false);
      } else if (event.type === "error") {
        setAssistantState("error");
        setError(event.message);
        setVoiceLevel(0);
        setBusy(false);
      }
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void onVoiceLevel((level) => {
      setVoiceLevel(Math.max(0, Math.min(1, level.rms * 8)));
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void hideQuickAssistant();
      }
    };
    window.addEventListener("keydown", onKeyDown);

    return () => {
      disposed = true;
      window.removeEventListener("keydown", onKeyDown);
      for (const fn of unlisten) fn();
    };
  }, [refreshVoice]);

  const statusLabel = useMemo(() => {
    switch (assistantState) {
      case "listening":
        return "Đang nghe…";
      case "processing":
        return "Đang suy luận…";
      case "executing":
        return "Đang thực hiện…";
      case "confirming":
        return "Cần xác nhận";
      case "speaking":
        return "Đang trả lời…";
      case "error":
        return "Có lỗi";
      default:
        return response ? "Sẵn sàng cho câu tiếp theo" : "Hỏi Assistant";
    }
  }, [assistantState, response]);

  const voiceReady = Boolean(voice?.whisper_compiled && voice.model_available);
  const displayedResponse = streamingText || response;
  const active = busy || !["idle", "error"].includes(assistantState);
  const style = {
    "--voice-level": voiceLevel.toFixed(3),
  } as CSSProperties;

  const send = useCallback(async () => {
    const prompt = input.trim();
    if (!prompt || busyRef.current) return;

    busyRef.current = true;
    setBusy(true);
    setInput("");
    setResponse(null);
    setStreamingText("");
    setError(null);

    try {
      const result = await submitPrompt(prompt);
      setResponse(result);
    } catch (cause) {
      setError(`Không thể hoàn thành yêu cầu: ${String(cause)}`);
    } finally {
      busyRef.current = false;
      setBusy(false);
      window.setTimeout(() => inputRef.current?.focus(), 20);
    }
  }, [input]);

  const startVoice = useCallback(async () => {
    if (busyRef.current) return;
    if (!voiceReady) {
      setError("Whisper voice chưa sẵn sàng. Mở ứng dụng đầy đủ để thiết lập Tài nguyên.");
      return;
    }

    busyRef.current = true;
    setBusy(true);
    setResponse(null);
    setStreamingText("");
    setError(null);
    setVoiceLevel(0);

    try {
      const result = await runVoiceTurn();
      setResponse(result.response);
      if (result.tts_error) {
        setError(`TTS: ${result.tts_error}`);
      }
    } catch (cause) {
      setError(`Voice turn thất bại: ${String(cause)}`);
    } finally {
      busyRef.current = false;
      setBusy(false);
      setVoiceLevel(0);
      window.setTimeout(() => inputRef.current?.focus(), 20);
    }
  }, [voiceReady]);

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    void send();
  }

  return (
    <main
      className={`quick-shell quick-state-${assistantState} ${active ? "quick-active" : ""}`}
      style={style}
    >
      <div className="quick-outer-glow" aria-hidden="true" />
      <section className="quick-card" aria-label="Quick Assistant">
        <header className="quick-header">
          <div className="quick-status">
            <span className="quick-spark"><SparkIcon /></span>
            <span>{statusLabel}</span>
            {assistantState === "listening" && (
              <span className="quick-listening-dots" aria-hidden="true">
                <i /><i /><i /><i />
              </span>
            )}
          </div>
          <div className="quick-window-actions">
            <button
              type="button"
              title="Mở ứng dụng đầy đủ"
              aria-label="Mở ứng dụng đầy đủ"
              onClick={() => void openFullAssistant()}
            >
              <ExpandIcon />
            </button>
            <button
              type="button"
              title="Đóng (Esc)"
              aria-label="Đóng quick assistant"
              onClick={() => void hideQuickAssistant()}
            >
              <CloseIcon />
            </button>
          </div>
        </header>

        <div className={`quick-answer ${displayedResponse || error ? "quick-answer-visible" : ""}`} aria-live="polite">
          {error ? (
            <p className="quick-error">{error}</p>
          ) : displayedResponse ? (
            <p>{displayedResponse}</p>
          ) : (
            <p className="quick-hint">Alt + Space để ẩn/hiện · Enter để gửi · Shift + Enter xuống dòng</p>
          )}
        </div>

        <form className="quick-composer" onSubmit={onSubmit}>
          <span className="quick-input-mark" aria-hidden="true"><SparkIcon /></span>
          <textarea
            ref={inputRef}
            value={input}
            rows={1}
            disabled={busy || assistantState === "listening"}
            placeholder={assistantState === "listening" ? "Đang nghe bạn nói…" : "Hỏi Assistant…"}
            aria-label="Nhập câu hỏi cho Assistant"
            onChange={(event) => setInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                void send();
              }
            }}
          />

          <button
            type="button"
            className={`quick-mic ${assistantState === "listening" ? "quick-mic-listening" : ""}`}
            disabled={busy && assistantState !== "listening"}
            title={voiceReady ? "Nói với Assistant" : "Whisper chưa sẵn sàng"}
            aria-label="Nói với Assistant"
            onClick={() => void startVoice()}
          >
            <span className="quick-mic-ring" aria-hidden="true" />
            <MicIcon />
          </button>

          <button
            type="submit"
            className={`quick-send ${input.trim() ? "quick-send-visible" : ""}`}
            disabled={!input.trim() || busy}
            title="Gửi"
            aria-label="Gửi câu hỏi"
          >
            <SendIcon />
          </button>
        </form>
      </section>
    </main>
  );
}
