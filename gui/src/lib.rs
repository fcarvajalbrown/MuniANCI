// Tauri app builder — registers all commands and plugins.
use tauri::Manager;

mod assistant;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(assistant::AssistantState::new())
        .invoke_handler(tauri::generate_handler![
            commands::start_scan::start_scan,
            commands::export_report::export_report,
            commands::branding::app_branding,
            assistant::assistant_status,
        ])
        .setup(|app| {
            // Start the MuniGPT backend sidecar (non-blocking; polls for readiness).
            assistant::start(&app.handle().clone());

            // Dev tools only in debug builds.
            #[cfg(debug_assertions)]
            app.get_webview_window("main")
                .unwrap()
                .open_devtools();
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar MuniANCI");

    // Reap the backend process tree when the app exits.
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            assistant::shutdown(app_handle);
        }
    });
}
