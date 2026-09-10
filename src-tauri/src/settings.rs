//! Persisted preferences.
//!
//! One JSON file in the app's config directory. Every field is optional or has a default, so a
//! file written by an older build still loads — the alternative, a versioned migration for a
//! handful of preferences, would cost more than it saves.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Where renders land. `None` means "the videos folder", resolved by the front end.
    pub output_dir: Option<String>,
    /// Append the date to the output file name.
    pub date_in_filename: bool,
    /// Folder to watch for new G-code.
    pub watch_dir: Option<String>,
    pub watch_enabled: bool,
    /// Path to an FFmpeg the user pointed at, overriding whatever is on PATH.
    pub ffmpeg_path: Option<String>,
    pub auto_update: bool,
    /// Unix seconds of the last update check.
    pub last_update_check: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: None,
            date_in_filename: false,
            watch_dir: None,
            watch_enabled: false,
            ffmpeg_path: None,
            auto_update: true,
            last_update_check: None,
        }
    }
}

fn path_of(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(FILE))
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Settings {
    // A missing or unreadable file is not an error worth surfacing: it just means defaults.
    path_of(&app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let path = path_of(&app)?;
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    // Write beside the target and rename, so a crash cannot leave half a settings file.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegInfo {
    pub path: String,
    pub version: String,
}

/// Ask an FFmpeg what version it is.
///
/// With no path, whatever is on `PATH` is tried. Nothing is ever bundled and nothing is
/// downloaded: this only reports on a binary the user already has.
#[tauri::command]
pub fn probe_ffmpeg(path: Option<String>) -> Option<FfmpegInfo> {
    let program = path.unwrap_or_else(|| "ffmpeg".to_string());

    let mut command = Command::new(&program);
    command.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Without this the probe flashes a console window on top of the app.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next()?;
    // "ffmpeg version 7.0.2 Copyright (c) ..."
    let version = first
        .split_whitespace()
        .nth(2)
        .unwrap_or("bilinmiyor")
        .to_string();

    Some(FfmpegInfo {
        path: program,
        version,
    })
}
