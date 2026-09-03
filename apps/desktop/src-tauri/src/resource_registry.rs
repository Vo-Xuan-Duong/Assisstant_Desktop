use std::{env, path::{Path, PathBuf}};

use serde::Serialize;
use voice_runtime::wake::SherpaWakeConfig;

use super::runtime_paths::RuntimePaths;

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
    whisper_model: PathBuf,
    wake_model_dir: PathBuf,
    wake_keywords: PathBuf,
    wake_bpe_model: PathBuf,
}

impl ResourceRegistry {
    pub fn resolve(paths: &RuntimePaths) -> Result<Self, String> {
        let models_root = paths.app_local_data.join("models");
        let whisper_model = absolute_override(
            "ASSISTANT_WHISPER_MODEL",
            models_root.join("whisper").join("ggml-base.bin"),
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
            whisper_model,
            wake_model_dir,
            wake_keywords,
            wake_bpe_model,
        })
    }

    pub fn whisper_model_path(&self) -> &Path {
        &self.whisper_model
    }

    pub fn wake_model_dir(&self) -> &Path {
        &self.wake_model_dir
    }

    pub fn wake_keywords_path(&self) -> &Path {
        &self.wake_keywords
    }

    pub fn wake_bpe_model_path(&self) -> &Path {
        &self.wake_bpe_model
    }

    pub fn wake_tokens_path(&self) -> PathBuf {
        self.wake_model_dir.join("tokens.txt")
    }

    pub fn wake_settings_path(&self) -> PathBuf {
        self.app_local_data.join("settings").join("wake.json")
    }

    pub fn snapshot(&self) -> RuntimeResourceSnapshot {
        RuntimeResourceSnapshot {
            resources: vec![self.whisper_status(), self.wake_status()],
        }
    }

    pub fn whisper_status(&self) -> RuntimeResourceStatus {
        let exists = self.whisper_model.is_file();
        let compiled = cfg!(feature = "voice-whisper");
        let state = if !compiled {
            ResourceState::NotCompiled
        } else if exists {
            ResourceState::Ready
        } else {
            ResourceState::Missing
        };

        RuntimeResourceStatus {
            id: "whisper",
            label: "Local Whisper STT",
            state,
            compiled,
            root_path: self
                .whisper_model
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .display()
                .to_string(),
            detail: match state {
                ResourceState::Ready => "Whisper feature và model file đều sẵn sàng.".into(),
                ResourceState::Missing => "Build có Whisper nhưng model file chưa được cài.".into(),
                ResourceState::NotCompiled => "Build chưa bật feature `voice-whisper`; model có thể được chuẩn bị trước nhưng chưa được runtime sử dụng.".into(),
                ResourceState::Incomplete => unreachable!("Whisper is a single-file resource"),
            },
            files: vec![ResourceFileStatus {
                name: "model",
                path: self.whisper_model.display().to_string(),
                exists,
            }],
            preparation_files: Vec::new(),
        }
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
