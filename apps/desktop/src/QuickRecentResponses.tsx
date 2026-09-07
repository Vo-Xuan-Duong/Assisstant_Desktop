import { useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { onAssistantEvent, speakResponse } from "./api";
import { copyQuickText } from "./quickClipboard";
import { setQuickAutoDismissHold } from "./quickLifecycle";
import type { AssistantState } from "./types";
import "./quick-recent-responses.css";

const MAX_RECENT_RESPONSES = 5;
const COPY_FEEDBACK_MS = 1_200;
const SPEAK_ERROR_MS = 1_600;
const QUICK_RESIZE_EVENT = "quick:resize_request";
const QUICK_HISTORY_HEIGHT = 380;
const DUPLICATE_EVENT_WINDOW_MS = 500;
const AUTO_DISMISS_HOLD_SOURCE = "recent-responses";

interface RecentResponse {
  id: number;
  text: string;
  createdAt: number;
}

function HistoryIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4.8 7.8A8 8 0 1 1 4 14" />
      <path d="M4 4v4h4M12 7v5l3.2 2" />
    </svg>
  );
}

function CopyIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="8" y="8" width="10" height="10" rx="2" />
      <path d="M6 16H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
    </svg>
  );
}

function SpeakerIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M5 10v4h3l4 3V7L8 10H5Z" />
      <path d="M15 9.2a4 4 0 0 1 0 5.6M17.6 6.8a7.4 7.4 0 0 1 0 10.4" />
    </svg>
  );
}

function formatTime(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

export default function QuickRecentResponses() {
  const [items, setItems] = useState<RecentResponse[]>([]);
  const [open, setOpen] = useState(false);
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [speakPendingId, setSpeakPendingId] = useState<number | null>(null);
  const [speakErrorId, setSpeakErrorId] = useState<number | null>(null);
  const nextIdRef = useRef(1);
  const feedbackTimerRef = useRef<number | null>(null);
  const speakErrorTimerRef = useRef<number | null>(null);
  const previousHeightRef = useRef<number | null>(null);

  const requestHeight = (height: number) => {
    void emit(QUICK_RESIZE_EVENT, { height });
  };

  const closeDrawer = () => {
    setQuickAutoDismissHold(AUTO_DISMISS_HOLD_SOURCE, false);
    setOpen(false);
    const previousHeight = previousHeightRef.current;
    previousHeightRef.current = null;
    if (previousHeight !== null) {
      requestHeight(previousHeight);
    }
  };

  const toggleDrawer = () => {
    if (open) {
      closeDrawer();
      return;
    }

    previousHeightRef.current = Math.max(1, Math.round(window.innerHeight));
    setQuickAutoDismissHold(AUTO_DISMISS_HOLD_SOURCE, true);
    requestHeight(QUICK_HISTORY_HEIGHT);
    setOpen(true);
  };

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];

    void onAssistantEvent((event) => {
      if (event.type === "state_changed") {
        setAssistantState(event.to);
        return;
      }
      if (event.type !== "response_completed") return;

      const text = event.text.trim();
      if (!text) return;

      const now = Date.now();
      setItems((current) => {
        const newest = current[0];
        if (
          newest &&
          newest.text === text &&
          now - newest.createdAt <= DUPLICATE_EVENT_WINDOW_MS
        ) {
          return current;
        }

        const item: RecentResponse = {
          id: nextIdRef.current++,
          text,
          createdAt: now,
        };
        return [item, ...current].slice(0, MAX_RECENT_RESPONSES);
      });
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void listen("quick:shown", () => {
      setQuickAutoDismissHold(AUTO_DISMISS_HOLD_SOURCE, false);
      setOpen(false);
      setCopiedId(null);
      setSpeakPendingId(null);
      setSpeakErrorId(null);
      previousHeightRef.current = null;
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    return () => {
      disposed = true;
      setQuickAutoDismissHold(AUTO_DISMISS_HOLD_SOURCE, false);
      if (feedbackTimerRef.current !== null) {
        window.clearTimeout(feedbackTimerRef.current);
      }
      if (speakErrorTimerRef.current !== null) {
        window.clearTimeout(speakErrorTimerRef.current);
      }
      for (const fn of unlisten) fn();
    };
  }, []);

  const showCopied = (id: number) => {
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
    setCopiedId(id);
    feedbackTimerRef.current = window.setTimeout(() => {
      feedbackTimerRef.current = null;
      setCopiedId(null);
    }, COPY_FEEDBACK_MS);
  };

  const showSpeakError = (id: number) => {
    if (speakErrorTimerRef.current !== null) {
      window.clearTimeout(speakErrorTimerRef.current);
    }
    setSpeakErrorId(id);
    speakErrorTimerRef.current = window.setTimeout(() => {
      speakErrorTimerRef.current = null;
      setSpeakErrorId(null);
    }, SPEAK_ERROR_MS);
  };

  const canSpeak = assistantState === "idle" && speakPendingId === null;

  if (items.length === 0) return null;

  return (
    <div className="quick-history-root">
      <button
        type="button"
        className={`quick-history-toggle ${open ? "quick-history-toggle-open" : ""}`}
        title="Câu trả lời gần đây"
        aria-label="Câu trả lời gần đây"
        aria-expanded={open}
        onClick={toggleDrawer}
      >
        <HistoryIcon />
        <span>{items.length}</span>
      </button>

      {open && (
        <section className="quick-history-panel" aria-label="Câu trả lời gần đây">
          <header className="quick-history-header">
            <div>
              <strong>Gần đây</strong>
              <span>Chỉ lưu trong phiên chạy hiện tại</span>
            </div>
            <button
              type="button"
              className="quick-history-clear"
              onClick={() => {
                setItems([]);
                setCopiedId(null);
                setSpeakPendingId(null);
                setSpeakErrorId(null);
                closeDrawer();
              }}
            >
              Xóa
            </button>
          </header>

          <div className="quick-history-list">
            {items.map((item) => {
              const speakingThisItem = speakPendingId === item.id || (
                assistantState === "speaking" && speakPendingId === item.id
              );
              const speakFailed = speakErrorId === item.id;

              return (
                <article key={item.id} className="quick-history-item">
                  <div className="quick-history-item-meta">
                    <time>{formatTime(item.createdAt)}</time>
                    <div className="quick-history-item-actions">
                      <button
                        type="button"
                        title="Sao chép câu trả lời này"
                        aria-label="Sao chép câu trả lời này"
                        onClick={() => {
                          void copyQuickText(item.text)
                            .then(() => showCopied(item.id))
                            .catch(() => setCopiedId(null));
                        }}
                      >
                        <CopyIcon />
                        <span>{copiedId === item.id ? "Đã copy" : "Copy"}</span>
                      </button>
                      <button
                        type="button"
                        className={speakFailed ? "quick-history-speak-error" : ""}
                        title={speakFailed ? "Không thể đọc câu trả lời này" : "Đọc câu trả lời này"}
                        aria-label={speakFailed ? "Không thể đọc câu trả lời này" : "Đọc câu trả lời này"}
                        disabled={!canSpeak}
                        onClick={() => {
                          if (!canSpeak) return;
                          setSpeakErrorId(null);
                          setSpeakPendingId(item.id);
                          void speakResponse(item.text)
                            .catch(() => showSpeakError(item.id))
                            .finally(() => setSpeakPendingId(null));
                        }}
                      >
                        <SpeakerIcon />
                        <span>{speakFailed ? "Lỗi" : speakingThisItem ? "Đang đọc" : "Đọc"}</span>
                      </button>
                    </div>
                  </div>
                  <p>{item.text}</p>
                </article>
              );
            })}
          </div>
        </section>
      )}
    </div>
  );
}
