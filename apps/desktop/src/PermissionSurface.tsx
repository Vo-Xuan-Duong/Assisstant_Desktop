import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { onPermissionRequest, submitPermissionDecision } from "./permissionApi";
import type { PermissionRequest } from "./types";
import "./permission-surface.css";

const PERMISSION_UI_TIMEOUT_SECONDS = 29;

function formatArguments(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "Không thể hiển thị arguments.";
  }
}

export default function PermissionSurface() {
  const [queue, setQueue] = useState<PermissionRequest[]>([]);
  const [remaining, setRemaining] = useState(PERMISSION_UI_TIMEOUT_SECONDS);
  const respondingRef = useRef(false);
  const active = queue[0] ?? null;

  const resolve = useCallback(
    async (approved: boolean) => {
      const request = active;
      if (!request || respondingRef.current) return;

      respondingRef.current = true;
      try {
        await submitPermissionDecision(request.request_id, approved);
      } catch {
        // The native broker may already have timed out. Do not surface the
        // request arguments through logs/errors from this minimal UI.
      } finally {
        setQueue((current) =>
          current.filter((item) => item.request_id !== request.request_id),
        );
        respondingRef.current = false;
      }
    },
    [active],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void onPermissionRequest((request) => {
      setQueue((current) => {
        if (current.some((item) => item.request_id === request.request_id)) {
          return current;
        }
        return [...current, request];
      });
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!active) {
      setRemaining(PERMISSION_UI_TIMEOUT_SECONDS);
      return;
    }

    setRemaining(PERMISSION_UI_TIMEOUT_SECONDS);
    const startedAt = Date.now();
    const ticker = window.setInterval(() => {
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      setRemaining(Math.max(0, PERMISSION_UI_TIMEOUT_SECONDS - elapsed));
    }, 250);
    const timeoutId = window.setTimeout(() => {
      void resolve(false);
    }, PERMISSION_UI_TIMEOUT_SECONDS * 1000);

    return () => {
      window.clearInterval(ticker);
      window.clearTimeout(timeoutId);
    };
  }, [active?.request_id, resolve]);

  useEffect(() => {
    if (active || queue.length > 0) return;
    const timer = window.setTimeout(() => {
      void getCurrentWindow().hide();
    }, 120);
    return () => window.clearTimeout(timer);
  }, [active, queue.length]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && active) {
        event.preventDefault();
        void resolve(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [active, resolve]);

  return (
    <main className="permission-surface-shell">
      {active ? (
        <section
          className="permission-surface-card"
          role="dialog"
          aria-modal="true"
          aria-labelledby="permission-surface-title"
        >
          <header className="permission-surface-heading">
            <div>
              <p>SENSITIVE TOOL</p>
              <h1 id="permission-surface-title">Xác nhận hành động</h1>
            </div>
            <span>{active.risk}</span>
          </header>

          <p className="permission-surface-copy">
            Assistant muốn thực thi <strong>{active.tool_name}</strong>. Hành động chỉ tiếp tục
            nếu bạn cho phép lần này.
          </p>

          <div className="permission-surface-meta">
            <span>Request</span>
            <code>{active.request_id}</code>
          </div>

          <div className="permission-surface-arguments">
            <span>Arguments</span>
            <pre>{formatArguments(active.arguments)}</pre>
          </div>

          <div className="permission-surface-timeout">
            Tự động từ chối sau <strong>{remaining}s</strong>
            {queue.length > 1 && ` · ${queue.length - 1} yêu cầu đang chờ`}
          </div>

          <div className="permission-surface-actions">
            <button type="button" className="deny" onClick={() => void resolve(false)}>
              Từ chối
            </button>
            <button type="button" className="allow" onClick={() => void resolve(true)}>
              Cho phép một lần
            </button>
          </div>
        </section>
      ) : (
        <section className="permission-surface-card permission-surface-idle" aria-live="polite">
          <p>ASSISSTANT DESKTOP</p>
          <h1>Assistant đang chạy nền</h1>
          <span>Quản trị hệ thống bằng terminal:</span>
          <code>assistant</code>
        </section>
      )}
    </main>
  );
}
