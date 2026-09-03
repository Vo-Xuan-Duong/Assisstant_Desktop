use serde::Serialize;

pub const WHISPER_RESOURCE_ID: &str = "whisper";
pub const WAKE_RESOURCE_ID: &str = "wake_word";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackageKind {
    SingleFile,
    TarBz2,
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
    vec![whisper_manifest(), wake_manifest()]
}

pub fn manifest(resource_id: &str) -> Option<ResourceInstallManifest> {
    match resource_id {
        WHISPER_RESOURCE_ID => Some(whisper_manifest()),
        WAKE_RESOURCE_ID => Some(wake_manifest()),
        _ => None,
    }
}

pub fn whisper_manifest() -> ResourceInstallManifest {
    ResourceInstallManifest {
        id: WHISPER_RESOURCE_ID,
        version: "ggerganov-whisper.cpp@5359861c739e955e79d9a303bcbc70fb988958b1/base",
        package_kind: ResourcePackageKind::SingleFile,
        installable: true,
        source_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.bin?download=true",
        source_page: "https://huggingface.co/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1",
        license: "MIT",
        expected_bytes: 147_951_465,
        sha256: Some("60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe"),
        note: "Pinned multilingual Whisper base model. Automatic install is allowed only because source revision, byte size and SHA-256 are pinned.",
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
        note: "Automatic wake installation stays disabled until the exact archive SHA-256/model redistribution terms and application-specific keywords generation contract are pinned.",
    }
}
