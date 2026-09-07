import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { hideQuickAssistant, onAssistantEvent } from "./api";
import {
  QUICK_AUTO_DISMISS_HOLD_EVENT,
  type QuickAutoDismissHoldDetail,
} from "./quickLifecycle";
import type { AssistantState } from "./types";

const SHORT_RESPONSE_DISMISS_MS = 9_000;
const LONG_RESPONSE_DISMISS_MS = 15_000;
const LONG_RESPONSE_THRESHOLD = 320;

/**
 * Auto-dismiss policy for the lifetime-owned Quick WebView.
 *
 * The controller is intentionally separate from QuickOverlay so presentation,
 * voice/cancel behavior, and lifecycle policy can evolve independently.
 */
export function useQuickAutoDismiss() {
  useEffect(() => {
    let disposed = false;
    let assistantState: AssistantState = "idle";
    let responseReady = false;
    let responseLength = 0;
    let pointerInside = false;
    let timer: number | null = null;
    const holdSources = new Set<string>();
    const unlisten: Array<() => void> = [];

    const clearTimer = () => {
      if (timer !== null) {
        window.clearTimeout(timer);
        timer = null;
      }
    };

    const composerHasText = () => {
      const input = document.querySelector<HTMLTextAreaElement>(".quick-composer textarea");
      return Boolean(input?.value.trim());
    };

    const canDismiss = () =>
      !disposed &&
      assistantState === "idle" &&
      responseReady &&
      holdSources.size === 0 &&
      !pointerInside &&
      !composerHasText();

    const schedule = () => {
      clearTimer();
      if (!canDismiss()) return;

      const delay = responseLength > LONG_RESPONSE_THRESHOLD
        ? LONG_RESPONSE_DISMISS_MS
        : SHORT_RESPONSE_DISMISS_MS;

      timer = window.setTimeout(() => {
        timer = null;
        if (!canDismiss()) return;
        void hideQuickAssistant();
      }, delay);
    };

    void listen("quick:shown", () => {
      responseReady = false;
      responseLength = 0;
      holdSources.clear();
      clearTimer();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    void onAssistantEvent((event) => {
      switch (event.type) {
        case "state_changed":
          assistantState = event.to;
          if (event.to === "idle") schedule();
          else clearTimer();
          break;
        case "response_completed":
          responseReady = true;
          responseLength = event.text.length;
          schedule();
          break;
        case "error":
          responseReady = false;
          responseLength = 0;
          clearTimer();
          break;
        case "text_delta":
        case "tool_started":
        case "tool_finished":
          break;
      }
    }).then((fn) => {
      if (disposed) fn();
      else unlisten.push(fn);
    });

    const onPointerEnter = () => {
      pointerInside = true;
      clearTimer();
    };

    const onPointerLeave = () => {
      pointerInside = false;
      schedule();
    };

    const onInput = (event: Event) => {
      const target = event.target;
      if (!(target instanceof HTMLTextAreaElement)) return;

      if (target.value.trim()) clearTimer();
      else schedule();
    };

    const onHoldChange = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      const detail = event.detail as QuickAutoDismissHoldDetail | undefined;
      if (!detail?.source) return;

      if (detail.held) {
        holdSources.add(detail.source);
        clearTimer();
      } else {
        holdSources.delete(detail.source);
        schedule();
      }
    };

    const root = document.documentElement;
    root.addEventListener("pointerenter", onPointerEnter);
    root.addEventListener("pointerleave", onPointerLeave);
    document.addEventListener("input", onInput, true);
    window.addEventListener(QUICK_AUTO_DISMISS_HOLD_EVENT, onHoldChange);

    return () => {
      disposed = true;
      clearTimer();
      holdSources.clear();
      root.removeEventListener("pointerenter", onPointerEnter);
      root.removeEventListener("pointerleave", onPointerLeave);
      document.removeEventListener("input", onInput, true);
      window.removeEventListener(QUICK_AUTO_DISMISS_HOLD_EVENT, onHoldChange);
      for (const fn of unlisten) fn();
    };
  }, []);
}
