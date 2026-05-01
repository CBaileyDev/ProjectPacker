//! ProjectPacker Tauri shell.

pub mod commands;
pub mod jobs;
pub mod settings;

use std::sync::Arc;
use tracing_subscriber::prelude::*;

pub fn run() {
    init_tracing();

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

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();

    // Use indented HierarchicalLayer in debug builds or when RUST_LOG_TREE=1 is set.
    let use_tree = cfg!(debug_assertions)
        || std::env::var("RUST_LOG_TREE").as_deref() == Ok("1");

    if use_tree {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_tree::HierarchicalLayer::new(2))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    }
}
