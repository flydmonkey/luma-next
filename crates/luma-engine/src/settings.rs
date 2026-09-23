use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct RegionSettings {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

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
    pub region: Option<RegionSettings>,
    #[serde(default)]
    pub window_id: Option<String>,
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default = "default_mic_device")]
    pub mic_device_id: String,
    pub quality: String,
    #[serde(default = "default_encoder")]
    pub encoder: String,
    #[serde(default = "default_start_stop_hotkey")]
    pub hotkey_start_stop: String,
    #[serde(default = "default_pause_hotkey")]
    pub hotkey_pause_resume: String,
}

impl Settings {
    pub fn defaults(output_directory: PathBuf) -> Self {
        Self {
            output_directory: output_directory.to_string_lossy().into_owned(),
            record_system_audio: true,
            record_microphone: false,
            capture_mode: default_capture_mode(),
            display_id: default_display_id(),
            region: None,
            window_id: None,
            game_id: None,
            mic_device_id: default_mic_device(),
            quality: "1080p30".into(),
            encoder: default_encoder(),
            hotkey_start_stop: default_start_stop_hotkey(),
            hotkey_pause_resume: default_pause_hotkey(),
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
        if !matches!(
            self.capture_mode.as_str(),
            "display" | "window" | "region" | "game" | "audio_only"
        ) {
            return Err("capture_mode must be display, window, region, game, or audio_only".into());
        }
        if self.capture_mode == "region" && self.region.is_none() {
            return Err("region is required for region capture".into());
        }
        if let Some(region) = self.region
            && (region.width < 32 || region.height < 32)
        {
            return Err("region must be at least 32x32 pixels".into());
        }
        if self.capture_mode == "window" && self.window_id.as_deref().unwrap_or_default().is_empty()
        {
            return Err("window_id is required for window capture".into());
        }
        if self.capture_mode == "game" && self.game_id.as_deref().unwrap_or_default().is_empty() {
            return Err("game_id is required for game capture".into());
        }
        if !matches!(self.quality.as_str(), "1080p30" | "1440p30" | "2160p30") {
            return Err("quality must be 1080p30, 1440p30, or 2160p30".into());
        }
        if self.encoder.is_empty() {
            return Err("encoder cannot be empty".into());
        }
        if self.hotkey_start_stop != "Ctrl+Shift+R" || self.hotkey_pause_resume != "Ctrl+Shift+P" {
            return Err("M6 currently supports fixed hotkeys Ctrl+Shift+R and Ctrl+Shift+P".into());
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

fn default_start_stop_hotkey() -> String {
    "Ctrl+Shift+R".into()
}

fn default_pause_hotkey() -> String {
    "Ctrl+Shift+P".into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_mode_requires_a_valid_rectangle() {
        let mut settings = Settings::defaults(PathBuf::from(r"C:\Videos\Luma"));
        settings.capture_mode = "region".into();
        assert!(settings.validate().is_err());
        settings.region = Some(RegionSettings {
            x: -100,
            y: 20,
            width: 640,
            height: 360,
        });
        assert!(settings.validate().is_ok());
        settings.region.as_mut().expect("region").width = 30;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn only_registered_m6_hotkeys_are_accepted() {
        let mut settings = Settings::defaults(PathBuf::from(r"C:\Videos\Luma"));
        assert!(settings.validate().is_ok());
        settings.hotkey_start_stop = "Ctrl+Alt+R".into();
        assert!(settings.validate().is_err());
    }
}
