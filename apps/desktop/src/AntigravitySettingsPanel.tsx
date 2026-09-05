import { useCallback, useEffect, useState } from "react";
import {
  getAntigravitySettings,
  launchAntigravityAuth,
  saveAntigravitySettings,
} from "./api";
import type { AntigravitySettingsView } from "./types";
import "./antigravitySettings.css";

const CUSTOM_MODEL_VALUE = "__custom__";

interface AntigravitySettingsPanelProps {
  open?: boolean;
  onClose?: () => void;
  showTrigger?: boolean;
  onSettingsSaved?: () => void | Promise<void>;
}

export default function AntigravitySettingsPanel({
  open: controlledOpen,
  onClose,
  showTrigger = false,
  onSettingsSaved,
}: AntigravitySettingsPanelProps) {
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;

  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [launchingAuth, setLaunchingAuth] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [view, setView] = useState<AntigravitySettingsView | null>(null);

  const [selectedModel, setSelectedModel] = useState("default");
  const [customModel, setCustomModel] = useState("");
  const [selectedEffort, setSelectedEffort] = useState("default");

  const handleClose = useCallback(() => {
    if (isControlled) {
      onClose?.();
    } else {
      setInternalOpen(false);
    }
  }, [isControlled, onClose]);

  const applyViewToState = useCallback((data: AntigravitySettingsView) => {
    setView(data);

    const currentM = data.current_model?.trim();
    if (!currentM || currentM === "default") {
      setSelectedModel("default");
      setCustomModel("");
    } else {
      const known = data.available_models.some((m) => m.id === currentM);
      if (known) {
        setSelectedModel(currentM);
        setCustomModel("");
      } else {
        setSelectedModel(CUSTOM_MODEL_VALUE);
        setCustomModel(currentM);
      }
    }

    const currentE = data.current_effort?.trim();
    if (!currentE || currentE === "default") {
      setSelectedEffort("default");
    } else {
      setSelectedEffort(currentE);
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getAntigravitySettings();
      applyViewToState(data);
    } catch (cause) {
      setError(`Không thể tải cấu hình Antigravity: ${String(cause)}`);
    } finally {
      setLoading(false);
    }
  }, [applyViewToState]);

  useEffect(() => {
    if (open) {
      setSuccess(null);
      void refresh();
    }
  }, [open, refresh]);

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

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      let modelPayload: string | null = null;
      if (selectedModel === CUSTOM_MODEL_VALUE) {
        const trimmed = customModel.trim();
        modelPayload = trimmed.length > 0 ? trimmed : null;
      } else if (selectedModel !== "default") {
        modelPayload = selectedModel;
      }

      const effortPayload = selectedEffort !== "default" ? selectedEffort : null;

      const updated = await saveAntigravitySettings({
        model: modelPayload,
        effort: effortPayload,
      });

      applyViewToState(updated);
      setSuccess("Đã lưu cấu hình AI thành công! Session hội thoại đã được reset với model mới.");
      await onSettingsSaved?.();
    } catch (cause) {
      setError(`Không thể lưu cấu hình: ${String(cause)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleLaunchAuth = async () => {
    setLaunchingAuth(true);
    setError(null);
    try {
      await launchAntigravityAuth();
      setSuccess(
        "Đã mở cửa sổ đăng nhập Antigravity CLI. Hãy thực hiện xác thực trong cửa sổ dòng lệnh vừa mở, sau đó bấm 'Làm mới' để cập nhật trạng thái.",
      );
    } catch (cause) {
      setError(`Không thể mở cửa sổ đăng nhập: ${String(cause)}`);
    } finally {
      setLaunchingAuth(false);
    }
  };

  return (
    <>
      {showTrigger && (
        <button
          type="button"
          className="antigravity-btn"
          onClick={() => {
            if (isControlled) {
              if (open) onClose?.();
            } else {
              setInternalOpen(true);
            }
          }}
        >
          Cài đặt AI
        </button>
      )}

      {open && (
        <div
          className="antigravity-settings-backdrop"
          onClick={handleClose}
          role="presentation"
        >
          <section
            className="antigravity-settings-panel"
            aria-label="Cài đặt Antigravity"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
          >
            <header className="antigravity-settings-header">
              <div>
                <span className="section-label">CẤU HÌNH AI & TÀI KHOẢN</span>
                <h2>Antigravity Settings</h2>
              </div>
              <button
                type="button"
                className="antigravity-settings-close"
                onClick={handleClose}
                aria-label="Đóng"
              >
                ×
              </button>
            </header>

            {/* Tài khoản & Kết nối */}
            <div className="antigravity-settings-section">
              <div className="antigravity-section-title">
                <span>Tài khoản & Kết nối CLI</span>
                <span
                  className={`antigravity-status-badge ${
                    view?.is_authenticated ? "connected" : "disconnected"
                  }`}
                >
                  {view?.is_authenticated ? "● Đã kết nối" : "○ Chưa kết nối"}
                </span>
              </div>

              <div className="antigravity-auth-info">
                <span>Đường dẫn CLI thực thi:</span>
                <div className="antigravity-binary-path">
                  {view?.cli_binary || "Đang kiểm tra..."}
                </div>
              </div>

              <div className="antigravity-auth-action">
                <small style={{ color: "#8e9bb0" }}>
                  Xác thực tài khoản Google với Antigravity CLI qua trình duyệt web.
                </small>
                <button
                  type="button"
                  className="antigravity-auth-btn"
                  disabled={launchingAuth || loading}
                  onClick={() => void handleLaunchAuth()}
                >
                  {launchingAuth ? "Đang mở..." : "Đăng nhập / Đổi tài khoản"}
                </button>
              </div>
            </div>

            {/* Model & Reasoning */}
            <div className="antigravity-settings-section">
              <div className="antigravity-section-title">
                <span>Model AI & Khả năng suy luận (Reasoning)</span>
              </div>

              <div className="antigravity-field">
                <label htmlFor="ai-model-select">Mô hình AI (Model)</label>
                <select
                  id="ai-model-select"
                  className="antigravity-select"
                  value={selectedModel}
                  disabled={loading || saving}
                  onChange={(e) => setSelectedModel(e.target.value)}
                >
                  <option value="default">
                    Mặc định của CLI (Antigravity Default)
                  </option>
                  {view?.available_models.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label} ({m.id})
                    </option>
                  ))}
                  <option value={CUSTOM_MODEL_VALUE}>
                    Tùy chỉnh (Nhập tên model khác)...
                  </option>
                </select>
                <small>
                  Chọn mô hình AI chính dùng cho hội thoại và thực thi công cụ.
                </small>
              </div>

              {selectedModel === CUSTOM_MODEL_VALUE && (
                <div className="antigravity-field">
                  <label htmlFor="custom-model-input">Tên Model tùy chỉnh</label>
                  <input
                    id="custom-model-input"
                    type="text"
                    className="antigravity-input"
                    placeholder="ví dụ: gemini-3.7-flash hoặc claude-sonnet-4-6"
                    value={customModel}
                    disabled={loading || saving}
                    onChange={(e) => setCustomModel(e.target.value)}
                  />
                  <small>
                    Nhập chính xác ID model được CLI Antigravity hỗ trợ.
                  </small>
                </div>
              )}

              <div className="antigravity-field">
                <label htmlFor="ai-effort-select">Mức độ suy luận (Reasoning Effort)</label>
                <select
                  id="ai-effort-select"
                  className="antigravity-select"
                  value={selectedEffort}
                  disabled={loading || saving}
                  onChange={(e) => setSelectedEffort(e.target.value)}
                >
                  <option value="default">Mặc định (Default)</option>
                  <option value="low">Thấp (Low - phản hồi nhanh)</option>
                  <option value="medium">Trung bình (Medium)</option>
                  <option value="high">Cao (High - suy nghĩ kỹ)</option>
                </select>
                <small>
                  Kiểm soát lượng token dùng cho bước suy nghĩ (thinking tokens) của model.
                </small>
              </div>
            </div>

            {error && (
              <div className="antigravity-feedback error">
                {error}
              </div>
            )}

            {success && (
              <div className="antigravity-feedback success">
                {success}
              </div>
            )}

            <footer className="antigravity-settings-footer">
              <button
                type="button"
                className="antigravity-btn"
                disabled={loading || saving}
                onClick={() => void refresh()}
              >
                {loading ? "Đang tải..." : "Làm mới"}
              </button>

              <div className="antigravity-footer-actions">
                <button
                  type="button"
                  className="antigravity-btn"
                  onClick={handleClose}
                >
                  Đóng
                </button>
                <button
                  type="button"
                  className="antigravity-btn primary"
                  disabled={loading || saving}
                  onClick={() => void handleSave()}
                >
                  {saving ? "Đang lưu..." : "Lưu cấu hình"}
                </button>
              </div>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
