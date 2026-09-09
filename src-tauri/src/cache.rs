//! Parse cache: file hash -> encoded IR buffer, under the app data directory.
//!
//! The cached bytes are exactly what the parser produced and exactly what the IPC layer hands
//! to the front end, so a cache hit is a file read and nothing else -- no decode, no re-encode.

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

const DIR: &str = "ir-cache";
/// Evict least-recently-used entries once the cache grows past this.
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = base.join(DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn entry_path(app: &AppHandle, key: &str) -> Result<PathBuf, String> {
    Ok(dir(app)?.join(format!("{key}.sfir")))
}

pub fn get(app: &AppHandle, key: &str) -> Option<Vec<u8>> {
    let path = entry_path(app, key).ok()?;
    let bytes = fs::read(&path).ok()?;
    if bytes.len() < skipframe_gcode::ir::HEADER_LEN || bytes[0..4] != skipframe_gcode::ir::MAGIC {
        let _ = fs::remove_file(&path);
        return None;
    }
    // Touch so LRU eviction keeps what people actually reopen.
    let _ = fs::File::open(&path).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    Some(bytes)
}

pub fn put(app: &AppHandle, key: &str, bytes: &[u8]) -> Result<(), String> {
    let path = entry_path(app, key)?;
    // Write to a temporary name first so a crash mid-write cannot leave a truncated entry that
    // later reads as valid.
    let tmp = path.with_extension("sfir.tmp");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    evict_if_needed(app);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct CacheStats {
    pub entries: usize,
    pub bytes: u64,
    pub path: String,
}

pub fn stats(app: &AppHandle) -> Result<CacheStats, String> {
    let dir = dir(app)?;
    let (entries, bytes) = walk(&dir);
    Ok(CacheStats {
        entries,
        bytes,
        path: dir.to_string_lossy().into_owned(),
    })
}

pub fn clear(app: &AppHandle) -> Result<(), String> {
    let dir = dir(app)?;
    for e in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let _ = fs::remove_file(e.path());
    }
    Ok(())
}

fn walk(dir: &Path) -> (usize, u64) {
    let mut entries = 0usize;
    let mut bytes = 0u64;
    if let Ok(read) = fs::read_dir(dir) {
        for e in read.flatten() {
            if let Ok(m) = e.metadata() {
                if m.is_file() {
                    entries += 1;
                    bytes += m.len();
                }
            }
        }
    }
    (entries, bytes)
}

fn evict_if_needed(app: &AppHandle) {
    let Ok(dir) = dir(app) else { return };
    let (_, total) = walk(&dir);
    if total <= MAX_BYTES {
        return;
    }
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    files.sort_by_key(|(t, _, _)| *t);

    let mut remaining = total;
    for (_, len, path) in files {
        if remaining <= MAX_BYTES {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            remaining = remaining.saturating_sub(len);
        }
    }
}
