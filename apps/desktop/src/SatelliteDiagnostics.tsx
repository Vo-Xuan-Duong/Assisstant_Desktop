import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getRuntimeReadiness,
  getTtsSettings,
  setSatelliteDeviceRevoked,
  setSatelliteEnabled,
  setTtsVoice,
} from "./api";
import { setQuickAutoDismissHold } from "./quickLifecycle";
import type {
  ReadinessCheck,
  ReadinessLevel,
  RuntimeReadinessReport,
  SapiVoiceInfo,
  SatelliteDeviceSnapshot,
  TtsLanguage,
  TtsSettingsSnapshot,
} from "./types";
import "./satellite-diagnostics.css";

const PANEL_HOLD_SOURCE = "satellite-diagnostics";
type VoicePanelTab = "satellite" | "tts" | "system";

function formatUnix(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "Chưa có";
  return new Intl.DateTimeFormat("vi-VN", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value * 1000));
}

function readinessLabel(level: ReadinessLevel): string {
  switch (level) {
    case "ready":
      return "Ready";
    case "optional_missing":
      return "Optional";
    case "blocking":
      return "Blocking";
  }
}

function readinessRank(level: ReadinessLevel): number {
  switch (level) {
    case "blocking":
      return 0;
    case "optional_missing":
      return 1;
    case "ready":
      return 2;
  }
}

function SystemReadinessPanel({
  report,
  loading,
  error,
}: {
  report: RuntimeReadinessReport | null;
  loading: boolean;
  error: string | null;
}) {
  if (error) {
    return <p className="satellite-diagnostics-error">Không thể đọc self-check: {error}</p>;
  }
  if (!report) {
    return loading
      ? <p className="satellite-device-empty">Đang kiểm tra runtime…</p>
      : <p className="satellite-device-empty">Chưa có dữ liệu self-check.</p>;
  }

  const checks = [...report.checks].sort((left, right) => {
    const rank = readinessRank(left.level) - readinessRank(right.level);
    return rank !== 0 ? rank : left.label.localeCompare(right.label);
  });
  const ready = checks.filter((check) => check.level === "ready").length;
  const optional = checks.filter((check) => check.level === "optional_missing").length;
  const blocking = checks.filter((check) => check.level === "blocking").length;

  return (
    <div className="system-readiness-panel" role="tabpanel">
      <div className="system-readiness-summary">
        <div>
          <span>Overall</span>
          <strong className={`readiness-${report.overall}`}>{readinessLabel(report.overall)}</strong>
        </div>
        <div><span>Ready</span><strong>{ready}</strong></div>
        <div><span>Optional</span><strong>{optional}</strong></div>
        <div><span>Blocking</span><strong>{blocking}</strong></div>
      </div>

      <p className="system-readiness-note">
        Self-check này chỉ phản ánh trạng thái runtime có thể kiểm tra tự động. QR, microphone,
        Android, Tailscale qua mobile data và installer vẫn phải được xác nhận trên thiết bị thật.
      </p>

      <ul className="system-readiness-list">
        {checks.map((check: ReadinessCheck) => (
          <li className="system-readiness-row" key={check.id}>
            <span className={`system-readiness-status readiness-${check.level}`}>
              {readinessLabel(check.level)}
            </span>
            <div>
              <strong>{check.label}</strong>
              <p>{check.detail}</p>
              {check.path ? <code title={check.path}>{check.path}</code> : null}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

interface DeviceRowProps {
  device: SatelliteDeviceSnapshot;
  disabled: boolean;
  busy: boolean;
  onToggle: (device: SatelliteDeviceSnapshot) => void;
}

function DeviceRow({ device, disabled, busy, onToggle }: DeviceRowProps) {
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
        <button
          type="button"
          className={device.revoked ? "is-allow" : "is-revoke"}
          disabled={disabled}
          onClick={() => onToggle(device)}
        >
          {busy ? "Đang lưu…" : device.revoked ? "Cho phép lại" : "Thu hồi"}
        </button>
      </div>
    </li>
  );
}

function localeRank(voice: SapiVoiceInfo, language: TtsLanguage): number {
  const attribute = voice.language?.toUpperCase() ?? "";
  const expected = language === "vi" ? "42A" : "409";
  return attribute.includes(expected) ? 0 : 1;
}

function sortedVoices(voices: SapiVoiceInfo[], language: TtsLanguage): SapiVoiceInfo[] {
  return [...voices].sort((left, right) => {
    const localeDifference = localeRank(left, language) - localeRank(right, language);
    if (localeDifference !== 0) return localeDifference;
    return left.name.localeCompare(right.name);
  });
}

interface TtsVoiceSelectProps {
  language: TtsLanguage;
  label: string;
  selectedVoiceId?: string | null;
  voices: SapiVoiceInfo[];
  disabled: boolean;
  onChange: (language: TtsLanguage, voiceId: string | null) => void;
}

function TtsVoiceSelect({
  language,
  label,
  selectedVoiceId,
  voices,
  disabled,
  onChange,
}: TtsVoiceSelectProps) {
  const installed = voices.some((voice) => voice.id === selectedVoiceId);
  const ordered = useMemo(() => sortedVoices(voices, language), [voices, language]);

  return (
    <label className="tts-settings-row">
      <span>{label}</span>
      <select
        value={selectedVoiceId ?? ""}
        disabled={disabled}
        onChange={(event) => onChange(language, event.target.value || null)}
      >
        <option value="">Tự động theo locale</option>
        {selectedVoiceId && !installed ? (
          <option value={selectedVoiceId} disabled>Voice đã chọn không còn được cài</option>
        ) : null}
        {ordered.map((voice) => (
          <option value={voice.id} key={voice.id}>
            {voice.name}{voice.language ? ` · ${voice.language}` : ""}
          </option>
        ))}
      </select>
      {selectedVoiceId && !installed ? (
        <small>Voice đã lưu không còn tồn tại; runtime sẽ dùng fallback an toàn.</small>
      ) : null}
    </label>
  );
}

export default function SatelliteDiagnostics() {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<VoicePanelTab>("satellite");
  const [report, setReport] = useState<RuntimeReadinessReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [satelliteSaving, setSatelliteSaving] = useState<string | null>(null);
  const [satelliteMutationError, setSatelliteMutationError] = useState<string | null>(null);
  const [tts, setTts] = useState<TtsSettingsSnapshot | null>(null);
  const [ttsLoading, setTtsLoading] = useState(false);
  const [ttsSaving, setTtsSaving] = useState(false);
  const [ttsError, setTtsError] = useState<string | null>(null);

  const refreshSatellite = useCallback(async () => {
    setLoading(true);
    setError(null);
    setSatelliteMutationError(null);
    try {
      setReport(await getRuntimeReadiness());
    } catch (cause) {
      setReport(null);
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshTts = useCallback(async () => {
    setTtsLoading(true);
    setTtsError(null);
    try {
      setTts(await getTtsSettings());
    } catch (cause) {
      setTts(null);
      setTtsError(String(cause));
    } finally {
      setTtsLoading(false);
    }
  }, []);

  const refresh = useCallback(() => {
    void refreshSatellite();
    void refreshTts();
  }, [refreshSatellite, refreshTts]);

  const close = useCallback(() => {
    setOpen(false);
    setTab("satellite");
    setSatelliteMutationError(null);
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
    setSatelliteMutationError(null);
    refresh();
  };

  const updateTtsVoice = useCallback(async (language: TtsLanguage, voiceId: string | null) => {
    if (ttsSaving || ttsLoading) return;
    setTtsSaving(true);
    setTtsError(null);
    try {
      setTts(await setTtsVoice(language, voiceId));
    } catch (cause) {
      setTtsError(String(cause));
    } finally {
      setTtsSaving(false);
    }
  }, [ttsLoading, ttsSaving]);

  const updateSatelliteEnabled = useCallback(async (enabled: boolean) => {
    if (satelliteSaving || loading) return;
    setSatelliteSaving("listener");
    setSatelliteMutationError(null);
    try {
      await setSatelliteEnabled(enabled);
      await refreshSatellite();
    } catch (cause) {
      setSatelliteMutationError(String(cause));
    } finally {
      setSatelliteSaving(null);
    }
  }, [loading, refreshSatellite, satelliteSaving]);

  const updateDeviceTrust = useCallback(async (device: SatelliteDeviceSnapshot) => {
    if (satelliteSaving || loading) return;
    const operation = `device:${device.id}`;
    setSatelliteSaving(operation);
    setSatelliteMutationError(null);
    try {
      await setSatelliteDeviceRevoked(device.id, !device.revoked);
      await refreshSatellite();
    } catch (cause) {
      setSatelliteMutationError(String(cause));
    } finally {
      setSatelliteSaving(null);
    }
  }, [loading, refreshSatellite, satelliteSaving]);

  const satellite = report?.satellite;
  const effectivelyPaired = Boolean(satellite?.paired || satellite?.environment_token_override);
  const credentialLabel = satellite?.environment_token_override
    ? "environment-override"
    : satellite?.credential_storage;
  const refreshing = loading || ttsLoading;
  const ttsControlsDisabled = ttsSaving || ttsLoading;
  const satelliteControlsDisabled = Boolean(satelliteSaving || loading);
  const listenerLocked = Boolean(
    satellite?.environment_token_override || satellite?.remote_managed,
  );
  const listenerCanEnable = Boolean(satellite?.enabled || satellite?.paired);
  const listenerLabel = satellite?.environment_token_override
    ? "Managed (env)"
    : satellite?.remote_managed
      ? satellite.enabled ? "Enabled (managed)" : "Disabled (managed)"
      : satellite?.enabled ? "Enabled" : "Disabled";
  const panelTitle = tab === "satellite"
    ? "Trạng thái kết nối"
    : tab === "tts"
      ? "Giọng đọc Windows"
      : "Kiểm tra hệ thống";

  return (
    <div className={`satellite-diagnostics ${open ? "is-open" : ""}`}>
      {!open ? (
        <button
          type="button"
          className="satellite-diagnostics-trigger"
          onClick={show}
          title="Assistant controls và local validation"
          aria-label="Mở Assistant controls và local validation"
        >
          <span aria-hidden="true">⚙</span>
          <span>Control</span>
        </button>
      ) : (
        <section className="satellite-diagnostics-panel" aria-label="Assistant control panel">
          <header>
            <div>
              <p>ASSISTANT CONTROL</p>
              <h2>{panelTitle}</h2>
            </div>
            <div className="satellite-diagnostics-actions">
              <button type="button" disabled={refreshing || ttsSaving || Boolean(satelliteSaving)} onClick={refresh}>
                {refreshing ? "Đang tải…" : "Làm mới"}
              </button>
              <button type="button" onClick={close}>Đóng</button>
            </div>
          </header>

          <div className="satellite-diagnostics-tabs" role="tablist" aria-label="Assistant control sections">
            <button
              type="button"
              role="tab"
              aria-selected={tab === "satellite"}
              className={tab === "satellite" ? "is-active" : ""}
              onClick={() => setTab("satellite")}
            >
              Satellite
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === "tts"}
              className={tab === "tts" ? "is-active" : ""}
              onClick={() => setTab("tts")}
            >
              TTS
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === "system"}
              className={tab === "system" ? "is-active" : ""}
              onClick={() => setTab("system")}
            >
              System
            </button>
          </div>

          {tab === "satellite" ? (
            <>
              {error ? <p className="satellite-diagnostics-error">Không thể đọc diagnostics: {error}</p> : null}
              {satelliteMutationError ? (
                <p className="satellite-diagnostics-error">Không thể cập nhật Satellite: {satelliteMutationError}</p>
              ) : null}

              {satellite ? (
                <>
                  <div className="satellite-diagnostics-grid">
                    <div><span>Listener</span><strong>{listenerLabel}</strong></div>
                    <div><span>Pairing</span><strong>{effectivelyPaired ? (satellite.environment_token_override ? "Paired (env)" : "Paired") : "Unpaired"}</strong></div>
                    <div><span>Bind</span><code>{satellite.bind}</code></div>
                    <div><span>Credential</span><strong>{credentialLabel}</strong></div>
                    <div><span>Remote</span><strong>{satellite.remote_managed ? `Tailscale :${satellite.remote_port ?? "?"}` : "LAN/local"}</strong></div>
                    <div><span>Devices</span><strong>{satellite.trusted_devices} trusted · {satellite.revoked_devices} revoked</strong></div>
                  </div>

                  <div className="satellite-management-row">
                    <div>
                      <strong>Satellite listener</strong>
                      <span>
                        {listenerLocked
                          ? satellite.remote_managed
                            ? "Tailscale managed mode đang giữ quyền điều khiển listener."
                            : "Environment token override đang giữ quyền điều khiển listener."
                          : satellite.enabled
                            ? "Tắt listener nhưng giữ nguyên pairing hiện tại."
                            : satellite.paired
                              ? "Bật lại listener với pairing hiện tại."
                              : "Cần pair thiết bị bằng CLI trước khi bật listener."}
                      </span>
                    </div>
                    <button
                      type="button"
                      disabled={
                        satelliteControlsDisabled
                        || listenerLocked
                        || (!satellite.enabled && !listenerCanEnable)
                      }
                      onClick={() => void updateSatelliteEnabled(!satellite.enabled)}
                    >
                      {satelliteSaving === "listener"
                        ? "Đang lưu…"
                        : satellite.enabled
                          ? "Tắt listener"
                          : "Bật listener"}
                    </button>
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
                    <span>Cho phép/thu hồi theo device ID · pairing token vẫn chỉ quản trị bằng CLI</span>
                  </div>
                  {satellite.devices.length > 0 ? (
                    <ul className="satellite-device-list">
                      {satellite.devices.map((device) => {
                        const operation = `device:${device.id}`;
                        return (
                          <DeviceRow
                            key={device.id}
                            device={device}
                            disabled={satelliteControlsDisabled}
                            busy={satelliteSaving === operation}
                            onToggle={(selected) => void updateDeviceTrust(selected)}
                          />
                        );
                      })}
                    </ul>
                  ) : (
                    <p className="satellite-device-empty">Chưa có Android device nào được đăng ký.</p>
                  )}
                </>
              ) : loading ? (
                <p className="satellite-device-empty">Đang đọc trạng thái…</p>
              ) : null}
            </>
          ) : tab === "tts" ? (
            <div className="tts-settings-panel" role="tabpanel">
              {ttsError ? <p className="satellite-diagnostics-error">Không thể cập nhật TTS: {ttsError}</p> : null}
              {tts ? (
                <>
                  <div className="tts-settings-list">
                    <TtsVoiceSelect
                      language="vi"
                      label="Tiếng Việt"
                      selectedVoiceId={tts.vietnamese_voice_id}
                      voices={tts.voices}
                      disabled={ttsControlsDisabled}
                      onChange={(language, voiceId) => void updateTtsVoice(language, voiceId)}
                    />
                    <TtsVoiceSelect
                      language="en"
                      label="English"
                      selectedVoiceId={tts.english_voice_id}
                      voices={tts.voices}
                      disabled={ttsControlsDisabled}
                      onChange={(language, voiceId) => void updateTtsVoice(language, voiceId)}
                    />
                  </div>
                  <p className="tts-settings-note">
                    {tts.voices.length} SAPI voice đã cài · thay đổi được lưu vào cùng `tts.conf` của runtime và áp dụng từ câu nói tiếp theo.
                  </p>
                </>
              ) : ttsLoading ? (
                <p className="satellite-device-empty">Đang đọc Windows SAPI voices…</p>
              ) : null}
            </div>
          ) : (
            <SystemReadinessPanel report={report} loading={loading} error={error} />
          )}
        </section>
      )}
    </div>
  );
}
