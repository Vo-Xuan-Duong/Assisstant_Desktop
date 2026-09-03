import { useCallback, useMemo, useState } from "react";
import { getRuntimeResources } from "./api";
import type { ResourceState, RuntimeResourceSnapshot } from "./types";
import "./resourceSetup.css";

const stateLabel: Record<ResourceState, string> = {
  ready: "Ready",
  missing: "Missing",
  incomplete: "Incomplete",
  not_compiled: "Not compiled",
};

export default function ResourceSetupPanel() {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [snapshot, setSnapshot] = useState<RuntimeResourceSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setSnapshot(await getRuntimeResources());
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  const toggle = useCallback(() => {
    setOpen((current) => {
      const next = !current;
      if (next && !snapshot && !loading) void refresh();
      return next;
    });
  }, [loading, refresh, snapshot]);

  const needsSetup = useMemo(
    () => snapshot?.resources.some((resource) => ["missing", "incomplete"].includes(resource.state)) ?? false,
    [snapshot],
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
              <span className="section-label">FIRST-RUN RESOURCES</span>
              <strong>{needsSetup ? "Cần bổ sung local resource" : "Resource registry"}</strong>
            </div>
            <button type="button" className="resource-refresh" disabled={loading} onClick={() => void refresh()}>
              {loading ? "Đang kiểm tra" : "Kiểm tra lại"}
            </button>
          </div>

          <p className="resource-note">
            Phase 13A chỉ kiểm tra và hướng dẫn vị trí file. Ứng dụng chưa tự tải model từ Internet.
          </p>

          {error && <p className="resource-error">Không thể đọc resource registry: {error}</p>}

          <div className="resource-list">
            {snapshot?.resources.map((resource) => (
              <article key={resource.id} className={`resource-card resource-${resource.state}`}>
                <div className="resource-title">
                  <strong>{resource.label}</strong>
                  <span>{stateLabel[resource.state]}</span>
                </div>
                <p>{resource.detail}</p>
                <code className="resource-root-path">{resource.root_path}</code>
                <div className="resource-files">
                  {resource.files.map((file) => (
                    <div key={`${resource.id}-${file.name}`} className={file.exists ? "resource-file-ready" : "resource-file-missing"}>
                      <span>{file.exists ? "✓" : "×"} {file.name}</span>
                      <code>{file.path}</code>
                    </div>
                  ))}
                </div>
              </article>
            ))}
            {!snapshot && !error && <p>{loading ? "Đang đọc resource registry..." : "Chưa có dữ liệu."}</p>}
          </div>
        </section>
      )}
    </div>
  );
}
