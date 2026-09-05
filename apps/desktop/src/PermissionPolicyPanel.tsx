import { useEffect, useMemo, useState } from "react";
import {
  onPermissionPolicy,
  onPermissionPolicyError,
  requestPermissionPolicy,
  setPermissionPolicy,
} from "./permissionApi";
import type {
  PermissionDecision,
  PermissionPolicyTool,
  PermissionPolicyView,
} from "./types";
import "./permissionPolicy.css";

const DEFAULT_VALUE = "default";
type PolicySelectValue = PermissionDecision | typeof DEFAULT_VALUE;

function effectiveDecision(tool: PermissionPolicyTool): PermissionDecision {
  return tool.override_decision ?? tool.default_decision;
}

interface PermissionPolicyPanelProps {
  open?: boolean;
  onClose?: () => void;
  showTrigger?: boolean;
}

export default function PermissionPolicyPanel({
  open: controlledOpen,
  onClose,
  showTrigger = true,
}: PermissionPolicyPanelProps = {}) {
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;

  const [view, setView] = useState<PermissionPolicyView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [savingTool, setSavingTool] = useState<string | null>(null);

  const handleClose = () => {
    if (isControlled) {
      onClose?.();
    } else {
      setInternalOpen(false);
    }
  };

  const handleToggle = () => {
    if (isControlled) {
      if (open) onClose?.();
    } else {
      setInternalOpen((curr) => !curr);
    }
  };

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open]);

  useEffect(() => {
    let disposed = false;
    let unlistenView: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    void onPermissionPolicy((next) => {
      setView(next);
      setError(next.load_error ?? null);
      setSavingTool(null);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenView = unlisten;
    });

    void onPermissionPolicyError((message) => {
      setError(message);
      setSavingTool(null);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenError = unlisten;
    });

    return () => {
      disposed = true;
      unlistenView?.();
      unlistenError?.();
    };
  }, []);

  useEffect(() => {
    if (open) void requestPermissionPolicy();
  }, [open]);

  const overriddenCount = useMemo(
    () => view?.tools.filter((tool) => tool.override_decision != null).length ?? 0,
    [view],
  );

  async function changePolicy(toolName: string, value: PolicySelectValue) {
    if (savingTool) return;
    setSavingTool(toolName);
    setError(null);
    try {
      await setPermissionPolicy(
        toolName,
        value === DEFAULT_VALUE ? null : (value as PermissionDecision),
      );
    } catch (cause) {
      setError(String(cause));
      setSavingTool(null);
    }
  }

  return (
    <>
      {showTrigger && (
        <button
          type="button"
          className="permission-policy-trigger"
          onClick={handleToggle}
          aria-expanded={open}
        >
          Permissions{overriddenCount ? ` · ${overriddenCount}` : ""}
        </button>
      )}

      {open && (
        <div className="permission-policy-backdrop" onClick={handleClose} role="presentation">
          <section
            className="permission-policy-panel"
            aria-label="Runtime permission policy"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
          >
            <header>
              <div>
                <span>RUNTIME POLICY</span>
                <strong>Moderate tools</strong>
              </div>
              <button
                type="button"
                className="permission-policy-close"
                onClick={handleClose}
                aria-label="Đóng"
              >
                ×
              </button>
            </header>

            <p className="permission-policy-note">
              Chỉ các tool Moderate có thể override. Sensitive vẫn luôn cần xác nhận; Blocked luôn bị từ chối.
            </p>

            {error && <p className="permission-policy-error">{error}</p>}

            <div className="permission-policy-list">
              {view?.tools.map((tool) => {
                const selected: PolicySelectValue = tool.override_decision ?? DEFAULT_VALUE;
                const effective = effectiveDecision(tool);
                return (
                  <label key={tool.name} className="permission-policy-row">
                    <div>
                      <strong>{tool.name}</strong>
                      <small>{tool.description}</small>
                      <span>Effective: {effective}</span>
                    </div>
                    <select
                      value={selected}
                      disabled={savingTool === tool.name}
                      onChange={(event) =>
                        void changePolicy(tool.name, event.target.value as PolicySelectValue)
                      }
                    >
                      <option value={DEFAULT_VALUE}>Default ({tool.default_decision})</option>
                      <option value="allow">Allow</option>
                      <option value="ask">Ask</option>
                      <option value="deny">Deny</option>
                    </select>
                  </label>
                );
              })}
              {!view && <p className="permission-policy-loading">Đang tải policy…</p>}
            </div>

            <footer>
              <span>Revision {view?.revision ?? 0}</span>
              <button type="button" onClick={() => void requestPermissionPolicy()}>
                Refresh
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
