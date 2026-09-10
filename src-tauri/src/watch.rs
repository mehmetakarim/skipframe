//! Watching a slicer's output folder.
//!
//! Polling rather than an OS file-notification API, deliberately. A slicer writes a G-code file
//! over hundreds of milliseconds, so a notification of "file created" arrives long before the
//! file is worth reading — every notification-based implementation ends up adding exactly the
//! settling logic that polling gives away for free. Two seconds of latency on a job that takes
//! minutes to slice is not a cost anyone can feel, and it saves a dependency.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

const POLL: Duration = Duration::from_secs(2);
/// A file is offered only once its size has stopped changing for this many polls.
const STABLE_POLLS: u32 = 2;

/// Emitted with the path of a finished G-code file that appeared while watching.
pub const EVENT: &str = "skipframe://gcode-appeared";

#[derive(Default)]
pub struct WatchState(pub Mutex<Option<Watcher>>);

pub struct Watcher {
    stop: Arc<AtomicBool>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn start_watch(app: AppHandle, path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("{path} bir klasör değil"));
    }

    stop_watch(app.clone());

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let handle = app.clone();

    std::thread::spawn(move || {
        // Everything already in the folder is history, not news.
        let mut seen: HashSet<PathBuf> = list_gcode(&dir).into_iter().collect();
        let mut settling: HashMap<PathBuf, (u64, u32)> = HashMap::new();

        while !thread_stop.load(Ordering::Relaxed) {
            std::thread::sleep(POLL);
            if thread_stop.load(Ordering::Relaxed) {
                break;
            }

            for file in list_gcode(&dir) {
                if seen.contains(&file) {
                    continue;
                }
                let Ok(size) = std::fs::metadata(&file).map(|m| m.len()) else {
                    continue;
                };

                match settling.get(&file) {
                    Some((last, count)) if *last == size => {
                        let count = count + 1;
                        if count >= STABLE_POLLS {
                            settling.remove(&file);
                            seen.insert(file.clone());
                            let _ = handle.emit(EVENT, file.to_string_lossy().to_string());
                        } else {
                            settling.insert(file, (size, count));
                        }
                    }
                    _ => {
                        settling.insert(file, (size, 0));
                    }
                }
            }
        }
    });

    *app.state::<WatchState>()
        .0
        .lock()
        .map_err(|e| e.to_string())? = Some(Watcher { stop });
    Ok(())
}

#[tauri::command]
pub fn stop_watch(app: AppHandle) {
    if let Ok(mut slot) = app.state::<WatchState>().0.lock() {
        // Dropping the handle sets the stop flag; the thread ends on its next tick.
        *slot = None;
    }
}

fn list_gcode(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_gcode(p))
        .collect()
}

fn is_gcode(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".gcode")
        || lower.ends_with(".gco")
        || lower.ends_with(".g")
        || lower.ends_with(".gcode.3mf")
}

#[cfg(test)]
mod tests {
    use super::is_gcode;
    use std::path::Path;

    #[test]
    fn recognises_the_extensions_the_app_opens() {
        assert!(is_gcode(Path::new("/tmp/a.gcode")));
        assert!(is_gcode(Path::new("/tmp/a.GCODE")));
        assert!(is_gcode(Path::new("/tmp/a.gco")));
        assert!(is_gcode(Path::new("/tmp/plate.gcode.3mf")));
        // A plain .3mf is a model, not a sliced plate, and must not be queued.
        assert!(!is_gcode(Path::new("/tmp/model.3mf")));
        assert!(!is_gcode(Path::new("/tmp/photo.png")));
    }
}
