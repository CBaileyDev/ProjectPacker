//! ProjectPacker Tauri shell.

pub mod commands;
pub mod jobs;
pub mod settings;

use std::sync::Arc;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let registry = Arc::new(jobs::JobRegistry::new());

    tauri::Builder::default()
        .manage(registry)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::pack_start,
            commands::pack_cancel,
            commands::pack_get_result,
            commands::validate_plan,
            commands::build_combined_prompt,
            commands::get_settings,
            commands::save_settings,
            commands::save_to_file,
        ])
        .setup(|app| {
            tracing::info!(
                "ProjectPacker started, version {}",
                env!("CARGO_PKG_VERSION")
            );
            let _ = app;
            std::panic::set_hook(Box::new(|info| {
                tracing::error!("PANIC in app process: {info}");
            }));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
