mod cache;
mod commands;
mod settings;
mod watch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .manage(watch::WatchState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|_app| {
            // The window draws its own title bar, so the native one would be a second set of
            // minimise, maximise and close buttons stacked above ours. macOS keeps its frame
            // and gets `titleBarStyle: Overlay` from the config instead, so the real traffic
            // lights float over our bar rather than being drawn twice.
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                if let Some(window) = _app.get_webview_window("main") {
                    let _ = window.set_decorations(false);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::parse_gcode,
            commands::list_plates,
            commands::file_cache_key,
            commands::cache_stats,
            commands::clear_cache,
            commands::bench_log,
            commands::write_export,
            settings::load_settings,
            settings::save_settings,
            settings::probe_ffmpeg,
            watch::start_watch,
            watch::stop_watch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SkipFrame");
}
