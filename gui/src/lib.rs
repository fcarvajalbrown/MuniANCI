// Tauri app builder — registers all commands and plugins.
use tauri::Manager;

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::start_scan::start_scan,
            commands::export_report::export_report,
        ])
        .setup(|app| {
            // Dev tools only in debug builds.
            #[cfg(debug_assertions)]
            app.get_webview_window("main")
                .unwrap()
                .open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error al iniciar MuniANCI");
}