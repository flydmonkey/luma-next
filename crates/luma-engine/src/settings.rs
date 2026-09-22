use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub output_directory: String,
    pub record_system_audio: bool,
    pub record_microphone: bool,
    #[serde(default = "default_capture_mode")]
    pub capture_mode: String,
    #[serde(default = "default_display_id")]
    pub display_id: String,
    #[serde(default)]
    pub window_id: Option<String>,
    #[serde(default = "default_mic_device")]
    pub mic_device_id: String,
    pub quality: String,
    #[serde(default = "default_encoder")]
    pub encoder: String,
}

impl Settings {
    pub fn defaults(output_directory: PathBuf) -> Self {
        Self {
            output_directory: output_directory.to_string_lossy().into_owned(),
            record_system_audio: true,
            record_microphone: false,
            capture_mode: default_capture_mode(),
            display_id: default_display_id(),
            window_id: None,
            mic_device_id: default_mic_device(),
            quality: "1080p30".into(),
            encoder: default_encoder(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let output = Path::new(&self.output_directory);
        if !output.is_absolute() {
            return Err("output_directory must be an absolute path".into());
        }
        if !self.record_system_audio && !self.record_microphone {
            return Err("at least one of system audio or microphone must be enabled".into());
        }
        if !matches!(self.capture_mode.as_str(), "display" | "window") {
            return Err("capture_mode must be display or window".into());
        }
        if self.capture_mode == "window" && self.window_id.as_deref().unwrap_or_default().is_empty()
        {
            return Err("window_id is required for window capture".into());
        }
        if self.quality != "1080p30" {
            return Err("only quality 1080p30 is currently supported".into());
        }
        if self.encoder.is_empty() {
            return Err("encoder cannot be empty".into());
        }
        Ok(())
    }
}

fn default_encoder() -> String {
    "obs_x264".into()
}

fn default_capture_mode() -> String {
    "display".into()
}

fn default_display_id() -> String {
    "primary".into()
}

fn default_mic_device() -> String {
    "default".into()
}

pub struct SettingsStore {
    path: PathBuf,
    current: Settings,
}

impl SettingsStore {
    pub fn load(path: PathBuf, default_output: PathBuf) -> Result<Self, String> {
        let current = if path.exists() {
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            serde_json::from_slice::<Settings>(&bytes)
                .map_err(|error| format!("invalid settings file {}: {error}", path.display()))?
        } else {
            Settings::defaults(default_output)
        };
        current.validate()?;
        Ok(Self { path, current })
    }

    pub fn current(&self) -> &Settings {
        &self.current
    }

    pub fn save(&mut self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let temporary = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&settings)
            .map_err(|error| format!("failed to serialize settings: {error}"))?;
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|error| format!("failed to replace {}: {error}", self.path.display()))?;
        }
        std::fs::rename(&temporary, &self.path)
            .map_err(|error| format!("failed to replace {}: {error}", self.path.display()))?;
        self.current = settings;
        Ok(())
    }
}

pub fn default_output_directory() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Videos")
        .join("Luma")
}

pub fn default_settings_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LumaNext")
        .join("settings.json")
}
