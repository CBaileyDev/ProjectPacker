//! ProjectPacker Tauri shell.

pub mod commands;
pub mod jobs;
pub mod settings;

use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    let registry = Arc::new(jobs::JobRegistry::new());

    tauri::Builder::default()
        .manage(registry)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Focus the existing main window when a second launch is attempted.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::pack_start,
            commands::pack_cancel,
            commands::pack_get_result,
            commands::validate_plan,
            commands::build_combined_prompt,
            commands::get_settings,
            commands::save_settings,
            commands::save_pack_output,
        ])
        .setup(|app| {
            log::info!(
                "ProjectPacker started, version {}",
                env!("CARGO_PKG_VERSION")
            );
            let _ = app;
            std::panic::set_hook(Box::new(|info| {
                log::error!("PANIC in app process: {info}");
            }));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
