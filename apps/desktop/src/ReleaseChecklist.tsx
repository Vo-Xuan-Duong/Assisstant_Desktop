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
import {
  buildReleaseEvidence,
  releaseEvidenceFilename,
  serializeReleaseEvidence,
} from "./releaseEvidence";
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
  const [evidenceError, setEvidenceError] = useState<string | null>(null);
  const [evidenceNotice, setEvidenceNotice] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [filter, setFilter] = useState<StatusFilter>("all");

  const gate = combinedGate(checklist, readiness, readinessError, checklistError);
  const runtimeBlocking = readiness?.checks.filter((check) => check.level === "blocking").length ?? 0;
  const canExportEvidence = Boolean(checklist && readiness && !checklistError && !readinessError);

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
    setEvidenceError(null);
    setEvidenceNotice(null);
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
    setEvidenceError(null);
    setEvidenceNotice(null);
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
    setEvidenceError(null);
    setEvidenceNotice(null);
    try {
      setChecklist(resetAcceptanceChecklist());
      setChecklistError(null);
      setFilter("all");
    } catch (cause) {
      setChecklistError(String(cause));
    }
  };

  const exportEvidence = () => {
    setEvidenceError(null);
    setEvidenceNotice(null);
    if (!checklist || !readiness || checklistError || readinessError) {
      setEvidenceError("Không thể xuất evidence khi checklist hoặc runtime readiness chưa hợp lệ.");
      return;
    }

    try {
      const evidence = buildReleaseEvidence(checklist, readiness, gate);
      const filename = releaseEvidenceFilename(evidence.generatedAtUnix);
      const blob = new Blob([serializeReleaseEvidence(evidence)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = filename;
      anchor.style.display = "none";
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
      setEvidenceNotice(`Đã xuất ${filename} · Gate ${gateLabel(gate)}.`);
    } catch (cause) {
      setEvidenceError(String(cause));
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
          <p className="release-evidence-note">
            Evidence JSON chỉ xuất trạng thái acceptance/readiness. Pairing credential, device identifier/name,
            runtime path và check detail được loại khỏi file để giảm rủi ro khi chia sẻ report.
          </p>

          {checklistError ? (
            <p className="release-checklist-error">
              Không thể đọc/lưu checklist local: {checklistError}. Dữ liệu không được tự ghi đè.
            </p>
          ) : null}
          {readinessError ? (
            <p className="release-checklist-error">Không thể đọc runtime readiness: {readinessError}</p>
          ) : null}
          {evidenceError ? (
            <p className="release-checklist-error">Không thể xuất release evidence: {evidenceError}</p>
          ) : null}
          {evidenceNotice ? <p className="release-evidence-success">{evidenceNotice}</p> : null}

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
            <button
              type="button"
              disabled={!canExportEvidence || loading || Boolean(savingId)}
              onClick={exportEvidence}
            >
              Xuất evidence JSON
            </button>
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
