export type AcceptanceStatus = "pending" | "passed" | "failed" | "blocked";
export type AcceptanceGate = "pending" | "blocked" | "ready";

export interface AcceptanceCheck {
  id: string;
  number: number;
  category: string;
  label: string;
  required: boolean;
}

export interface AcceptanceItem extends AcceptanceCheck {
  status: AcceptanceStatus;
  updatedUnix: number;
}

export interface AcceptanceSnapshot {
  gate: AcceptanceGate;
  total: number;
  requiredTotal: number;
  requiredPassed: number;
  passed: number;
  failed: number;
  blocked: number;
  pending: number;
  items: AcceptanceItem[];
}

interface StoredItem {
  status: AcceptanceStatus;
  updatedUnix: number;
}

interface StoredChecklist {
  schemaVersion: 1;
  items: Record<string, StoredItem>;
}

const STORAGE_KEY = "assistant.acceptance.v1";
const VALID_STATUSES = new Set<AcceptanceStatus>(["pending", "passed", "failed", "blocked"]);

const LABELS = [
  "QR pairing quét đúng trên điện thoại mục tiêu.",
  "Android chỉ kết nối sau thao tác rõ ràng của người dùng.",
  "Nhận dạng vi-VN và en-US đạt chất lượng chấp nhận được.",
  "Chỉ final transcript tạo đúng một Assistant turn.",
  "Permission desktop không thể bị Android bỏ qua.",
  "Revoke theo device ngắt/từ chối đúng điện thoại.",
  "Android Keystore giữ credential qua restart và migration.",
  "Reconnect không làm chạy lặp lại Windows action.",
  "Quick Settings activation và interrupt-to-talk hoạt động.",
  "Launcher shortcut Nói AI xuất hiện và forward đúng MainActivity/task.",
  "Launcher không bỏ qua pairing hoặc microphone permission.",
  "STT engine label khớp recognizer thực tế sau fallback.",
  "Confidence là optional và thiếu confidence được xử lý an toàn.",
  "Recognition alternatives chỉ hiển thị và không tạo thêm command.",
  "STT/desktop-turn timing hợp lý ở success, cancel và error.",
  "Conversation mode không nghe trong lúc desktop TTS đang phát.",
  "Follow-up recognition mở sau khi TTS hoàn tất.",
  "Follow-up giữ đúng Assistant conversation context.",
  "Silence/NO_MATCH kết thúc conversation thay vì retry vô hạn.",
  "Kết thúc hội thoại hủy pending microphone follow-up.",
  "DPAPI token persist/reload qua desktop restart với cùng Windows user.",
  "Legacy plaintext pairing credential migrate đúng thiết kế.",
  "Firewall helper chỉ tạo rule Private + LocalSubnet mong đợi.",
  "VI/EN/Auto tạo đúng ngôn ngữ response.",
  "SAPI voice VI/EN đã chọn được sử dụng trên máy mục tiêu.",
  "Preferred voice bị thiếu/xóa fallback an toàn.",
  "SAPI cancellation hoạt động.",
  "Satellite diagnostics phản ánh đúng listener/bind/credential/device state.",
  "Pairing token không xuất hiện trong frontend/devtools payload.",
  "Satellite state malformed chỉ cảnh báo, không crash Quick UI.",
  "Mở Control giữ Quick auto-dismiss trong lúc tương tác.",
  "Tailscale Serve map tailnet port vào 127.0.0.1.",
  "Android hoạt động qua mobile data trong tailnet cho phép.",
  "Device revoke vẫn chặn điện thoại remote.",
  "Remote disable restore previous local state khi an toàn.",
  "Tailscale Serve config không liên quan được giữ nguyên.",
  "Không tạo Funnel/public endpoint.",
  "Zipformer fallback và desktop wake vẫn hoạt động.",
  "Canonical/helper executables resolve đúng sau staging/install.",
  "NSIS package/startup flow hoạt động trên Windows mục tiêu.",
  "Control -> TTS liệt kê cùng installed voices như assistant tts voices.",
  "Chọn VI/EN trong Quick đồng bộ assistant tts show và response kế tiếp.",
  "Tự động theo locale xóa explicit preference và restore fallback.",
  "TTS settings malformed/stale báo lỗi mà không crash hay tự rewrite.",
  "Quick disable local listener nhưng giữ pairing.",
  "Quick re-enable listener không cần pair lại.",
  "Enable bị từ chối khi không có persisted pairing hợp lệ.",
  "Environment token override khóa/từ chối listener mutation.",
  "Tailscale managed state khóa/từ chối listener mutation.",
  "Revoke một connected device chỉ chặn thiết bị đó.",
  "Allow device restore access mà không rotate shared token.",
  "WebView không có pairing token/bind/firewall/Tailscale lifecycle authority.",
  "Control -> System render mọi readiness check dù optional component thiếu.",
  "Blocking sort trước Optional và Ready.",
  "Ready/Optional/Blocking summary khớp các row hiển thị.",
  "Làm mới phản ánh thay đổi local configuration thật.",
  "Path hiển thị an toàn và không có generic filesystem read capability.",
  "System payload/UI không lộ secret và không tự nhận là release certification.",
] as const;

function categoryFor(number: number): string {
  if (number <= 20) return "Core / Android";
  if (number <= 31) return "Windows / Security / TTS";
  if (number <= 37) return "Remote / Tailscale";
  if (number <= 40) return "Fallback / Release";
  if (number <= 44) return "Phase 32A / TTS UI";
  if (number <= 52) return "Phase 32B / Satellite UI";
  return "Phase 33A / System";
}

export const ACCEPTANCE_CATALOG: AcceptanceCheck[] = LABELS.map((label, index) => {
  const number = index + 1;
  return {
    id: `acceptance-${String(number).padStart(2, "0")}`,
    number,
    category: categoryFor(number),
    label,
    required: true,
  };
});

function emptyStore(): StoredChecklist {
  return { schemaVersion: 1, items: {} };
}

function readStore(): StoredChecklist {
  const raw = window.localStorage.getItem(STORAGE_KEY);
  if (raw === null) return emptyStore();

  const parsed = JSON.parse(raw) as Partial<StoredChecklist>;
  if (parsed.schemaVersion !== 1 || !parsed.items || typeof parsed.items !== "object") {
    throw new Error("Dữ liệu acceptance local không đúng schema v1.");
  }

  for (const [id, item] of Object.entries(parsed.items)) {
    if (!item || typeof item !== "object") {
      throw new Error(`Acceptance item ${id} không hợp lệ.`);
    }
    const candidate = item as Partial<StoredItem>;
    if (!candidate.status || !VALID_STATUSES.has(candidate.status)) {
      throw new Error(`Acceptance status của ${id} không hợp lệ.`);
    }
    if (!Number.isFinite(candidate.updatedUnix) || (candidate.updatedUnix ?? -1) < 0) {
      throw new Error(`Acceptance timestamp của ${id} không hợp lệ.`);
    }
  }

  return parsed as StoredChecklist;
}

function writeStore(store: StoredChecklist): void {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
}

function buildSnapshot(store: StoredChecklist): AcceptanceSnapshot {
  let passed = 0;
  let failed = 0;
  let blocked = 0;
  let pending = 0;
  let requiredPassed = 0;
  let requiredTotal = 0;

  const items = ACCEPTANCE_CATALOG.map((check): AcceptanceItem => {
    const stored = store.items[check.id];
    const status = stored?.status ?? "pending";
    if (status === "passed") passed += 1;
    else if (status === "failed") failed += 1;
    else if (status === "blocked") blocked += 1;
    else pending += 1;

    if (check.required) {
      requiredTotal += 1;
      if (status === "passed") requiredPassed += 1;
    }

    return {
      ...check,
      status,
      updatedUnix: stored?.updatedUnix ?? 0,
    };
  });

  const gate: AcceptanceGate = requiredTotal > 0 && requiredPassed === requiredTotal
    ? "ready"
    : failed > 0 || blocked > 0
      ? "blocked"
      : "pending";

  return {
    gate,
    total: items.length,
    requiredTotal,
    requiredPassed,
    passed,
    failed,
    blocked,
    pending,
    items,
  };
}

export function loadAcceptanceChecklist(): AcceptanceSnapshot {
  return buildSnapshot(readStore());
}

export function setAcceptanceStatus(
  checkId: string,
  status: AcceptanceStatus,
): AcceptanceSnapshot {
  if (!VALID_STATUSES.has(status)) throw new Error(`Acceptance status không hợp lệ: ${status}`);
  if (!ACCEPTANCE_CATALOG.some((check) => check.id === checkId)) {
    throw new Error(`Acceptance check không tồn tại: ${checkId}`);
  }

  const store = readStore();
  if (status === "pending") {
    delete store.items[checkId];
  } else {
    store.items[checkId] = {
      status,
      updatedUnix: Math.floor(Date.now() / 1000),
    };
  }
  writeStore(store);
  return buildSnapshot(store);
}

export function resetAcceptanceChecklist(): AcceptanceSnapshot {
  window.localStorage.removeItem(STORAGE_KEY);
  return buildSnapshot(emptyStore());
}
