//! ProjectPacker Tauri shell.

pub mod commands;
pub mod github;
pub mod jobs;
pub mod settings;

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_specta::collect_commands;

/// Single source of truth for the command surface and the wire types
/// exported to TypeScript. `run()` wires it into the Tauri invoke handler
/// and `emit-bindings` exports the same builder, so the two lists can
/// never drift apart.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::pack_start,
            commands::pack_cancel,
            commands::pack_get_result,
            commands::validate_plan,
            commands::build_combined_prompt,
            commands::get_settings,
            commands::save_settings,
            commands::save_pack_output,
            commands::github_set_token,
            commands::github_clear_token,
            commands::github_token_status,
            commands::github_get_user,
            commands::github_list_repos,
        ])
        .typ::<projectpacker_core::types::ProgressEvent>()
        .typ::<projectpacker_core::tokens::TokenModel>()
        .typ::<projectpacker_core::tokens::TokensPerModel>()
        // The engine-level `Redaction` (with `matched_excerpt`) is internal
        // to `secrets::engine`; the pack surface uses `PackRedaction` (no
        // excerpt by design — excerpts would echo the very secret we just
        // redacted into the security report). Don't expose `Redaction` to
        // TS to keep the wire surface tight.
        .typ::<projectpacker_core::types::PackRedaction>()
        // GitHub wire types — also flow into commands above. Listed
        // explicitly so TS imports stay tidy even if a command surface
        // shifts later.
        .typ::<github::GithubUser>()
        .typ::<github::GithubRepo>()
        .typ::<github::GithubRepoOwner>()
}

pub fn run() {
    let registry = Arc::new(jobs::JobRegistry::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Refocus the existing main window when a second launch is attempted,
            // and forward argv to the renderer so it can switch target on a
            // dropped-folder path.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
            let _ = app.emit("single-instance", argv);
        }))
        .manage(registry)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .invoke_handler(specta_builder().invoke_handler())
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
