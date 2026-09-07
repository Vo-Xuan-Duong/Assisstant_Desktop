import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { onAssistantEvent, speakResponse } from "./api";
import { copyQuickText } from "./quickClipboard";
import type { AssistantState } from "./types";
import "./quick-response-actions.css";

function isCancellationError(cause: unknown) {
  return String(cause).toLowerCase().includes("cancel");
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

export default function QuickResponseActions() {
  const [response, setResponse] = useState<string | null>(null);
  const [assistantState, setAssistantState] = useState<AssistantState>("idle");
  const [copyFeedback, setCopyFeedback] = useState<"copied" | "error" | null>(null);
  const [speakError, setSpeakError] = useState(false);
  const [speakPending, setSpeakPending] = useState(false);
  const copyTimerRef = useRef<number | null>(null);
  const speakTimerRef = useRef<number | null>(null);

  const clearCopyFeedback = () => {
    if (copyTimerRef.current !== null) {
      window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = null;
    }
    setCopyFeedback(null);
  };

  const clearSpeakFeedback = () => {
    if (speakTimerRef.current !== null) {
      window.clearTimeout(speakTimerRef.current);
      speakTimerRef.current = null;
    }
    setSpeakError(false);
  };

  const showCopyFeedback = (value: "copied" | "error") => {
    if (copyTimerRef.current !== null) {
      window.clearTimeout(copyTimerRef.current);
    }
    setCopyFeedback(value);
    copyTimerRef.current = window.setTimeout(() => {
      copyTimerRef.current = null;
      setCopyFeedback(null);
    }, 1_400);
  };

  const showSpeakError = () => {
    if (speakTimerRef.current !== null) {
      window.clearTimeout(speakTimerRef.current);
    }
    setSpeakError(true);
    speakTimerRef.current = window.setTimeout(() => {
      speakTimerRef.current = null;
      setSpeakError(false);
    }, 1_600);
  };

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];

    void onAssistantEvent((event) => {
      if (event.type === "state_changed") {
        setAssistantState(event.to);
      } else if (event.type === "response_completed") {
        setResponse(event.text);
        clearCopyFeedback();
        clearSpeakFeedback();
      } else if (event.type === "error") {
        setResponse(null);
        setSpeakPending(false);
        clearCopyFeedback();
        clearSpeakFeedback();
      }
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void listen("quick:shown", () => {
      setResponse(null);
      setSpeakPending(false);
      clearCopyFeedback();
      clearSpeakFeedback();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    return () => {
      disposed = true;
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
      if (speakTimerRef.current !== null) {
        window.clearTimeout(speakTimerRef.current);
      }
      for (const fn of unlisten) fn();
    };
  }, []);

  if (!response) return null;

  const copyLabel = copyFeedback === "copied"
    ? "Đã sao chép"
    : copyFeedback === "error"
      ? "Không thể sao chép"
      : "Sao chép câu trả lời";
  const speaking = speakPending || assistantState === "speaking";
  const canSpeak = assistantState === "idle" && !speakPending;
  const speakLabel = speakError
    ? "Không thể đọc câu trả lời"
    : speaking
      ? "Đang đọc câu trả lời"
      : "Đọc câu trả lời";

  return (
    <div className="quick-response-actions" aria-live="polite">
      <button
        type="button"
        className={`quick-response-action quick-response-copy ${copyFeedback ? `quick-response-copy-${copyFeedback}` : ""}`}
        title={copyLabel}
        aria-label={copyLabel}
        onClick={() => {
          void copyQuickText(response)
            .then(() => showCopyFeedback("copied"))
            .catch(() => showCopyFeedback("error"));
        }}
      >
        <CopyIcon />
        <span>{copyFeedback === "copied" ? "Đã sao chép" : copyFeedback === "error" ? "Lỗi" : "Copy"}</span>
      </button>

      <button
        type="button"
        className={`quick-response-action quick-response-speak ${speakError ? "quick-response-speak-error" : ""}`}
        title={speakLabel}
        aria-label={speakLabel}
        disabled={!canSpeak}
        onClick={() => {
          if (!canSpeak) return;
          clearSpeakFeedback();
          setSpeakPending(true);
          void speakResponse(response)
            .catch((cause) => {
              if (!isCancellationError(cause)) showSpeakError();
            })
            .finally(() => setSpeakPending(false));
        }}
      >
        <SpeakerIcon />
        <span>{speakError ? "Lỗi" : speaking ? "Đang đọc" : "Đọc"}</span>
      </button>
    </div>
  );
}
