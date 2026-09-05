use std::{
    env,
    path::{Path, PathBuf},
};

use serde::Serialize;
use voice_runtime::wake::SherpaWakeConfig;

use super::runtime_paths::RuntimePaths;

const ZIPFORMER_DIR_NAME: &str = "sherpa-onnx-zipformer-vi-30M-int8-2026-02-09";
const ZIPFORMER_ENCODER: &str = "encoder.int8.onnx";
const ZIPFORMER_DECODER: &str = "decoder.onnx";
const ZIPFORMER_JOINER: &str = "joiner.int8.onnx";
const ZIPFORMER_TOKENS: &str = "tokens.txt";
const ZIPFORMER_BPE: &str = "bpe.model";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceState {
    Ready,
    Missing,
    Incomplete,
    NotCompiled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceFileStatus {
    pub name: &'static str,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeResourceStatus {
    pub id: &'static str,
    pub label: &'static str,
    pub state: ResourceState,
    pub compiled: bool,
    pub root_path: String,
    pub detail: String,
    pub files: Vec<ResourceFileStatus>,
    pub preparation_files: Vec<ResourceFileStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeResourceSnapshot {
    pub resources: Vec<RuntimeResourceStatus>,
}

#[derive(Debug, Clone)]
pub struct ResourceRegistry {
    app_local_data: PathBuf,
    stt_model_dir: PathBuf,
    wake_model_dir: PathBuf,
    wake_keywords: PathBuf,
    wake_bpe_model: PathBuf,
}

impl ResourceRegistry {
    pub fn resolve(paths: &RuntimePaths) -> Result<Self, String> {
        let models_root = paths.app_local_data.join("models");
        let stt_model_dir = absolute_override(
            "ASSISTANT_ZIPFORMER_MODEL_DIR",
            models_root.join("stt").join(ZIPFORMER_DIR_NAME),
        )?;
        let wake_model_dir = absolute_override(
            "ASSISTANT_WAKE_MODEL_DIR",
            models_root
                .join("wake")
                .join("sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01"),
        )?;
        let wake_keywords = absolute_override(
            "ASSISTANT_WAKE_KEYWORDS",
            wake_model_dir.join("keywords.txt"),
        )?;
        let wake_bpe_model = wake_model_dir.join("bpe.model");

        Ok(Self {
            app_local_data: paths.app_local_data.clone(),
            stt_model_dir,
            wake_model_dir,
            wake_keywords,
            wake_bpe_model,
        })
    }

    pub fn stt_model_dir(&self) -> &Path {
        &self.stt_model_dir
    }

    /// Temporary compatibility accessor for the current desktop voice state.
    /// The returned path is the Zipformer encoder; the recognizer facade resolves
    /// its parent directory and loads the complete Zipformer bundle.
    pub fn whisper_model_path(&self) -> PathBuf {
        self.stt_model_dir.join(ZIPFORMER_ENCODER)
    }

    pub fn wake_model_dir(&self) -> &Path {
        &self.wake_model_dir
    }

    pub fn wake_keywords_path(&self) -> &Path {
        &self.wake_keywords
    }

    #[cfg(feature = "wake-word")]
    pub fn wake_bpe_model_path(&self) -> &Path {
        &self.wake_bpe_model
    }

    #[cfg(feature = "wake-word")]
    pub fn wake_tokens_path(&self) -> PathBuf {
        self.wake_model_dir.join("tokens.txt")
    }

    pub fn wake_settings_path(&self) -> PathBuf {
        self.app_local_data.join("settings").join("wake.json")
    }

    pub fn snapshot(&self) -> RuntimeResourceSnapshot {
        RuntimeResourceSnapshot {
            resources: vec![self.stt_status(), self.wake_status()],
        }
    }

    pub fn stt_status(&self) -> RuntimeResourceStatus {
        let files = vec![
            file_status("encoder", &self.stt_model_dir.join(ZIPFORMER_ENCODER)),
            file_status("decoder", &self.stt_model_dir.join(ZIPFORMER_DECODER)),
            file_status("joiner", &self.stt_model_dir.join(ZIPFORMER_JOINER)),
            file_status("tokens", &self.stt_model_dir.join(ZIPFORMER_TOKENS)),
        ];
        let present = files.iter().filter(|file| file.exists).count();
        let compiled = cfg!(feature = "voice-stt");
        let state = if !compiled {
            ResourceState::NotCompiled
        } else if present == files.len() {
            ResourceState::Ready
        } else if present == 0 {
            ResourceState::Missing
        } else {
            ResourceState::Incomplete
        };

        RuntimeResourceStatus {
            id: "stt_zipformer_vi",
            label: "Vietnamese Zipformer STT",
            state,
            compiled,
            root_path: self.stt_model_dir.display().to_string(),
            detail: match state {
                ResourceState::Ready => "sherpa-onnx Vietnamese Zipformer INT8 và toàn bộ runtime model file đã sẵn sàng.".into(),
                ResourceState::Missing => "Chưa có Vietnamese Zipformer model. Có thể cài trực tiếp từ Resource Setup.".into(),
                ResourceState::Incomplete => format!(
                    "Vietnamese Zipformer chưa đầy đủ: {present}/{} runtime model file đã có. Xóa thư mục model dở dang rồi cài lại để đảm bảo bundle nhất quán.",
                    files.len()
                ),
                ResourceState::NotCompiled => "Build chưa bật voice STT runtime; model có thể được chuẩn bị trước nhưng chưa được sử dụng.".into(),
            },
            files,
            preparation_files: vec![file_status(
                "bpe_model",
                &self.stt_model_dir.join(ZIPFORMER_BPE),
            )],
        }
    }

    /// Temporary compatibility method. Existing readiness/UI code calls the old
    /// name but receives the primary Zipformer STT status.
    pub fn whisper_status(&self) -> RuntimeResourceStatus {
        self.stt_status()
    }

    pub fn wake_status(&self) -> RuntimeResourceStatus {
        let config = SherpaWakeConfig::gigaspeech_int8(&self.wake_model_dir, &self.wake_keywords);
        let files = vec![
            file_status("encoder", &config.encoder),
            file_status("decoder", &config.decoder),
            file_status("joiner", &config.joiner),
            file_status("tokens", &config.tokens),
            file_status("keywords", &config.keywords),
        ];
        let present = files.iter().filter(|file| file.exists).count();
        let compiled = cfg!(feature = "wake-word");
        let state = if !compiled {
            ResourceState::NotCompiled
        } else if present == files.len() {
            ResourceState::Ready
        } else if present == 0 {
            ResourceState::Missing
        } else {
            ResourceState::Incomplete
        };

        RuntimeResourceStatus {
            id: "wake_word",
            label: "Wake Word Resources",
            state,
            compiled,
            root_path: self.wake_model_dir.display().to_string(),
            detail: match state {
                ResourceState::Ready => "Sherpa wake-word model và keywords đều sẵn sàng.".into(),
                ResourceState::Missing => "Chưa có wake-word resource nào tại model directory đã resolve.".into(),
                ResourceState::Incomplete => format!(
                    "Wake-word resources chưa đầy đủ: {present}/{} runtime file đã có.",
                    files.len()
                ),
                ResourceState::NotCompiled => "Build chưa bật feature `wake-word`; registry vẫn hiển thị layout để có thể chuẩn bị resource trước.".into(),
            },
            files,
            preparation_files: vec![file_status("bpe_model", &self.wake_bpe_model)],
        }
    }
}

fn file_status(name: &'static str, path: &Path) -> ResourceFileStatus {
    ResourceFileStatus {
        name,
        path: path.display().to_string(),
        exists: path.is_file(),
    }
}

fn absolute_override(name: &str, fallback: PathBuf) -> Result<PathBuf, String> {
    let Some(value) = env::var_os(name) else {
        return Ok(fallback);
    };
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!(
            "{name} must be an absolute path so resource resolution never depends on the process working directory"
        ))
    }
}
