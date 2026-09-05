use serde::Serialize;

pub const STT_RESOURCE_ID: &str = "stt_zipformer_vi";
pub const WAKE_RESOURCE_ID: &str = "wake_word";
pub const WAKE_KEYWORDS_RESOURCE_ID: &str = "wake_keywords";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackageKind {
    SingleFile,
    MultiFile,
    TarBz2,
    Generated,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceInstallManifest {
    pub id: &'static str,
    pub version: &'static str,
    pub package_kind: ResourcePackageKind,
    pub installable: bool,
    pub source_url: &'static str,
    pub source_page: &'static str,
    pub license: &'static str,
    pub expected_bytes: u64,
    pub sha256: Option<&'static str>,
    pub note: &'static str,
}

pub fn manifests() -> Vec<ResourceInstallManifest> {
    vec![stt_manifest(), wake_manifest(), wake_keywords_manifest()]
}

pub fn manifest(resource_id: &str) -> Option<ResourceInstallManifest> {
    match resource_id {
        STT_RESOURCE_ID => Some(stt_manifest()),
        WAKE_RESOURCE_ID => Some(wake_manifest()),
        WAKE_KEYWORDS_RESOURCE_ID => Some(wake_keywords_manifest()),
        _ => None,
    }
}

pub fn stt_manifest() -> ResourceInstallManifest {
    ResourceInstallManifest {
        id: STT_RESOURCE_ID,
        version: "csukuangfj2/sherpa-onnx-zipformer-vi-30M-int8-2026-02-09@83e140d",
        package_kind: ResourcePackageKind::MultiFile,
        installable: true,
        source_url: "https://huggingface.co/csukuangfj2/sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/tree/83e140db6d23fbb8480fd5fb868f74ab80e7092c",
        source_page: "https://huggingface.co/hynt/Zipformer-30M-RNNT-6000h",
        license: "CC-BY-NC-ND-4.0",
        // Exact pinned bytes for encoder + decoder + joiner + bpe.model.
        // tokens.txt is a small commit-pinned text file validated structurally by the installer.
        expected_bytes: 34_165_670,
        sha256: None,
        note: "Primary Vietnamese CPU STT. The installer downloads an immutable five-file sherpa-onnx bundle, verifies SHA-256 for all LFS model assets, validates tokens.txt structure, then promotes the staging directory atomically. The upstream model license is non-commercial/no-derivatives; the model is downloaded at runtime and is not bundled with the application.",
    }
}

pub fn wake_manifest() -> ResourceInstallManifest {
    ResourceInstallManifest {
        id: WAKE_RESOURCE_ID,
        version: "sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01",
        package_kind: ResourcePackageKind::TarBz2,
        installable: false,
        source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2",
        source_page: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/kws-models",
        license: "UNRESOLVED_FOR_AUTO_INSTALL",
        expected_bytes: 17_626_723,
        sha256: None,
        note: "Automatic wake model installation remains disabled until archive SHA-256 and model redistribution terms are pinned.",
    }
}

pub fn wake_keywords_manifest() -> ResourceInstallManifest {
    ResourceInstallManifest {
        id: WAKE_KEYWORDS_RESOURCE_ID,
        version: "local-gigaspeech-bpe-v1",
        package_kind: ResourcePackageKind::Generated,
        installable: true,
        source_url: "",
        source_page: "https://k2-fsa.github.io/sherpa/onnx/kws/pretrained_models/index.html",
        license: "LOCAL_GENERATED_FILE",
        expected_bytes: 0,
        sha256: None,
        note: "Generated locally from the manually installed GigaSpeech bpe.model + tokens.txt. The backend validates every SentencePiece token against the runtime vocabulary and never downloads wake model data for this action.",
    }
}
