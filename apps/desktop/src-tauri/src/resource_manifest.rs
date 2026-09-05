use serde::Serialize;

/// Compatibility id retained so existing frontend/resource APIs keep working.
/// The resource behind it is now Vietnamese Zipformer STT, not Whisper.
pub const WHISPER_RESOURCE_ID: &str = "whisper";
pub const WAKE_RESOURCE_ID: &str = "wake_word";
pub const WAKE_KEYWORDS_RESOURCE_ID: &str = "wake_keywords";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePackageKind {
    SingleFile,
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
    vec![
        whisper_manifest(),
        wake_manifest(),
        wake_keywords_manifest(),
    ]
}

pub fn manifest(resource_id: &str) -> Option<ResourceInstallManifest> {
    match resource_id {
        WHISPER_RESOURCE_ID => Some(whisper_manifest()),
        WAKE_RESOURCE_ID => Some(wake_manifest()),
        WAKE_KEYWORDS_RESOURCE_ID => Some(wake_keywords_manifest()),
        _ => None,
    }
}

pub fn whisper_manifest() -> ResourceInstallManifest {
    ResourceInstallManifest {
        id: WHISPER_RESOURCE_ID,
        version: "sherpa-onnx-zipformer-vi-30M-int8-2026-02-09",
        package_kind: ResourcePackageKind::TarBz2,
        // The existing installer intentionally supports verified single files
        // only. Keep automatic installation disabled until the multi-file
        // transaction can verify and atomically promote the complete bundle.
        installable: false,
        source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-zipformer-vi-30M-int8-2026-02-09.tar.bz2",
        source_page: "https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/zipformer-transducer-models.html",
        license: "SEE_UPSTREAM_MODEL_CARD",
        expected_bytes: 0,
        sha256: None,
        note: "Primary Vietnamese STT model. Install the upstream archive into the displayed model directory so encoder.int8.onnx, decoder.onnx, joiner.int8.onnx and tokens.txt are present. Whisper remains optional fallback only.",
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
