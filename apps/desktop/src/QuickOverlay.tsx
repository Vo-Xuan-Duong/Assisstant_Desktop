import {
  type CSSProperties,
  type FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { emit, listen } from "@tauri-apps/api/event";
import {
  getVoiceCapabilities,
  hideQuickAssistant,
  onAssistantEvent,
  onVoiceLevel,
  runVoiceTurn,
  submitPrompt,
} from "./api";
import type { AssistantState, VoiceCapabilities } from "./types";
import "./quick.css";

interface QuickShownPayload {
  reason: "shortcut" | "wake" | "cli" | string;
}

const WAKE_TO_COMMAND_DELAY_MS = 180;
const QUICK_MIN_HEIGHT = 206;
const QUICK_MAX_HEIGHT = 380;
const QUICK_RESIZE_EVENT = "quick:resize_request";

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
  const shellRef = useRef<HTMLElement | null>(null);
  const cardRef = useRef<HTMLElement | null>(null);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const busyRef = useRef(false);
  const wakeVoiceStarterRef = useRef<() => void>(() => {});
  const wakeTimerRef = useRef<number | null>(null);
  const resizeFrameRef = useRef<number | null>(null);
  const lastRequestedHeightRef = useRef<number | null>(null);

  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);

  const scheduleResize = useCallback(() => {
    if (resizeFrameRef.current !== null) {
      window.cancelAnimationFrame(resizeFrameRef.current);
    }

    resizeFrameRef.current = window.requestAnimationFrame(() => {
      resizeFrameRef.current = null;
      const shell = shellRef.current;
      const card = cardRef.current;
      if (!shell || !card) return;

      const shellStyle = window.getComputedStyle(shell);
      const topPadding = Number.parseFloat(shellStyle.paddingTop) || 0;
      const bottomPadding = Number.parseFloat(shellStyle.paddingBottom) || 0;
      const measured = Math.ceil(card.scrollHeight + topPadding + bottomPadding);
      const height = Math.max(QUICK_MIN_HEIGHT, Math.min(QUICK_MAX_HEIGHT, measured));

      if (lastRequestedHeightRef.current === height) return;
      lastRequestedHeightRef.current = height;
      void emit(QUICK_RESIZE_EVENT, { height }).catch(() => {
        lastRequestedHeightRef.current = null;
      });
    });
  }, []);

  useEffect(() => {
    const card = cardRef.current;
    if (!card) return;

    const observer = typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(() => scheduleResize());
    observer?.observe(card);
    window.addEventListener("resize", scheduleResize);
    scheduleResize();

    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", scheduleResize);
      if (resizeFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
    };
  }, [scheduleResize]);

  const refreshVoice = useCallback(async () => {
    try {
      const capabilities = await getVoiceCapabilities();
      setVoice(capabilities);
      return capabilities;
    } catch {
      setVoice(null);
      return null;
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
      setResponse(null);
      lastRequestedHeightRef.current = null;

      if (wakeTimerRef.current !== null) {
        window.clearTimeout(wakeTimerRef.current);
        wakeTimerRef.current = null;
      }

      window.setTimeout(() => inputRef.current?.focus(), payload.reason === "wake" ? 90 : 20);
      void refreshVoice();

      // The Quick WebView exists for the lifetime of the background runtime,
      // even while hidden. It therefore owns wake-triggered voice turns instead
      // of relying on the retired full MainSurface. The delay preserves the
      // previous wake-to-command gap so the wake phrase tail is not captured.
      if (payload.reason === "wake") {
        wakeTimerRef.current = window.setTimeout(() => {
          wakeTimerRef.current = null;
          wakeVoiceStarterRef.current();
        }, WAKE_TO_COMMAND_DELAY_MS);
      }
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
      if (wakeTimerRef.current !== null) {
        window.clearTimeout(wakeTimerRef.current);
        wakeTimerRef.current = null;
      }
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

  useEffect(() => {
    scheduleResize();
  }, [assistantState, displayedResponse, error, scheduleResize]);

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

    const capabilities = await refreshVoice();
    const ready = Boolean(capabilities?.whisper_compiled && capabilities.model_available);
    if (!ready) {
      setError("STT tiếng Việt chưa sẵn sàng. Dùng `assistant resources install stt_zipformer_vi` trong terminal.");
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
  }, [refreshVoice]);

  wakeVoiceStarterRef.current = () => {
    void startVoice();
  };

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    void send();
  }

  return (
    <main
      ref={shellRef}
      className={`quick-shell quick-state-${assistantState} ${active ? "quick-active" : ""}`}
      style={style}
    >
      <div className="quick-outer-glow" aria-hidden="true" />
      <section ref={cardRef} className="quick-card" aria-label="Quick Assistant">
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
            title={voiceReady ? "Nói với Assistant" : "STT chưa sẵn sàng"}
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
