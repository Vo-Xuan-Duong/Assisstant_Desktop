import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { onAssistantEvent } from "./api";
import { copyQuickText } from "./quickClipboard";
import "./quick-recent-responses.css";

const MAX_RECENT_RESPONSES = 5;
const COPY_FEEDBACK_MS = 1_200;

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

function formatTime(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

export default function QuickRecentResponses() {
  const [items, setItems] = useState<RecentResponse[]>([]);
  const [open, setOpen] = useState(false);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const nextIdRef = useRef(1);
  const feedbackTimerRef = useRef<number | null>(null);

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];

    void onAssistantEvent((event) => {
      if (event.type !== "response_completed") return;

      const text = event.text.trim();
      if (!text) return;

      const item: RecentResponse = {
        id: nextIdRef.current++,
        text,
        createdAt: Date.now(),
      };
      setItems((current) => [item, ...current].slice(0, MAX_RECENT_RESPONSES));
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void listen("quick:shown", () => {
      setOpen(false);
      setCopiedId(null);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    return () => {
      disposed = true;
      if (feedbackTimerRef.current !== null) {
        window.clearTimeout(feedbackTimerRef.current);
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

  if (items.length === 0) return null;

  return (
    <div className="quick-history-root">
      <button
        type="button"
        className={`quick-history-toggle ${open ? "quick-history-toggle-open" : ""}`}
        title="Câu trả lời gần đây"
        aria-label="Câu trả lời gần đây"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
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
                setOpen(false);
                setCopiedId(null);
              }}
            >
              Xóa
            </button>
          </header>

          <div className="quick-history-list">
            {items.map((item) => (
              <article key={item.id} className="quick-history-item">
                <div className="quick-history-item-meta">
                  <time>{formatTime(item.createdAt)}</time>
                  <button
                    type="button"
                    title="Sao chép câu trả lời này"
                    aria-label="Sao chép câu trả lời này"
                    onClick={() => {
                      void copyQuickText(item.text).then(() => showCopied(item.id));
                    }}
                  >
                    <CopyIcon />
                    <span>{copiedId === item.id ? "Đã copy" : "Copy"}</span>
                  </button>
                </div>
                <p>{item.text}</p>
              </article>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
