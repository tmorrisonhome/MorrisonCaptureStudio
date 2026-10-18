//! Settings that are remembered between runs (saved as JSON in %LOCALAPPDATA%).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("MorrisonCaptureStudio")
}

pub fn default_output_dir() -> PathBuf {
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Videos")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Morrison Capture Studio")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output_dir: PathBuf,
    pub audio_mode: u8,
    pub record_screen: bool,
    pub record_cursor: bool,
    pub highlight_clicks: bool,
    pub video_quality: usize,
    pub frame_rate: u32,
    pub export_format: usize,
    pub export_quality: usize,
    pub preview_volume: f32,
    pub keep_raw_tracks: bool,
    pub custom_region: [i32; 4],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            audio_mode: 0,
            record_screen: true,
            record_cursor: true,
            highlight_clicks: false,
            video_quality: 1,
            frame_rate: 30,
            export_format: 0,
            export_quality: 0,
            preview_volume: 0.85,
            keep_raw_tracks: true,
            custom_region: [0, 0, 1280, 720],
        }
    }
}

impl Settings {
    fn file() -> PathBuf {
        app_data_dir().join("settings.json")
    }
    pub fn load() -> Self {
        std::fs::read(Self::file())
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let _ = std::fs::create_dir_all(app_data_dir());
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::file(), b);
        }
    }
}
