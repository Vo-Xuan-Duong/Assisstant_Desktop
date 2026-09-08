import { useCallback, useEffect, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getRuntimeReadiness } from "./api";
import { setQuickAutoDismissHold } from "./quickLifecycle";
import type { RuntimeReadinessReport, SatelliteDeviceSnapshot } from "./types";
import "./satellite-diagnostics.css";

const QUICK_RESIZE_EVENT = "quick:resize_request";
const PANEL_HEIGHT = 380;
const PANEL_HOLD_SOURCE = "satellite-diagnostics";

function formatUnix(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "Chưa có";
  return new Intl.DateTimeFormat("vi-VN", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value * 1000));
}

function DeviceRow({ device }: { device: SatelliteDeviceSnapshot }) {
  return (
    <li className="satellite-device-row">
      <div>
        <strong>{device.name || "Android device"}</strong>
        <code>{device.id}</code>
      </div>
      <div className="satellite-device-meta">
        <span className={device.revoked ? "is-revoked" : "is-trusted"}>
          {device.revoked ? "Revoked" : "Trusted"}
        </span>
        <span>Last seen {formatUnix(device.last_seen_unix)}</span>
      </div>
    </li>
  );
}

export default function SatelliteDiagnostics() {
  const [open, setOpen] = useState(false);
  const [report, setReport] = useState<RuntimeReadinessReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setReport(await getRuntimeReadiness());
    } catch (cause) {
      setReport(null);
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

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
    void emit(QUICK_RESIZE_EVENT, { height: PANEL_HEIGHT });
    void refresh();
  };

  const satellite = report?.satellite;

  return (
    <div className={`satellite-diagnostics ${open ? "is-open" : ""}`}>
      {!open ? (
        <button
          type="button"
          className="satellite-diagnostics-trigger"
          onClick={show}
          title="Android Voice Satellite"
          aria-label="Mở trạng thái Android Voice Satellite"
        >
          <span aria-hidden="true">📱</span>
          <span>Satellite</span>
        </button>
      ) : (
        <section className="satellite-diagnostics-panel" aria-label="Android Voice Satellite diagnostics">
          <header>
            <div>
              <p>ANDROID VOICE SATELLITE</p>
              <h2>Trạng thái kết nối</h2>
            </div>
            <div className="satellite-diagnostics-actions">
              <button type="button" disabled={loading} onClick={() => void refresh()}>
                {loading ? "Đang tải…" : "Làm mới"}
              </button>
              <button type="button" onClick={close}>Đóng</button>
            </div>
          </header>

          {error ? <p className="satellite-diagnostics-error">Không thể đọc diagnostics: {error}</p> : null}

          {satellite ? (
            <>
              <div className="satellite-diagnostics-grid">
                <div><span>Listener</span><strong>{satellite.enabled ? "Enabled" : "Disabled"}</strong></div>
                <div><span>Pairing</span><strong>{satellite.paired ? "Paired" : "Unpaired"}</strong></div>
                <div><span>Bind</span><code>{satellite.bind}</code></div>
                <div><span>Credential</span><strong>{satellite.credential_storage}</strong></div>
                <div><span>Remote</span><strong>{satellite.remote_managed ? `Tailscale :${satellite.remote_port ?? "?"}` : "LAN/local"}</strong></div>
                <div><span>Devices</span><strong>{satellite.trusted_devices} trusted · {satellite.revoked_devices} revoked</strong></div>
              </div>

              {satellite.environment_token_override ? (
                <p className="satellite-diagnostics-warning">
                  Environment token override đang bật; giá trị secret không được gửi tới UI.
                </p>
              ) : null}

              {satellite.warnings.map((warning) => (
                <p className="satellite-diagnostics-warning" key={warning}>{warning}</p>
              ))}

              <div className="satellite-device-list-heading">
                <strong>Thiết bị đã biết</strong>
                <span>Read-only · quản trị bằng `assistant satellite ...`</span>
              </div>
              {satellite.devices.length > 0 ? (
                <ul className="satellite-device-list">
                  {satellite.devices.map((device) => <DeviceRow key={device.id} device={device} />)}
                </ul>
              ) : (
                <p className="satellite-device-empty">Chưa có Android device nào được đăng ký.</p>
              )}
            </>
          ) : loading ? (
            <p className="satellite-device-empty">Đang đọc trạng thái…</p>
          ) : null}
        </section>
      )}
    </div>
  );
}
