import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getRuntimeReadiness, getTtsSettings, setTtsVoice } from "./api";
import { setQuickAutoDismissHold } from "./quickLifecycle";
import type {
  RuntimeReadinessReport,
  SapiVoiceInfo,
  SatelliteDeviceSnapshot,
  TtsLanguage,
  TtsSettingsSnapshot,
} from "./types";
import "./satellite-diagnostics.css";

const PANEL_HOLD_SOURCE = "satellite-diagnostics";
type VoicePanelTab = "satellite" | "tts";

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
  const [tts, setTts] = useState<TtsSettingsSnapshot | null>(null);
  const [ttsLoading, setTtsLoading] = useState(false);
  const [ttsSaving, setTtsSaving] = useState(false);
  const [ttsError, setTtsError] = useState<string | null>(null);

  const refreshSatellite = useCallback(async () => {
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

  const satellite = report?.satellite;
  const effectivelyPaired = Boolean(satellite?.paired || satellite?.environment_token_override);
  const credentialLabel = satellite?.environment_token_override
    ? "environment-override"
    : satellite?.credential_storage;
  const refreshing = loading || ttsLoading;
  const ttsControlsDisabled = ttsSaving || ttsLoading;

  return (
    <div className={`satellite-diagnostics ${open ? "is-open" : ""}`}>
      {!open ? (
        <button
          type="button"
          className="satellite-diagnostics-trigger"
          onClick={show}
          title="Voice, TTS và Android Satellite"
          aria-label="Mở cài đặt Voice và Android Satellite"
        >
          <span aria-hidden="true">🔊</span>
          <span>Voice</span>
        </button>
      ) : (
        <section className="satellite-diagnostics-panel" aria-label="Voice and Satellite settings">
          <header>
            <div>
              <p>VOICE &amp; SATELLITE</p>
              <h2>{tab === "satellite" ? "Trạng thái kết nối" : "Giọng đọc Windows"}</h2>
            </div>
            <div className="satellite-diagnostics-actions">
              <button type="button" disabled={refreshing || ttsSaving} onClick={refresh}>
                {refreshing ? "Đang tải…" : "Làm mới"}
              </button>
              <button type="button" onClick={close}>Đóng</button>
            </div>
          </header>

          <div className="satellite-diagnostics-tabs" role="tablist" aria-label="Voice settings sections">
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
          </div>

          {tab === "satellite" ? (
            <>
              {error ? <p className="satellite-diagnostics-error">Không thể đọc diagnostics: {error}</p> : null}

              {satellite ? (
                <>
                  <div className="satellite-diagnostics-grid">
                    <div><span>Listener</span><strong>{satellite.enabled ? "Enabled" : "Disabled"}</strong></div>
                    <div><span>Pairing</span><strong>{effectivelyPaired ? (satellite.environment_token_override ? "Paired (env)" : "Paired") : "Unpaired"}</strong></div>
                    <div><span>Bind</span><code>{satellite.bind}</code></div>
                    <div><span>Credential</span><strong>{credentialLabel}</strong></div>
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
            </>
          ) : (
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
          )}
        </section>
      )}
    </div>
  );
}
