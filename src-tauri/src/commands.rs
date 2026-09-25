//! IPC surface.
//!
//! `parse_gcode` returns [`tauri::ipc::Response`] carrying the raw `SFIR` buffer. It must stay
//! that way: serialising the IR as JSON would turn a 25 MB TypedArray into a several-hundred
//! megabyte string and would take longer than the parse itself.

use std::path::PathBuf;

use tauri::ipc::Response;
use tauri::AppHandle;

use crate::cache;

/// What a failed command tells the front end.
///
/// A bare string would have to be translated by matching on English prose, and the interface
/// is Turkish. `code` is [`skipframe_gcode::Error::code`] where the failure came from the
/// parser, and a coarse label otherwise; `message` is always the original English, which is
/// what gets shown when a code is not recognised. `detail` carries particulars the interface
/// shows — the line a truncated file stops at — and is left out when there are none.
#[derive(Debug, serde::Serialize)]
pub struct IpcError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub detail: serde_json::Value,
}

impl From<skipframe_gcode::Error> for IpcError {
    fn from(e: skipframe_gcode::Error) -> Self {
        IpcError {
            code: e.code().to_string(),
            message: e.to_string(),
            detail: e.detail(),
        }
    }
}

impl IpcError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        IpcError {
            code: code.to_string(),
            message: message.into(),
            detail: serde_json::Value::Null,
        }
    }

    fn other(code: &str, e: impl std::fmt::Display) -> Self {
        IpcError::new(code, e.to_string())
    }
}

impl From<String> for IpcError {
    fn from(message: String) -> Self {
        IpcError::new("other", message)
    }
}

impl From<&str> for IpcError {
    fn from(message: &str) -> Self {
        IpcError::from(message.to_string())
    }
}

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
) -> Result<Response, IpcError> {
    let use_cache = use_cache.unwrap_or(true);
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, IpcError> {
        let path = PathBuf::from(path);
        let key = skipframe_gcode::cache_key(&path)?;
        let key = match plate {
            Some(p) => format!("{key}-p{p}"),
            None => key,
        };

        if use_cache {
            if let Some(mut hit) = cache::get(&app, &key) {
                // The cache is keyed by content, so the same bytes under two names share an
                // entry. The name and size belong to the path that was opened, not to the
                // content, and must be put back before the front end sees them.
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed.gcode");
                let bytes = std::fs::metadata(&path).ok().map(|m| m.len());
                skipframe_gcode::ir::retag(&mut hit, name, bytes);
                return Ok(hit);
            }
        }

        let ir = skipframe_gcode::parse_file(&path, plate)?;
        let buf = ir.encode();
        let _ = cache::put(&app, &key, &buf);
        Ok(buf)
    })
    .await
    .map_err(|e| IpcError::other("worker", e))??;

    Ok(Response::new(bytes))
}

/// Plate numbers inside a `.gcode.3mf`. Plain `.gcode` always answers `[1]`.
#[tauri::command]
pub async fn list_plates(path: String) -> Result<Vec<u32>, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        skipframe_gcode::container::list_plates(&PathBuf::from(path)).map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::other("worker", e))?
}

/// Content hash of a file, used by the front end to key its own in-memory state.
#[tauri::command]
pub async fn file_cache_key(path: String) -> Result<String, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        skipframe_gcode::cache_key(&PathBuf::from(path)).map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::other("worker", e))?
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
pub fn write_export(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<(), String> {
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
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;

    // The share screen plays the video back before it is published. The asset protocol's scope
    // is empty in the config, and this grants exactly one file: one SkipFrame itself just wrote,
    // and only an MP4. Nothing else on disk becomes readable by the webview.
    let is_mp4 = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
    if is_mp4 {
        use tauri::Manager;
        let _ = app.asset_protocol_scope().allow_file(&path);
    }
    Ok(())
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

/// Free bytes on the volume that holds `path`, for checking a render will fit before starting it.
///
/// `path` may be a file that does not exist yet — the output the save dialog just named — so the
/// nearest existing folder above it is what gets asked.
#[tauri::command]
pub fn free_space(path: String) -> Result<u64, IpcError> {
    let mut dir = PathBuf::from(path);
    while !dir.is_dir() {
        if !dir.pop() {
            return Err(IpcError::new("io", "no existing folder in the path"));
        }
    }
    available_bytes(&dir).map_err(|e| IpcError::other("io", e))
}

#[cfg(windows)]
fn available_bytes(dir: &std::path::Path) -> std::io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0u64;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call, `available` is a
    // valid out pointer, and the two totals are optional and passed as null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(available)
    }
}

#[cfg(unix)]
fn available_bytes(dir: &std::path::Path) -> std::io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;

    let c = std::ffi::CString::new(dir.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    // SAFETY: `c` is a valid NUL-terminated path and `stats` is a zeroed statvfs owned here.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut stats) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Blocks available to an unprivileged user, not the root reserve.
    Ok(stats.f_bavail as u64 * stats.f_frsize as u64)
}

/// The readable comment lines at the top of a G-code file — where a slicer names itself.
///
/// Also answers for a file that failed to parse, so a copied error report can say which slicer
/// wrote it.
#[tauri::command]
pub async fn gcode_header(path: String) -> Result<Vec<String>, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        skipframe_gcode::header::read_header(
            &PathBuf::from(path),
            skipframe_gcode::header::MAX_LINES,
        )
    })
    .await
    .map_err(|e| IpcError::other("worker", e))?
    .map_err(IpcError::from)
}

/// Open a pre-filled GitHub issue about a slicer SkipFrame did not recognise.
///
/// Nothing is sent. The browser opens an issue draft on this project's repository containing
/// only the file's header comments — the slicer's banner and first settings, never geometry, a
/// thumbnail or the file's name — and the user reads it, edits it and submits it, or does not.
#[tauri::command]
pub async fn report_dialect(
    app: AppHandle,
    path: String,
    layers: Option<u32>,
) -> Result<(), IpcError> {
    let header = gcode_header(path).await?;
    let url = dialect_issue_url(&header, layers)?;
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| IpcError::other("browser_open_failed", e))
}

fn dialect_issue_url(header: &[String], layers: Option<u32>) -> Result<String, IpcError> {
    // The banner is the first line that names something; fall back to a plain title.
    let banner = header
        .iter()
        .find(|l| !l.ends_with("BLOCK_START") && !l.ends_with("BLOCK_END"))
        .map(|l| l.chars().take(80).collect::<String>())
        .unwrap_or_else(|| "no header comments".into());
    let title = format!("Unrecognised slicer: {banner}");

    let layers_line = match layers {
        Some(n) => format!("SkipFrame read it with the fallback reader and inferred {n} layers."),
        None => "SkipFrame read it with the fallback reader.".to_string(),
    };
    let comments = if header.is_empty() {
        "(the file has no header comments)".to_string()
    } else {
        header
            .iter()
            .map(|l| format!("; {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let body = format!(
        "SkipFrame {version} did not recognise the slicer that wrote a G-code file. {layers_line}\n\n\
         Header comments from the file — no geometry, no thumbnail, no file name:\n\n\
         ```\n{comments}\n```\n\n\
         <!-- Anything that helps: the slicer and its version, and a link to the file if you can \
         share it. Remove anything above you would rather not post. -->\n",
        version = env!("CARGO_PKG_VERSION"),
    );

    let base = format!("{}/issues/new", env!("CARGO_PKG_REPOSITORY"));
    url::Url::parse_with_params(&base, &[("title", title.as_str()), ("body", body.as_str())])
        .map(String::from)
        .map_err(|e| IpcError::other("other", e))
}

#[cfg(test)]
mod system_tests {
    use super::*;

    #[test]
    fn the_issue_draft_carries_the_header_and_nothing_else() {
        let header = vec![
            "HEADER_BLOCK_START".to_string(),
            "generated by FooSlicer 3.1 on 2026-09-15".to_string(),
            "total layer number: 318".to_string(),
        ];
        let url = url::Url::parse(&dialect_issue_url(&header, Some(318)).unwrap()).unwrap();
        assert_eq!(url.host_str(), Some("github.com"));
        assert_eq!(url.path(), "/mehmetakarim/skipframe/issues/new");

        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            q["title"],
            "Unrecognised slicer: generated by FooSlicer 3.1 on 2026-09-15"
        );
        assert!(q["body"].contains("; total layer number: 318"));
        assert!(q["body"].contains("inferred 318 layers"));
        assert_eq!(q.len(), 2, "only a title and a body");
    }

    #[test]
    fn free_space_finds_the_folder_above_a_file_that_does_not_exist_yet() {
        let dir = std::env::temp_dir();
        let missing = dir.join("skipframe-not-written-yet").join("clip.mp4");
        let bytes = free_space(missing.to_string_lossy().into_owned()).unwrap();
        assert!(bytes > 0);
    }
}

/// The front end is up: show the main window and take the splash down.
///
/// Called once, from the boot path that ends the splash. Calling it twice is harmless — the
/// window is already visible and the splash already gone.
#[tauri::command]
pub fn app_ready(app: AppHandle) {
    reveal_main_window(&app);
}

pub fn reveal_main_window(app: &AppHandle) {
    use tauri::Manager;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    if let Some(splash) = app.get_webview_window("splash") {
        let _ = splash.close();
    }
}

/// Commits behind this binary, or `None` when it was built without git history.
#[tauri::command]
pub fn build_number() -> Option<&'static str> {
    option_env!("SKIPFRAME_BUILD")
}

/// The licences of everything SkipFrame ships, generated by `npm run licenses` and bundled.
///
/// Read from disk rather than embedded in the binary: it is over a megabyte of other people's
/// text, and it is read once, by someone who pressed a button to see it.
#[tauri::command]
pub fn third_party_licenses(app: AppHandle) -> Result<String, IpcError> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve(
            "licenses/THIRD-PARTY.txt",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| IpcError::other("io", e))?;
    std::fs::read_to_string(path).map_err(|e| IpcError::other("io", e))
}

/// Open the About window, or bring it forward if it is already up.
#[tauri::command]
pub async fn open_about(app: AppHandle) -> Result<(), IpcError> {
    use tauri::Manager;
    if let Some(existing) = app.get_webview_window("about") {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(&app, "about", tauri::WebviewUrl::App("about.html".into()))
        .title("SkipFrame Hakkında")
        .inner_size(420.0, 400.0)
        .min_inner_size(420.0, 400.0)
        .center()
        .theme(Some(tauri::Theme::Dark))
        .build()
        .map(|_| ())
        .map_err(|e| IpcError::other("other", e))
}
