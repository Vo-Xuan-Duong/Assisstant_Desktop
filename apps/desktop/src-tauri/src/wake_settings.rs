use std::{fs, path::{Path, PathBuf}};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WakePreferences {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub phrase: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WakeSettingsStore {
    path: PathBuf,
}

impl WakeSettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<WakePreferences, String> {
        if !self.path.is_file() {
            return Ok(WakePreferences::default());
        }
        let bytes = fs::read(&self.path)
            .map_err(|error| format!("cannot read wake settings: {error}"))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("cannot parse wake settings: {error}"))
    }

    pub fn save(&self, preferences: &WakePreferences) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "wake settings path has no parent directory".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create wake settings directory: {error}"))?;

        let bytes = serde_json::to_vec_pretty(preferences)
            .map_err(|error| format!("cannot serialize wake settings: {error}"))?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("wake.json");
        let temporary = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));
        let backup = parent.join(format!(".{file_name}.{}.bak", Uuid::new_v4()));

        fs::write(&temporary, bytes)
            .map_err(|error| format!("cannot write temporary wake settings: {error}"))?;

        let had_existing = self.path.exists();
        if had_existing {
            if let Err(error) = fs::rename(&self.path, &backup) {
                let _ = fs::remove_file(&temporary);
                return Err(format!("cannot stage previous wake settings: {error}"));
            }
        }

        if let Err(error) = fs::rename(&temporary, &self.path) {
            if had_existing {
                let _ = fs::rename(&backup, &self.path);
            }
            let _ = fs::remove_file(&temporary);
            return Err(format!("cannot install wake settings: {error}"));
        }

        if had_existing {
            let _ = fs::remove_file(&backup);
        }
        Ok(())
    }
}
