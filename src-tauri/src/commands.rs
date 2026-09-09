//! IPC surface.
//!
//! `parse_gcode` returns [`tauri::ipc::Response`] carrying the raw `SFIR` buffer. It must stay
//! that way: serialising the IR as JSON would turn a 25 MB TypedArray into a several-hundred
//! megabyte string and would take longer than the parse itself.

use std::path::PathBuf;

use tauri::ipc::Response;
use tauri::AppHandle;

use crate::cache;

/// Parse a file (or serve it from the cache) and return the encoded IR as raw bytes.
///
/// Everything descriptive -- dialect, printer, warnings, timings -- travels inside the buffer's
/// own meta section, so this command's payload stays a pure byte buffer.
#[tauri::command]
pub async fn parse_gcode(
    app: AppHandle,
    path: String,
    plate: Option<u32>,
    use_cache: Option<bool>,
) -> Result<Response, String> {
    let use_cache = use_cache.unwrap_or(true);
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let path = PathBuf::from(path);
        let key = skipframe_gcode::cache_key(&path).map_err(|e| e.to_string())?;
        let key = match plate {
            Some(p) => format!("{key}-p{p}"),
            None => key,
        };

        if use_cache {
            if let Some(hit) = cache::get(&app, &key) {
                return Ok(hit);
            }
        }

        let ir = skipframe_gcode::parse_file(&path, plate).map_err(|e| e.to_string())?;
        let buf = ir.encode();
        let _ = cache::put(&app, &key, &buf);
        Ok(buf)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(Response::new(bytes))
}

/// Plate numbers inside a `.gcode.3mf`. Plain `.gcode` always answers `[1]`.
#[tauri::command]
pub async fn list_plates(path: String) -> Result<Vec<u32>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        skipframe_gcode::container::list_plates(&PathBuf::from(path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Content hash of a file, used by the front end to key its own in-memory state.
#[tauri::command]
pub async fn file_cache_key(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        skipframe_gcode::cache_key(&PathBuf::from(path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cache_stats(app: AppHandle) -> Result<cache::CacheStats, String> {
    cache::stats(&app)
}

#[tauri::command]
pub fn clear_cache(app: AppHandle) -> Result<(), String> {
    cache::clear(&app)
}
