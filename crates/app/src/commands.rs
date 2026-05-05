use crate::jobs::JobRegistry;
use crate::settings::{load_or_default, save, Settings};
use projectpacker_core::error::CoreError;
use projectpacker_core::pack;
use projectpacker_core::protocol::{self, PlanValidation};
use projectpacker_core::types::{PackOptions, PackResult, ProgressEvent};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl From<CoreError> for AppError {
    fn from(e: CoreError) -> Self {
        let code = match &e {
            CoreError::InvalidTarget(_) => "invalid_target",
            CoreError::PathNotFound(_) => "path_not_found",
            CoreError::CloneFailed(_) => "clone_failed",
            CoreError::TokenizerUnavailable(_) => "tokenizer_unavailable",
            CoreError::PlanInvalid { .. } => "plan_invalid",
            CoreError::Cancelled => "cancelled",
            _ => "internal",
        }
        .to_string();
        AppError {
            code,
            message: e.to_string(),
            details: None,
        }
    }
}

pub type CmdResult<T> = Result<T, AppError>;

#[tauri::command]
#[specta::specta]
pub async fn pack_start(
    registry: State<'_, Arc<JobRegistry>>,
    opts: PackOptions,
    on_event: tauri::ipc::Channel<ProgressEvent>,
) -> CmdResult<String> {
    let job_id = Uuid::now_v7().to_string();
    let registry_arc = registry.inner().clone();
    let registry_for_task = registry_arc.clone();
    let id_for_task = job_id.clone();

    let (tx, rx) = std::sync::mpsc::channel::<ProgressEvent>();

    let on_event_for_relay = on_event.clone();
    std::thread::spawn(move || {
        for ev in rx {
            let _ = on_event_for_relay.send(ev);
        }
    });

    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    // Register BEFORE spawning so a fast-completing task can't race past
    // the registration and leak its token entry.
    registry_arc.register(&job_id, cancel);

    tokio::task::spawn_blocking(move || {
        let tx_for_err = tx.clone();
        match pack::pack(&opts.target, &opts, tx, &id_for_task, cancel_for_task) {
            Ok(result) => registry_for_task.store_result(&id_for_task, result),
            Err(e) => {
                // Evict the token entry too — store_result would have done
                // this, but the error path needs to clean up explicitly.
                registry_for_task.discard(&id_for_task);
                let _ = tx_for_err.send(ProgressEvent::Error {
                    message: e.to_string(),
                    fatal: true,
                });
            }
        }
    });

    Ok(job_id)
}

#[tauri::command]
#[specta::specta]
pub async fn pack_cancel(registry: State<'_, Arc<JobRegistry>>, job_id: String) -> CmdResult<()> {
    registry.inner().cancel(&job_id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn pack_get_result(
    registry: State<'_, Arc<JobRegistry>>,
    job_id: String,
) -> CmdResult<PackResult> {
    registry.inner().take_result(&job_id).ok_or(AppError {
        code: "result_not_ready".into(),
        message: format!("no result for job {job_id}"),
        details: None,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn validate_plan(plan_md: String, protocol_version: String) -> CmdResult<PlanValidation> {
    protocol::validate_plan(&plan_md, &protocol_version).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn build_combined_prompt(plan_md: String, protocol_version: String) -> CmdResult<String> {
    protocol::build_combined_prompt(&plan_md, &protocol_version).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(app: AppHandle) -> CmdResult<Settings> {
    Ok(load_or_default(&settings_path(&app)))
}

#[tauri::command]
#[specta::specta]
pub async fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<Settings> {
    save(&settings_path(&app), &settings).map_err(|e| AppError {
        code: "settings_save_failed".into(),
        message: e.to_string(),
        details: None,
    })?;
    Ok(settings)
}

/// Show a save dialog and write `contents` to the user-chosen path.
///
/// The path comes from the OS save dialog, not the renderer — a compromised
/// renderer cannot supply an arbitrary path. Returns `Some(path)` on success
/// or `None` if the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn save_pack_output(
    app: AppHandle,
    suggested_filename: String,
    contents: String,
) -> CmdResult<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&suggested_filename)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let chosen = rx.await.map_err(|e| AppError {
        code: "dialog_failed".into(),
        message: e.to_string(),
        details: None,
    })?;
    let Some(file_path) = chosen else {
        return Ok(None);
    };
    let path: PathBuf = file_path.into_path().map_err(|e| AppError {
        code: "invalid_path".into(),
        message: e.to_string(),
        details: None,
    })?;
    std::fs::write(&path, contents).map_err(|e| AppError {
        code: "save_failed".into(),
        message: e.to_string(),
        details: None,
    })?;
    Ok(Some(path.display().to_string()))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("settings.json")
}
