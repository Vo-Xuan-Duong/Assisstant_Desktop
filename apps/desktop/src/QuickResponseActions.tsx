import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { onAssistantEvent } from "./api";
import { copyQuickText } from "./quickClipboard";
import "./quick-response-actions.css";

function CopyIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="8" y="8" width="10" height="10" rx="2" />
      <path d="M6 16H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
    </svg>
  );
}

export default function QuickResponseActions() {
  const [response, setResponse] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<"copied" | "error" | null>(null);
  const feedbackTimerRef = useRef<number | null>(null);

  const clearFeedback = () => {
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
      feedbackTimerRef.current = null;
    }
    setFeedback(null);
  };

  const showFeedback = (value: "copied" | "error") => {
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
    setFeedback(value);
    feedbackTimerRef.current = window.setTimeout(() => {
      feedbackTimerRef.current = null;
      setFeedback(null);
    }, 1_400);
  };

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];

    void onAssistantEvent((event) => {
      if (event.type === "response_completed") {
        setResponse(event.text);
        clearFeedback();
      } else if (event.type === "error") {
        setResponse(null);
        clearFeedback();
      }
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void listen("quick:shown", () => {
      setResponse(null);
      clearFeedback();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    return () => {
      disposed = true;
      if (feedbackTimerRef.current !== null) {
        window.clearTimeout(feedbackTimerRef.current);
        feedbackTimerRef.current = null;
      }
      for (const fn of unlisten) fn();
    };
  }, []);

  if (!response) return null;

  const label = feedback === "copied"
    ? "Đã sao chép"
    : feedback === "error"
      ? "Không thể sao chép"
      : "Sao chép câu trả lời";

  return (
    <div className="quick-response-actions" aria-live="polite">
      <button
        type="button"
        className={`quick-response-copy ${feedback ? `quick-response-copy-${feedback}` : ""}`}
        title={label}
        aria-label={label}
        onClick={() => {
          void copyQuickText(response)
            .then(() => showFeedback("copied"))
            .catch(() => showFeedback("error"));
        }}
      >
        <CopyIcon />
        <span>{feedback === "copied" ? "Đã sao chép" : feedback === "error" ? "Lỗi" : "Copy"}</span>
      </button>
    </div>
  );
}
