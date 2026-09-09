import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getRuntimeReadiness } from "./api";
import {
  loadAcceptanceChecklist,
  resetAcceptanceChecklist,
  setAcceptanceStatus,
  type AcceptanceGate,
  type AcceptanceItem,
  type AcceptanceSnapshot,
  type AcceptanceStatus,
} from "./acceptanceChecklist";
import { setQuickAutoDismissHold } from "./quickLifecycle";
import type { RuntimeReadinessReport } from "./types";
import "./release-checklist.css";

const PANEL_HOLD_SOURCE = "release-checklist";
type StatusFilter = "all" | AcceptanceStatus;

function statusLabel(status: AcceptanceStatus): string {
  switch (status) {
    case "pending": return "Pending";
    case "passed": return "Passed";
    case "failed": return "Failed";
    case "blocked": return "Blocked";
  }
}

function gateLabel(gate: AcceptanceGate): string {
  switch (gate) {
    case "ready": return "Ready";
    case "blocked": return "Blocked";
    case "pending": return "Pending";
  }
}

function formatUnix(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "Chưa kiểm tra";
  return new Intl.DateTimeFormat("vi-VN", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value * 1000));
}

function combinedGate(
  checklist: AcceptanceSnapshot | null,
  readiness: RuntimeReadinessReport | null,
  readinessError: string | null,
  checklistError: string | null,
): AcceptanceGate {
  if (readinessError || checklistError) return "blocked";
  if (!checklist || !readiness) return "pending";
  if (readiness.overall === "blocking") return "blocked";
  return checklist.gate;
}

export default function ReleaseChecklist() {
  const [open, setOpen] = useState(false);
  const [checklist, setChecklist] = useState<AcceptanceSnapshot | null>(null);
  const [readiness, setReadiness] = useState<RuntimeReadinessReport | null>(null);
  const [checklistError, setChecklistError] = useState<string | null>(null);
  const [readinessError, setReadinessError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [filter, setFilter] = useState<StatusFilter>("all");

  const loadLocal = useCallback(() => {
    try {
      setChecklist(loadAcceptanceChecklist());
      setChecklistError(null);
    } catch (cause) {
      setChecklist(null);
      setChecklistError(String(cause));
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    loadLocal();
    setReadinessError(null);
    try {
      setReadiness(await getRuntimeReadiness());
    } catch (cause) {
      setReadiness(null);
      setReadinessError(String(cause));
    } finally {
      setLoading(false);
    }
  }, [loadLocal]);

  const close = useCallback(() => {
    setOpen(false);
    setQuickAutoDismissHold(PANEL_HOLD_SOURCE, false);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("quick:shown", () => {
      if (!disposed) close();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
      setQuickAutoDismissHold(PANEL_HOLD_SOURCE, false);
    };
  }, [close]);

  const show = () => {
    setOpen(true);
    setQuickAutoDismissHold(PANEL_HOLD_SOURCE, true);
    void refresh();
  };

  const updateStatus = (item: AcceptanceItem, status: AcceptanceStatus) => {
    if (savingId) return;
    setSavingId(item.id);
    setChecklistError(null);
    try {
      setChecklist(setAcceptanceStatus(item.id, status));
    } catch (cause) {
      setChecklistError(String(cause));
    } finally {
      setSavingId(null);
    }
  };

  const resetAll = () => {
    if (!window.confirm("Đặt lại toàn bộ 58 acceptance checks về Pending?")) return;
    try {
      setChecklist(resetAcceptanceChecklist());
      setChecklistError(null);
      setFilter("all");
    } catch (cause) {
      setChecklistError(String(cause));
    }
  };

  const visibleItems = useMemo(() => {
    if (!checklist) return [];
    return filter === "all"
      ? checklist.items
      : checklist.items.filter((item) => item.status === filter);
  }, [checklist, filter]);

  const groups = useMemo(() => {
    const grouped = new Map<string, AcceptanceItem[]>();
    for (const item of visibleItems) {
      const existing = grouped.get(item.category) ?? [];
      existing.push(item);
      grouped.set(item.category, existing);
    }
    return [...grouped.entries()];
  }, [visibleItems]);

  const gate = combinedGate(checklist, readiness, readinessError, checklistError);
  const runtimeBlocking = readiness?.checks.filter((check) => check.level === "blocking").length ?? 0;

  return (
    <div className={`release-checklist ${open ? "is-open" : ""}`}>
      {!open ? (
        <button
          type="button"
          className="release-checklist-trigger"
          onClick={show}
          title="Local acceptance và release gate"
          aria-label="Mở local acceptance checklist"
        >
          <span aria-hidden="true">✓</span>
          <span>Release</span>
        </button>
      ) : (
        <section className="release-checklist-panel" aria-label="Local release acceptance checklist">
          <header>
            <div>
              <p>LOCAL ACCEPTANCE</p>
              <h2>Release gate</h2>
            </div>
            <div className="release-checklist-actions">
              <button type="button" disabled={loading || Boolean(savingId)} onClick={() => void refresh()}>
                {loading ? "Đang tải…" : "Làm mới"}
              </button>
              <button type="button" onClick={close}>Đóng</button>
            </div>
          </header>

          <div className="release-gate-summary">
            <div>
              <span>Gate</span>
              <strong className={`acceptance-${gate}`}>{gateLabel(gate)}</strong>
            </div>
            <div><span>Passed</span><strong>{checklist?.passed ?? 0}/{checklist?.total ?? 58}</strong></div>
            <div><span>Failed</span><strong>{checklist?.failed ?? 0}</strong></div>
            <div><span>Blocked</span><strong>{checklist?.blocked ?? 0}</strong></div>
            <div><span>Runtime blockers</span><strong>{runtimeBlocking}</strong></div>
          </div>

          <p className="release-gate-note">
            Đây là tracking thủ công cho acceptance matrix trên máy của bạn. Nó không tự chạy microphone,
            Android, installer, Firewall hay Tailscale test. Gate chỉ Ready khi 58 mục bắt buộc đã Passed
            và runtime self-check hiện không có Blocking.
          </p>

          {checklistError ? (
            <p className="release-checklist-error">
              Không thể đọc/lưu checklist local: {checklistError}. Dữ liệu không được tự ghi đè.
            </p>
          ) : null}
          {readinessError ? (
            <p className="release-checklist-error">Không thể đọc runtime readiness: {readinessError}</p>
          ) : null}

          <div className="release-checklist-toolbar">
            <label>
              <span>Lọc</span>
              <select value={filter} onChange={(event) => setFilter(event.target.value as StatusFilter)}>
                <option value="all">Tất cả</option>
                <option value="pending">Pending</option>
                <option value="passed">Passed</option>
                <option value="failed">Failed</option>
                <option value="blocked">Blocked</option>
              </select>
            </label>
            <span>{visibleItems.length} mục đang hiển thị</span>
            <button type="button" className="release-reset-button" onClick={resetAll}>Reset checklist</button>
          </div>

          {groups.length > 0 ? groups.map(([category, items]) => (
            <section className="release-checklist-group" key={category}>
              <h3>{category}</h3>
              <ul>
                {items.map((item) => (
                  <li className="release-checklist-row" key={item.id}>
                    <span className="release-check-number">{item.number}</span>
                    <div className="release-check-copy">
                      <strong>{item.label}</strong>
                      <span>{item.required ? "Required" : "Optional"} · {formatUnix(item.updatedUnix)}</span>
                    </div>
                    <select
                      aria-label={`Trạng thái acceptance ${item.number}`}
                      className={`acceptance-status acceptance-${item.status}`}
                      value={item.status}
                      disabled={Boolean(savingId)}
                      onChange={(event) => updateStatus(item, event.target.value as AcceptanceStatus)}
                    >
                      <option value="pending">Pending</option>
                      <option value="passed">Passed</option>
                      <option value="failed">Failed</option>
                      <option value="blocked">Blocked</option>
                    </select>
                  </li>
                ))}
              </ul>
            </section>
          )) : (
            <p className="release-checklist-empty">Không có acceptance check phù hợp bộ lọc.</p>
          )}
        </section>
      )}
    </div>
  );
}
