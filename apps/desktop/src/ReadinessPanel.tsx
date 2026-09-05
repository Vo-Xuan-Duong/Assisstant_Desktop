import { useCallback, useEffect, useMemo, useState } from "react";
import { getRuntimeReadiness } from "./api";
import ResourceSetupPanel from "./ResourceSetupPanel";
import type { ReadinessLevel, RuntimeReadinessReport } from "./types";
import "./readiness.css";

const levelLabel: Record<ReadinessLevel, string> = {
  ready: "Ready",
  optional_missing: "Optional",
  blocking: "Blocking",
};

function summary(report: RuntimeReadinessReport | null): string {
  if (!report) return "Chưa kiểm tra";
  if (report.overall === "blocking") return "Có thành phần bắt buộc chưa sẵn sàng";
  if (report.overall === "optional_missing") return "Core sẵn sàng · thiếu thành phần tùy chọn";
  return "Full runtime sẵn sàng";
}

interface ReadinessPanelProps {
  onWakeChanged?: () => void | Promise<void>;
  open?: boolean;
  onClose?: () => void;
  showTrigger?: boolean;
}

export default function ReadinessPanel({
  onWakeChanged,
  open: controlledOpen,
  onClose,
  showTrigger = true,
}: ReadinessPanelProps) {
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;

  const [loading, setLoading] = useState(false);
  const [report, setReport] = useState<RuntimeReadinessReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleClose = useCallback(() => {
    if (isControlled) {
      onClose?.();
    } else {
      setInternalOpen(false);
    }
  }, [isControlled, onClose]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setReport(await getRuntimeReadiness());
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  const resourcesChanged = useCallback(async () => {
    await refresh();
    await onWakeChanged?.();
  }, [onWakeChanged, refresh]);

  const toggle = useCallback(() => {
    if (isControlled) {
      if (open) onClose?.();
    } else {
      setInternalOpen((current) => {
        const next = !current;
        if (next && !report && !loading) void refresh();
        return next;
      });
    }
  }, [isControlled, open, onClose, report, loading, refresh]);

  useEffect(() => {
    if (open && !report && !loading) {
      void refresh();
    }
  }, [open, report, loading, refresh]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, handleClose]);

  const counts = useMemo(() => {
    const result = { ready: 0, optional_missing: 0, blocking: 0 };
    for (const check of report?.checks ?? []) result[check.level] += 1;
    return result;
  }, [report]);

  return (
    <div className="readiness-root">
      {showTrigger && (
        <button
          type="button"
          className={`readiness-trigger readiness-${report?.overall ?? "unknown"}`}
          onClick={toggle}
          aria-expanded={open}
        >
          Readiness
        </button>
      )}

      {open && (
        <div className="readiness-backdrop" onClick={handleClose} role="presentation">
          <section
            className="readiness-panel"
            aria-label="Runtime readiness"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
          >
            <div className="readiness-header">
              <div>
                <span className="section-label">RUNTIME READINESS</span>
                <strong>{summary(report)}</strong>
              </div>
              <div className="readiness-header-actions">
                <ResourceSetupPanel onResourcesChanged={resourcesChanged} />
                <button
                  type="button"
                  className="readiness-refresh"
                  disabled={loading}
                  onClick={() => void refresh()}
                >
                  {loading ? "Đang kiểm tra" : "Kiểm tra lại"}
                </button>
                <button
                  type="button"
                  className="readiness-close"
                  onClick={handleClose}
                  aria-label="Đóng"
                >
                  ×
                </button>
              </div>
            </div>

            {report && (
              <div className="readiness-summary">
                <span className="readiness-count readiness-count-ready">{counts.ready} ready</span>
                <span className="readiness-count readiness-count-optional">{counts.optional_missing} optional</span>
                <span className="readiness-count readiness-count-blocking">{counts.blocking} blocking</span>
              </div>
            )}

            {error && <p className="readiness-error">Không thể tạo readiness report: {error}</p>}

            <div className="readiness-checks">
              {report?.checks.map((check) => (
                <article key={check.id} className={`readiness-check readiness-check-${check.level}`}>
                  <div className="readiness-check-title">
                    <strong>{check.label}</strong>
                    <span>{levelLabel[check.level]}</span>
                  </div>
                  <p>{check.detail}</p>
                  {check.path && <code>{check.path}</code>}
                </article>
              ))}
              {!report && !error && <p className="readiness-empty">{loading ? "Đang kiểm tra runtime..." : "Chưa có dữ liệu."}</p>}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
