mod cache;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            commands::parse_gcode,
            commands::list_plates,
            commands::file_cache_key,
            commands::cache_stats,
            commands::clear_cache,
            commands::bench_log,
            commands::write_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SkipFrame");
}
