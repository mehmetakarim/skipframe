//! Render progress in the macOS menu bar.
//!
//! A render is minutes long and the window is usually behind something else by then. The design
//! asks for the count and the time left up in the menu bar, next to the clock, drawn with the
//! template mark so the system can invert it with the rest of the bar.
//!
//! macOS only. Windows has no text beside a tray icon — the same thing there would be a tooltip
//! nobody sees — so on every other platform these calls do nothing.

use serde::Deserialize;

use crate::commands::IpcError;

/// What the front end reports while it renders. `None` ends the run.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderProgress {
    pub frame: u32,
    pub frame_count: u32,
    /// Seconds left, when there is an estimate yet.
    pub eta_s: Option<f64>,
}

impl RenderProgress {
    /// `436 / 570  ≈ 01:12`, or just the counts before there is an estimate.
    fn title(&self) -> String {
        let counts = format!("{} / {}", self.frame, self.frame_count);
        match self.eta_s {
            Some(eta) if eta.is_finite() && eta >= 0.0 => {
                let total = eta.round() as u64;
                format!("{counts}  ≈ {:02}:{:02}", total / 60, total % 60)
            }
            _ => counts,
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::Mutex;

    use tauri::image::Image;
    use tauri::tray::{TrayIcon, TrayIconBuilder};
    use tauri::AppHandle;

    use super::{IpcError, RenderProgress};

    /// The icon lives only while a render does, so the menu bar is not occupied by an app that is
    /// sitting idle.
    #[derive(Default)]
    pub struct MenuBar(Mutex<Option<TrayIcon>>);

    /// 36 px, black on transparency: a template image, which macOS recolours for the light or
    /// dark menu bar. The design's own rule — the gold layer turns black here too.
    const TEMPLATE: &[u8] = include_bytes!("../icons/menubar-template@2x.png");

    const TRAY_ID: &str = "render-progress";

    pub fn set(
        app: &AppHandle,
        state: &MenuBar,
        progress: Option<RenderProgress>,
    ) -> Result<(), IpcError> {
        let mut held = state.0.lock().unwrap();
        let Some(progress) = progress else {
            // Dropping our handle is not enough: `build` also registers the icon in the app's own
            // resource table, which keeps it in the bar showing the last count it was given.
            // Removing it by id is what closes it.
            held.take();
            app.remove_tray_by_id(TRAY_ID);
            return Ok(());
        };

        let title = progress.title();
        match held.as_ref() {
            Some(tray) => tray
                .set_title(Some(title))
                .map_err(|e| IpcError::new("other", e.to_string())),
            None => {
                let icon = Image::from_bytes(TEMPLATE)
                    .map_err(|e| IpcError::new("other", e.to_string()))?;
                let tray = TrayIconBuilder::with_id(TRAY_ID)
                    .icon(icon)
                    .icon_as_template(true)
                    .title(title)
                    .tooltip("SkipFrame render ediyor")
                    .build(app)
                    .map_err(|e| IpcError::new("other", e.to_string()))?;
                *held = Some(tray);
                Ok(())
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use tauri::AppHandle;

    use super::{IpcError, RenderProgress};

    /// Nothing to hold: there is no menu bar to put a counter in.
    #[derive(Default)]
    pub struct MenuBar {
        _unused: (),
    }

    pub fn set(
        _app: &AppHandle,
        _state: &MenuBar,
        progress: Option<RenderProgress>,
    ) -> Result<(), IpcError> {
        // Keeps the shared type honest off macOS: the title is still what would be shown, and
        // its tests run on every platform.
        let _ = progress.map(|p| p.title());
        Ok(())
    }
}

pub use platform::MenuBar;

/// Report where a render has got to, or `None` when it has finished, failed or been cancelled.
#[tauri::command]
pub fn render_progress(
    app: tauri::AppHandle,
    state: tauri::State<'_, MenuBar>,
    progress: Option<RenderProgress>,
) -> Result<(), IpcError> {
    platform::set(&app, &state, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_reads_as_the_design_writes_it() {
        let with_eta = RenderProgress {
            frame: 436,
            frame_count: 570,
            eta_s: Some(72.0),
        };
        assert_eq!(with_eta.title(), "436 / 570  ≈ 01:12");
    }

    #[test]
    fn an_estimate_that_is_not_there_yet_leaves_the_counts_alone() {
        let no_eta = RenderProgress {
            frame: 1,
            frame_count: 570,
            eta_s: None,
        };
        assert_eq!(no_eta.title(), "1 / 570");

        let silly = RenderProgress {
            frame: 1,
            frame_count: 570,
            eta_s: Some(f64::NAN),
        };
        assert_eq!(silly.title(), "1 / 570");
    }

    #[test]
    fn a_long_render_keeps_counting_in_minutes() {
        let long = RenderProgress {
            frame: 10,
            frame_count: 20000,
            eta_s: Some(3671.4),
        };
        assert_eq!(long.title(), "10 / 20000  ≈ 61:11");
    }
}
