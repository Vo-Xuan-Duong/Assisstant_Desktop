use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::DesktopState;

const ACCEPTANCE_FILE: &str = "acceptance.json";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
struct AcceptanceDefinition {
    id: &'static str,
    number: u16,
    category: &'static str,
    label: &'static str,
    required: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceStatus {
    Pending,
    Passed,
    Failed,
    Blocked,
}

impl Default for AcceptanceStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredAcceptanceItem {
    #[serde(default)]
    status: AcceptanceStatus,
    #[serde(default)]
    updated_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AcceptanceStore {
    schema_version: u32,
    #[serde(default)]
    items: BTreeMap<String, StoredAcceptanceItem>,
}

impl Default for AcceptanceStore {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            items: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcceptanceItemView {
    pub id: String,
    pub number: u16,
    pub category: String,
    pub label: String,
    pub required: bool,
    pub status: AcceptanceStatus,
    pub updated_unix: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceGate {
    Pending,
    Blocked,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcceptanceSnapshot {
    pub gate: AcceptanceGate,
    pub total: usize,
    pub required_total: usize,
    pub passed: usize,
    pub failed: usize,
    pub blocked: usize,
    pub pending: usize,
    pub required_passed: usize,
    pub items: Vec<AcceptanceItemView>,
}

#[tauri::command]
pub async fn assistant_acceptance_checklist(
    state: State<'_, DesktopState>,
) -> Result<AcceptanceSnapshot, String> {
    let app_data = state.runtime_paths.app_local_data.clone();
    tokio::task::spawn_blocking(move || snapshot(&acceptance_path(&app_data)))
        .await
        .map_err(|error| format!("acceptance checklist worker failed: {error}"))?
}

#[tauri::command]
pub async fn assistant_acceptance_set_status(
    check_id: String,
    status: AcceptanceStatus,
    state: State<'_, DesktopState>,
) -> Result<AcceptanceSnapshot, String> {
    let app_data = state.runtime_paths.app_local_data.clone();
    tokio::task::spawn_blocking(move || {
        let path = acceptance_path(&app_data);
        set_status(&path, &check_id, status)?;
        snapshot(&path)
    })
    .await
    .map_err(|error| format!("acceptance checklist worker failed: {error}"))?
}

#[tauri::command]
pub async fn assistant_acceptance_reset(
    state: State<'_, DesktopState>,
) -> Result<AcceptanceSnapshot, String> {
    let app_data = state.runtime_paths.app_local_data.clone();
    tokio::task::spawn_blocking(move || {
        let path = acceptance_path(&app_data);
        save_store(&path, &AcceptanceStore::default())?;
        snapshot(&path)
    })
    .await
    .map_err(|error| format!("acceptance checklist worker failed: {error}"))?
}

fn acceptance_path(app_data: &Path) -> PathBuf {
    app_data.join("settings").join(ACCEPTANCE_FILE)
}

fn snapshot(path: &Path) -> Result<AcceptanceSnapshot, String> {
    let store = load_store(path)?;
    let mut items = Vec::with_capacity(CATALOG.len());
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut blocked = 0usize;
    let mut pending = 0usize;
    let mut required_total = 0usize;
    let mut required_passed = 0usize;
    let mut required_has_failure = false;

    for definition in CATALOG {
        let stored = store.items.get(definition.id).cloned().unwrap_or_default();
        match stored.status {
            AcceptanceStatus::Passed => passed += 1,
            AcceptanceStatus::Failed => failed += 1,
            AcceptanceStatus::Blocked => blocked += 1,
            AcceptanceStatus::Pending => pending += 1,
        }
        if definition.required {
            required_total += 1;
            if stored.status == AcceptanceStatus::Passed {
                required_passed += 1;
            }
            if matches!(stored.status, AcceptanceStatus::Failed | AcceptanceStatus::Blocked) {
                required_has_failure = true;
            }
        }
        items.push(AcceptanceItemView {
            id: definition.id.to_owned(),
            number: definition.number,
            category: definition.category.to_owned(),
            label: definition.label.to_owned(),
            required: definition.required,
            status: stored.status,
            updated_unix: stored.updated_unix,
        });
    }

    let gate = if required_total > 0 && required_passed == required_total {
        AcceptanceGate::Ready
    } else if required_has_failure {
        AcceptanceGate::Blocked
    } else {
        AcceptanceGate::Pending
    };

    Ok(AcceptanceSnapshot {
        gate,
        total: items.len(),
        required_total,
        passed,
        failed,
        blocked,
        pending,
        required_passed,
        items,
    })
}

fn set_status(path: &Path, raw_id: &str, status: AcceptanceStatus) -> Result<(), String> {
    let check_id = raw_id.trim();
    if !CATALOG.iter().any(|definition| definition.id == check_id) {
        return Err(format!("unknown acceptance check `{check_id}`"));
    }

    let mut store = load_store(path)?;
    if status == AcceptanceStatus::Pending {
        store.items.remove(check_id);
    } else {
        store.items.insert(
            check_id.to_owned(),
            StoredAcceptanceItem {
                status,
                updated_unix: unix_now(),
            },
        );
    }
    save_store(path, &store)
}

fn load_store(path: &Path) -> Result<AcceptanceStore, String> {
    if !path.is_file() {
        return Ok(AcceptanceStore::default());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let store: AcceptanceStore = serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    if store.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported acceptance checklist schema {} in {}; expected {}",
            store.schema_version,
            path.display(),
            SCHEMA_VERSION
        ));
    }
    Ok(store)
}

fn save_store(path: &Path, store: &AcceptanceStore) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "acceptance checklist path has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|error| format!("cannot serialize acceptance checklist: {error}"))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    if path.is_file() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    }
    fs::rename(&temp, path)
        .map_err(|error| format!("cannot move {} to {}: {error}", temp.display(), path.display()))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

const CATALOG: &[AcceptanceDefinition] = &[
    AcceptanceDefinition { id: "acceptance-01", number: 1, category: "Core / Android", label: "QR pairing quét đúng trên điện thoại mục tiêu.", required: true },
    AcceptanceDefinition { id: "acceptance-02", number: 2, category: "Core / Android", label: "Android chỉ kết nối sau thao tác rõ ràng của người dùng.", required: true },
    AcceptanceDefinition { id: "acceptance-03", number: 3, category: "Core / Android", label: "Nhận dạng vi-VN và en-US đạt chất lượng chấp nhận được.", required: true },
    AcceptanceDefinition { id: "acceptance-04", number: 4, category: "Core / Android", label: "Chỉ final transcript tạo đúng một Assistant turn.", required: true },
    AcceptanceDefinition { id: "acceptance-05", number: 5, category: "Core / Android", label: "Permission desktop không thể bị Android bỏ qua.", required: true },
    AcceptanceDefinition { id: "acceptance-06", number: 6, category: "Core / Android", label: "Revoke theo device ngắt/từ chối đúng điện thoại.", required: true },
    AcceptanceDefinition { id: "acceptance-07", number: 7, category: "Core / Android", label: "Android Keystore giữ credential qua restart và migration.", required: true },
    AcceptanceDefinition { id: "acceptance-08", number: 8, category: "Core / Android", label: "Reconnect không làm chạy lặp lại Windows action.", required: true },
    AcceptanceDefinition { id: "acceptance-09", number: 9, category: "Core / Android", label: "Quick Settings activation và interrupt-to-talk hoạt động.", required: true },
    AcceptanceDefinition { id: "acceptance-10", number: 10, category: "Core / Android", label: "Launcher shortcut Nói AI xuất hiện và forward đúng MainActivity/task.", required: true },
    AcceptanceDefinition { id: "acceptance-11", number: 11, category: "Core / Android", label: "Launcher không bỏ qua pairing hoặc microphone permission.", required: true },
    AcceptanceDefinition { id: "acceptance-12", number: 12, category: "Core / Android", label: "STT engine label khớp recognizer thực tế sau fallback.", required: true },
    AcceptanceDefinition { id: "acceptance-13", number: 13, category: "Core / Android", label: "Confidence là optional và thiếu confidence được xử lý an toàn.", required: true },
    AcceptanceDefinition { id: "acceptance-14", number: 14, category: "Core / Android", label: "Recognition alternatives chỉ hiển thị và không tạo thêm command.", required: true },
    AcceptanceDefinition { id: "acceptance-15", number: 15, category: "Core / Android", label: "STT/desktop-turn timing hợp lý ở success, cancel và error.", required: true },
    AcceptanceDefinition { id: "acceptance-16", number: 16, category: "Core / Android", label: "Conversation mode không nghe trong lúc desktop TTS đang phát.", required: true },
    AcceptanceDefinition { id: "acceptance-17", number: 17, category: "Core / Android", label: "Follow-up recognition mở sau khi TTS hoàn tất.", required: true },
    AcceptanceDefinition { id: "acceptance-18", number: 18, category: "Core / Android", label: "Follow-up giữ đúng Assistant conversation context.", required: true },
    AcceptanceDefinition { id: "acceptance-19", number: 19, category: "Core / Android", label: "Silence/NO_MATCH kết thúc conversation thay vì retry vô hạn.", required: true },
    AcceptanceDefinition { id: "acceptance-20", number: 20, category: "Core / Android", label: "Kết thúc hội thoại hủy pending microphone follow-up.", required: true },

    AcceptanceDefinition { id: "acceptance-21", number: 21, category: "Windows / Security / TTS", label: "DPAPI token persist/reload qua desktop restart với cùng Windows user.", required: true },
    AcceptanceDefinition { id: "acceptance-22", number: 22, category: "Windows / Security / TTS", label: "Legacy plaintext pairing credential migrate đúng thiết kế.", required: true },
    AcceptanceDefinition { id: "acceptance-23", number: 23, category: "Windows / Security / TTS", label: "Firewall helper chỉ tạo rule Private + LocalSubnet mong đợi.", required: true },
    AcceptanceDefinition { id: "acceptance-24", number: 24, category: "Windows / Security / TTS", label: "VI/EN/Auto tạo đúng ngôn ngữ response.", required: true },
    AcceptanceDefinition { id: "acceptance-25", number: 25, category: "Windows / Security / TTS", label: "SAPI voice VI/EN đã chọn được sử dụng trên máy mục tiêu.", required: true },
    AcceptanceDefinition { id: "acceptance-26", number: 26, category: "Windows / Security / TTS", label: "Preferred voice bị thiếu/xóa fallback an toàn.", required: true },
    AcceptanceDefinition { id: "acceptance-27", number: 27, category: "Windows / Security / TTS", label: "SAPI cancellation hoạt động.", required: true },
    AcceptanceDefinition { id: "acceptance-28", number: 28, category: "Windows / Security / TTS", label: "Satellite diagnostics phản ánh đúng listener/bind/credential/device state.", required: true },
    AcceptanceDefinition { id: "acceptance-29", number: 29, category: "Windows / Security / TTS", label: "Pairing token không xuất hiện trong frontend/devtools payload.", required: true },
    AcceptanceDefinition { id: "acceptance-30", number: 30, category: "Windows / Security / TTS", label: "Satellite state malformed chỉ cảnh báo, không crash Quick UI.", required: true },
    AcceptanceDefinition { id: "acceptance-31", number: 31, category: "Windows / Security / TTS", label: "Mở Control giữ Quick auto-dismiss trong lúc tương tác.", required: true },

    AcceptanceDefinition { id: "acceptance-32", number: 32, category: "Remote / Tailscale", label: "Tailscale Serve map tailnet port vào 127.0.0.1.", required: true },
    AcceptanceDefinition { id: "acceptance-33", number: 33, category: "Remote / Tailscale", label: "Android hoạt động qua mobile data trong tailnet cho phép.", required: true },
    AcceptanceDefinition { id: "acceptance-34", number: 34, category: "Remote / Tailscale", label: "Device revoke vẫn chặn điện thoại remote.", required: true },
    AcceptanceDefinition { id: "acceptance-35", number: 35, category: "Remote / Tailscale", label: "Remote disable restore previous local state khi an toàn.", required: true },
    AcceptanceDefinition { id: "acceptance-36", number: 36, category: "Remote / Tailscale", label: "Tailscale Serve config không liên quan được giữ nguyên.", required: true },
    AcceptanceDefinition { id: "acceptance-37", number: 37, category: "Remote / Tailscale", label: "Không tạo Funnel/public endpoint.", required: true },

    AcceptanceDefinition { id: "acceptance-38", number: 38, category: "Fallback / Release", label: "Zipformer fallback và desktop wake vẫn hoạt động.", required: true },
    AcceptanceDefinition { id: "acceptance-39", number: 39, category: "Fallback / Release", label: "Canonical/helper executables resolve đúng sau staging/install.", required: true },
    AcceptanceDefinition { id: "acceptance-40", number: 40, category: "Fallback / Release", label: "NSIS package/startup flow hoạt động trên Windows mục tiêu.", required: true },

    AcceptanceDefinition { id: "acceptance-41", number: 41, category: "Phase 32A / TTS UI", label: "Control -> TTS liệt kê cùng installed voices như assistant tts voices.", required: true },
    AcceptanceDefinition { id: "acceptance-42", number: 42, category: "Phase 32A / TTS UI", label: "Chọn VI/EN trong Quick đồng bộ assistant tts show và response kế tiếp.", required: true },
    AcceptanceDefinition { id: "acceptance-43", number: 43, category: "Phase 32A / TTS UI", label: "Tự động theo locale xóa explicit preference và restore fallback.", required: true },
    AcceptanceDefinition { id: "acceptance-44", number: 44, category: "Phase 32A / TTS UI", label: "TTS settings malformed/stale báo lỗi mà không crash hay tự rewrite.", required: true },

    AcceptanceDefinition { id: "acceptance-45", number: 45, category: "Phase 32B / Satellite UI", label: "Quick disable local listener nhưng giữ pairing.", required: true },
    AcceptanceDefinition { id: "acceptance-46", number: 46, category: "Phase 32B / Satellite UI", label: "Quick re-enable listener không cần pair lại.", required: true },
    AcceptanceDefinition { id: "acceptance-47", number: 47, category: "Phase 32B / Satellite UI", label: "Enable bị từ chối khi không có persisted pairing hợp lệ.", required: true },
    AcceptanceDefinition { id: "acceptance-48", number: 48, category: "Phase 32B / Satellite UI", label: "Environment token override khóa/từ chối listener mutation.", required: true },
    AcceptanceDefinition { id: "acceptance-49", number: 49, category: "Phase 32B / Satellite UI", label: "Tailscale managed state khóa/từ chối listener mutation.", required: true },
    AcceptanceDefinition { id: "acceptance-50", number: 50, category: "Phase 32B / Satellite UI", label: "Revoke một connected device chỉ chặn thiết bị đó.", required: true },
    AcceptanceDefinition { id: "acceptance-51", number: 51, category: "Phase 32B / Satellite UI", label: "Allow device restore access mà không rotate shared token.", required: true },
    AcceptanceDefinition { id: "acceptance-52", number: 52, category: "Phase 32B / Satellite UI", label: "WebView không có pairing token/bind/firewall/Tailscale lifecycle authority.", required: true },

    AcceptanceDefinition { id: "acceptance-53", number: 53, category: "Phase 33A / System", label: "Control -> System render mọi readiness check dù optional component thiếu.", required: true },
    AcceptanceDefinition { id: "acceptance-54", number: 54, category: "Phase 33A / System", label: "Blocking sort trước Optional và Ready.", required: true },
    AcceptanceDefinition { id: "acceptance-55", number: 55, category: "Phase 33A / System", label: "Ready/Optional/Blocking summary khớp các row hiển thị.", required: true },
    AcceptanceDefinition { id: "acceptance-56", number: 56, category: "Phase 33A / System", label: "Làm mới phản ánh thay đổi local configuration thật.", required: true },
    AcceptanceDefinition { id: "acceptance-57", number: 57, category: "Phase 33A / System", label: "Path hiển thị an toàn và không có generic filesystem read capability.", required: true },
    AcceptanceDefinition { id: "acceptance-58", number: 58, category: "Phase 33A / System", label: "System payload/UI không lộ secret và không tự nhận là release certification.", required: true },
];
