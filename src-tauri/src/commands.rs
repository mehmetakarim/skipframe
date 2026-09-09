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

/// Print a line from the phase-0 harness so it lands in the terminal that started `tauri dev`.
///
/// It prints unconditionally, because the numbers that matter come from a release build. In a
/// bundled app there is no console attached on Windows and nothing reads stdout on macOS, so
/// this is inert unless someone launched the binary from a terminal.
#[tauri::command]
pub fn bench_log(line: String) {
    println!("[bench] {line}");
}

/// Write an exported file.
///
/// The bytes arrive as the request's raw body — an encoded video is tens of megabytes, and
/// letting it become a JSON array of numbers would cost more than the encode did. The
/// destination travels in a header because a command can carry exactly one raw body.
///
/// Writing here rather than through the filesystem plugin means the destination the user chose
/// in the save dialog is the destination used, with no scope list to keep in sync.
#[tauri::command]
pub fn write_export(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let encoded = request
        .headers()
        .get("x-sf-path")
        .and_then(|v| v.to_str().ok())
        .ok_or("no output path was given")?;
    let path = PathBuf::from(percent_decode(encoded)?);

    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes,
        _ => return Err("expected the file contents as a raw body".into()),
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Percent-decode a UTF-8 path. Headers are ASCII, so the front end encodes the path before
/// sending it; anything else would mangle the first non-Latin file name someone exports.
fn percent_decode(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes
                .get(i + 1..i + 3)
                .ok_or("output path ends in a truncated escape")?;
            let hex = std::str::from_utf8(hex).map_err(|_| "output path is not valid UTF-8")?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| "output path has a bad escape")?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "output path is not valid UTF-8".into())
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_paths_with_spaces_and_non_ascii() {
        assert_eq!(
            percent_decode("C%3A%2FVideos%2Fbir%20baski.mp4").unwrap(),
            "C:/Videos/bir baski.mp4"
        );
        // "çıktı.mp4" — the case a byte-blind decoder would corrupt.
        assert_eq!(
            percent_decode("%C3%A7%C4%B1kt%C4%B1.mp4").unwrap(),
            "çıktı.mp4"
        );
        assert_eq!(percent_decode("plain.mp4").unwrap(), "plain.mp4");
        assert!(percent_decode("bad%2").is_err());
    }
}
