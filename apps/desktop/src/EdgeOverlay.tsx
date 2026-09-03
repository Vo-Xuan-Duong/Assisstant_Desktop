import { useEffect, useRef, useState, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { onAssistantEvent, onVoiceLevel } from "./api";
import type { AssistantState } from "./types";
import "./edge.css";

type Edge = "top" | "right" | "bottom" | "left";
type EdgeMode = AssistantState | "activated";

interface EdgeModePayload {
  mode: EdgeMode;
}

function parseEdge(): Edge {
  const value = new URLSearchParams(window.location.search).get("edge");
  if (value === "right" || value === "bottom" || value === "left") return value;
  return "top";
}

export default function EdgeOverlay() {
  const edge = parseEdge();
  const [mode, setMode] = useState<EdgeMode>("idle");
  const [voiceLevel, setVoiceLevel] = useState(0);
  const activationTimer = useRef<number | null>(null);

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];

    const clearActivationTimer = () => {
      if (activationTimer.current !== null) {
        window.clearTimeout(activationTimer.current);
        activationTimer.current = null;
      }
    };

    void listen<EdgeModePayload>("edge:mode", ({ payload }) => {
      clearActivationTimer();
      setMode(payload.mode);
      if (payload.mode === "activated") {
        activationTimer.current = window.setTimeout(() => {
          setMode("idle");
          activationTimer.current = null;
        }, 950);
      }
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void onAssistantEvent((event) => {
      if (event.type === "state_changed") {
        clearActivationTimer();
        setMode(event.to);
        if (event.to !== "listening") setVoiceLevel(0);
      }
      if (event.type === "error") {
        clearActivationTimer();
        setMode("error");
        setVoiceLevel(0);
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

    return () => {
      disposed = true;
      clearActivationTimer();
      for (const fn of unlisten) fn();
    };
  }, []);

  const style = {
    "--voice-level": voiceLevel.toFixed(3),
    "--voice-opacity": (0.48 + voiceLevel * 0.5).toFixed(3),
    "--voice-scale": (0.48 + voiceLevel * 0.5).toFixed(3),
  } as CSSProperties;

  return (
    <div
      className={`edge-surface edge-${edge} edge-mode-${mode}`}
      data-edge={edge}
      data-mode={mode}
      style={style}
      aria-hidden="true"
    >
      <div className="edge-aura" />
      <div className="edge-flow" />
      <div className="edge-core" />
    </div>
  );
}
