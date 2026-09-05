use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AntigravitySettings {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityModelInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AntigravitySettingsView {
    pub current_model: Option<String>,
    pub current_effort: Option<String>,
    pub available_models: Vec<AntigravityModelInfo>,
    pub cli_binary: String,
    pub is_authenticated: bool,
}

#[derive(Debug, Clone)]
pub struct AntigravitySettingsStore {
    path: PathBuf,
}

impl AntigravitySettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<AntigravitySettings, String> {
        if !self.path.is_file() {
            return Ok(AntigravitySettings::default());
        }
        let bytes = fs::read(&self.path)
            .map_err(|error| format!("cannot read antigravity settings: {error}"))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("cannot parse antigravity settings: {error}"))
    }

    pub fn save(&self, settings: &AntigravitySettings) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "settings path has no parent directory".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create settings directory: {error}"))?;

        let bytes = serde_json::to_vec_pretty(settings)
            .map_err(|error| format!("cannot serialize settings: {error}"))?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("antigravity.json");
        let temporary = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));
        let backup = parent.join(format!(".{file_name}.{}.bak", Uuid::new_v4()));

        fs::write(&temporary, bytes)
            .map_err(|error| format!("cannot write temporary settings: {error}"))?;

        let had_existing = self.path.exists();
        if had_existing {
            if let Err(error) = fs::rename(&self.path, &backup) {
                let _ = fs::remove_file(&temporary);
                return Err(format!("cannot stage previous settings: {error}"));
            }
        }

        if let Err(error) = fs::rename(&temporary, &self.path) {
            if had_existing {
                let _ = fs::rename(&backup, &self.path);
            }
            return Err(format!("cannot commit settings file: {error}"));
        }

        if had_existing {
            let _ = fs::remove_file(&backup);
        }

        Ok(())
    }
}

pub fn fetch_available_models(binary_path: &str) -> Vec<AntigravityModelInfo> {
    let mut models = Vec::new();

    if let Ok(output) = Command::new(binary_path).arg("models").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("Fetching") {
                    continue;
                }
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let id = parts[0].trim().to_string();
                    let label = parts[1].trim().to_string();
                    if !id.is_empty() {
                        models.push(AntigravityModelInfo { id, label });
                    }
                } else {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if !parts.is_empty() {
                        let id = parts[0].to_string();
                        let label = parts[1..].join(" ");
                        models.push(AntigravityModelInfo {
                            id: id.clone(),
                            label: if label.is_empty() { id } else { label },
                        });
                    }
                }
            }
        }
    }

    if !models.is_empty() {
        return models;
    }

    vec![
        AntigravityModelInfo {
            id: "gemini-3.7-flash-high".into(),
            label: "Gemini 3.7 Flash (High Reasoning)".into(),
        },
        AntigravityModelInfo {
            id: "gemini-3.7-flash-medium".into(),
            label: "Gemini 3.7 Flash (Medium Reasoning)".into(),
        },
        AntigravityModelInfo {
            id: "gemini-3.7-flash-low".into(),
            label: "Gemini 3.7 Flash (Low Reasoning)".into(),
        },
        AntigravityModelInfo {
            id: "gemini-3.8-flash-high".into(),
            label: "Gemini 3.8 Flash (High Reasoning)".into(),
        },
        AntigravityModelInfo {
            id: "gemini-3.1-pro-high".into(),
            label: "Gemini 3.1 Pro (High Reasoning)".into(),
        },
        AntigravityModelInfo {
            id: "claude-sonnet-4-6".into(),
            label: "Claude Sonnet 4.6 (Thinking)".into(),
        },
        AntigravityModelInfo {
            id: "claude-opus-4-6-thinking".into(),
            label: "Claude Opus 4.6 (Thinking)".into(),
        },
        AntigravityModelInfo {
            id: "gpt-oss-120b-medium".into(),
            label: "GPT-OSS 120B (Medium)".into(),
        },
    ]
}

pub fn launch_cli_login(binary_path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args([
                "/c",
                "start",
                "Antigravity Google Sign-in",
                "cmd",
                "/k",
                binary_path,
            ])
            .creation_flags(0x00000010)
            .spawn()
            .map_err(|e| format!("cannot launch login window: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new(binary_path)
            .spawn()
            .map_err(|e| format!("cannot launch login: {e}"))?;
        Ok(())
    }
}
