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
use tauri::{AppHandle, Emitter, Manager, State};
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
        }.to_string();
        AppError { code, message: e.to_string(), details: None }
    }
}

pub type CmdResult<T> = Result<T, AppError>;

#[tauri::command]
#[specta::specta]
pub async fn pack_start(
    app: AppHandle,
    registry: State<'_, Arc<JobRegistry>>,
    opts: PackOptions,
) -> CmdResult<String> {
    let job_id = Uuid::now_v7().to_string();
    let registry_arc = registry.inner().clone();
    let registry_for_task = registry_arc.clone();
    let app_for_emit = app.clone();
    let id = job_id.clone();
    let id_for_task = id.clone();

    // Reject GitHub URLs in v0.1.0 (URL parsing/clone exist in core::github but
    // are not wired through the orchestrator yet — see follow-up plan).
    if matches!(opts.target, projectpacker_core::types::PackTarget::GitHub(_)) {
        return Err(AppError {
            code: "not_implemented".into(),
            message: "GitHub URL packing is deferred to v0.2.0. Use a local folder.".into(),
            details: None,
        });
    }

    let (tx, rx) = std::sync::mpsc::channel::<ProgressEvent>();

    std::thread::spawn(move || {
        for ev in rx {
            let topic = format!("pack:{id}:progress");
            let _ = app_for_emit.emit(&topic, ev);
        }
    });

    let handle = tokio::task::spawn_blocking(move || {
        let root = match &opts.target {
            projectpacker_core::types::PackTarget::Folder(p) => p.clone(),
            projectpacker_core::types::PackTarget::GitHub(_) => return, // unreachable — guarded above
        };
        if let Ok(result) = pack::pack(&root, &opts, tx, &id_for_task) {
            registry_for_task.store_result(&id_for_task, result);
        }
        // Failure path: orchestrator did not emit Done; UI distinguishes by
        // absence-of-Done plus a timeout. Better error plumbing is a follow-up.
    });

    registry_arc.register(&job_id, handle);
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
pub async fn pack_get_result(registry: State<'_, Arc<JobRegistry>>, job_id: String) -> CmdResult<PackResult> {
    registry.inner()
        .take_result(&job_id)
        .ok_or(AppError { code: "result_not_ready".into(), message: format!("no result for job {job_id}"), details: None })
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

#[tauri::command]
#[specta::specta]
pub async fn save_to_file(path: PathBuf, contents: String) -> CmdResult<()> {
    std::fs::write(&path, contents).map_err(|e| AppError {
        code: "save_failed".into(),
        message: e.to_string(),
        details: None,
    })
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("settings.json")
}
