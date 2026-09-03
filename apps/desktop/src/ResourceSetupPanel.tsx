import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getResourceCatalog,
  getRuntimeResources,
  installResource,
  onResourceInstallProgress,
} from "./api";
import type {
  ResourceInstallManifest,
  ResourceInstallProgress,
  ResourceState,
  RuntimeResourceSnapshot,
} from "./types";
import "./resourceSetup.css";

const stateLabel: Record<ResourceState, string> = {
  ready: "Ready",
  missing: "Missing",
  incomplete: "Incomplete",
  not_compiled: "Not compiled",
};

function formatBytes(value: number): string {
  if (value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let amount = value;
  let index = 0;
  while (amount >= 1024 && index < units.length - 1) {
    amount /= 1024;
    index += 1;
  }
  return `${amount.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

interface ResourceSetupPanelProps {
  onResourcesChanged?: () => void | Promise<void>;
}

export default function ResourceSetupPanel({ onResourcesChanged }: ResourceSetupPanelProps) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [snapshot, setSnapshot] = useState<RuntimeResourceSnapshot | null>(null);
  const [catalog, setCatalog] = useState<ResourceInstallManifest[]>([]);
  const [progress, setProgress] = useState<Record<string, ResourceInstallProgress>>({});
  const [installing, setInstalling] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [resources, manifests] = await Promise.all([
        getRuntimeResources(),
        getResourceCatalog(),
      ]);
      setSnapshot(resources);
      setCatalog(manifests);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void onResourceInstallProgress((event) => {
      setProgress((current) => ({ ...current, [event.resource_id]: event }));
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const toggle = useCallback(() => {
    setOpen((current) => {
      const next = !current;
      if (next && !snapshot && !loading) void refresh();
      return next;
    });
  }, [loading, refresh, snapshot]);

  const manifestById = useMemo(
    () => new Map(catalog.map((manifest) => [manifest.id, manifest])),
    [catalog],
  );

  const needsSetup = useMemo(
    () => snapshot?.resources.some((resource) => ["missing", "incomplete"].includes(resource.state)) ?? false,
    [snapshot],
  );

  const startInstall = useCallback(
    async (resourceId: string) => {
      if (installing[resourceId]) return;
      setInstalling((current) => ({ ...current, [resourceId]: true }));
      setError(null);
      try {
        await installResource(resourceId);
        await refresh();
        await onResourcesChanged?.();
      } catch (cause) {
        setError(`Không thể cài ${resourceId}: ${String(cause)}`);
      } finally {
        setInstalling((current) => ({ ...current, [resourceId]: false }));
      }
    },
    [installing, onResourcesChanged, refresh],
  );

  return (
    <div className="resource-root">
      <button
        type="button"
        className={`resource-trigger ${needsSetup ? "resource-trigger-warning" : ""}`}
        onClick={toggle}
        aria-expanded={open}
      >
        Resources
      </button>

      {open && (
        <section className="resource-panel" aria-label="Runtime resource setup">
          <div className="resource-header">
            <div>
              <span className="section-label">VERIFIED LOCAL RESOURCES</span>
              <strong>{needsSetup ? "Cần bổ sung local resource" : "Resource registry"}</strong>
            </div>
            <button type="button" className="resource-refresh" disabled={loading} onClick={() => void refresh()}>
              {loading ? "Đang kiểm tra" : "Kiểm tra lại"}
            </button>
          </div>

          <p className="resource-note">
            Chỉ resource có manifest nguồn/version/kích thước/checksum đã khóa mới được cài tự động. Wake word hiện vẫn yêu cầu cài thủ công.
          </p>

          {error && <p className="resource-error">{error}</p>}

          <div className="resource-list">
            {snapshot?.resources.map((resource) => {
              const manifest = manifestById.get(resource.id);
              const currentProgress = progress[resource.id];
              const isInstalling = Boolean(installing[resource.id]);
              const allFilesPresent = resource.files.length > 0 && resource.files.every((file) => file.exists);
              const percent = currentProgress?.total_bytes
                ? Math.min(100, Math.round((currentProgress.downloaded_bytes / currentProgress.total_bytes) * 100))
                : 0;
              const canInstall = Boolean(
                manifest?.installable &&
                  !allFilesPresent &&
                  !isInstalling,
              );

              return (
                <article key={resource.id} className={`resource-card resource-${resource.state}`}>
                  <div className="resource-title">
                    <strong>{resource.label}</strong>
                    <span>{stateLabel[resource.state]}</span>
                  </div>
                  <p>{resource.detail}</p>
                  <code className="resource-root-path">{resource.root_path}</code>

                  {manifest && (
                    <div className="resource-manifest">
                      <span>Version: {manifest.version}</span>
                      <span>Size: {formatBytes(manifest.expected_bytes)}</span>
                      <span>License: {manifest.license}</span>
                      <code>{manifest.source_page}</code>
                      <p>{manifest.note}</p>
                    </div>
                  )}

                  {currentProgress && (isInstalling || currentProgress.stage === "failed") && (
                    <div className="resource-progress" aria-live="polite">
                      <div className="resource-progress-bar" aria-hidden="true">
                        <span style={{ transform: `scaleX(${Math.max(0, percent / 100)})` }} />
                      </div>
                      <span>{currentProgress.stage} · {percent}%</span>
                      <small>{currentProgress.message}</small>
                    </div>
                  )}

                  <div className="resource-files">
                    {resource.files.map((file) => (
                      <div key={`${resource.id}-${file.name}`} className={file.exists ? "resource-file-ready" : "resource-file-missing"}>
                        <span>{file.exists ? "✓" : "×"} {file.name}</span>
                        <code>{file.path}</code>
                      </div>
                    ))}
                  </div>

                  {manifest && (
                    <div className="resource-install-actions">
                      {manifest.installable ? (
                        <button
                          type="button"
                          className="resource-install"
                          disabled={!canInstall}
                          onClick={() => void startInstall(resource.id)}
                        >
                          {allFilesPresent
                            ? "File đã có"
                            : isInstalling
                              ? `Đang cài ${percent}%`
                              : "Tải và xác minh"}
                        </button>
                      ) : (
                        <span className="resource-manual">Manual install required</span>
                      )}
                    </div>
                  )}
                </article>
              );
            })}
            {!snapshot && !error && <p>{loading ? "Đang đọc resource registry..." : "Chưa có dữ liệu."}</p>}
          </div>
        </section>
      )}
    </div>
  );
}
