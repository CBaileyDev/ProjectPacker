use crate::github;
use crate::jobs::JobRegistry;
use crate::settings::{load_or_default, save, Settings};
use projectpacker_core::error::CoreError;
use projectpacker_core::pack;
use projectpacker_core::protocol::{self, PlanValidation};
use projectpacker_core::types::{PackOptions, PackResult, PackTarget, ProgressEvent};
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

impl AppError {
    fn new(code: &str, message: impl std::fmt::Display) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
            details: None,
        }
    }
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
        };
        AppError::new(code, e)
    }
}

impl From<github::GithubError> for AppError {
    fn from(e: github::GithubError) -> Self {
        AppError::new(e.code(), e)
    }
}

pub type CmdResult<T> = Result<T, AppError>;

/// Map a tokio JoinError into AppError. JoinError happens when a
/// spawn_blocking panics — rare but worth surfacing rather than swallowing.
fn join_error_to_app_error(e: tokio::task::JoinError) -> AppError {
    AppError::new("internal", format!("join error: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn pack_start(
    app: AppHandle,
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

    // For a GitHub target, look up the stored PAT (user-only 0600 file
    // under the app-data dir — see the `github` module docs) so the clone
    // can pull private repos. We do this synchronously here (a small file
    // read) and pass the token down into `pack` as a runtime parameter —
    // never serialized into PackOptions, never reachable from JS.
    let github_token = if matches!(opts.target, PackTarget::GitHub(_)) {
        token_dir(&app)
            .ok()
            .and_then(|dir| github::read_token(&dir).ok().flatten())
    } else {
        None
    };

    log::info!(
        "pack_start: job={} compress={} remove_comments={} secret_scan={} count_tokens={} respect_gitignore={} format={:?} target={:?}",
        job_id,
        opts.compress,
        opts.remove_comments,
        opts.secret_scan,
        opts.count_tokens,
        opts.respect_gitignore,
        opts.format,
        opts.target
    );

    tokio::task::spawn_blocking(move || {
        // Two clones of the channel:
        //   - `tx` is moved into pack() for progress events
        //   - `tx_for_terminal` is kept here for the terminal event we
        //     emit ourselves after the spawn_blocking task has stashed
        //     the result (or discarded the token on error).
        let tx_for_terminal = tx.clone();
        let pack_start_time = std::time::Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pack::pack(
                &opts.target,
                &opts,
                tx,
                &id_for_task,
                cancel_for_task,
                github_token.as_deref(),
            )
        }));
        let elapsed = pack_start_time.elapsed();
        match result {
            Ok(Ok(result)) => {
                log::info!(
                    "pack_start: job={} pack() Ok in {:?}, files={} bytes={}",
                    id_for_task,
                    elapsed,
                    result.stats.files_included,
                    result.stats.bytes_total
                );
                // Stash the result FIRST, then emit Done with a clone of
                // its stats. Doing it in this order means a fast renderer
                // that calls `pack_get_result` immediately on receipt of
                // Done is guaranteed to find the result in the registry —
                // the previous "Done emitted from inside pack()" path had
                // a race where the result wasn't stored yet, and the
                // renderer saw "no result for job …".
                let stats = result.stats.clone();
                registry_for_task.store_result(&id_for_task, result);
                let _ = tx_for_terminal.send(ProgressEvent::Done { stats });
            }
            Ok(Err(e)) => {
                log::error!(
                    "pack_start: job={} pack() Err in {:?}: {e}",
                    id_for_task,
                    elapsed
                );
                registry_for_task.discard(&id_for_task);
                let _ = tx_for_terminal.send(ProgressEvent::Error {
                    message: e.to_string(),
                    fatal: true,
                });
            }
            Err(panic) => {
                // A panic inside pack() (or any code it called) used to
                // be silent — the spawn_blocking task simply dies, the
                // renderer never gets a Done or Error, and the UI sits
                // on "Packing…" forever. Catch + surface as a fatal
                // Error event so the user sees something actionable.
                let msg = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "<unknown panic payload>".into());
                log::error!(
                    "pack_start: job={} pack() PANIC in {:?}: {msg}",
                    id_for_task,
                    elapsed
                );
                registry_for_task.discard(&id_for_task);
                let _ = tx_for_terminal.send(ProgressEvent::Error {
                    message: format!("internal panic: {msg}"),
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
    registry
        .inner()
        .take_result(&job_id)
        .ok_or_else(|| AppError::new("result_not_ready", format!("no result for job {job_id}")))
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
    let path = settings_path(&app);
    tokio::task::spawn_blocking(move || load_or_default(&path))
        .await
        .map_err(join_error_to_app_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<Settings> {
    let path = settings_path(&app);
    tokio::task::spawn_blocking(move || save(&path, &settings).map(|()| settings))
        .await
        .map_err(join_error_to_app_error)?
        .map_err(|e| AppError::new("settings_save_failed", e))
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
    let chosen = rx.await.map_err(|e| AppError::new("dialog_failed", e))?;
    let Some(file_path) = chosen else {
        return Ok(None);
    };
    let path: PathBuf = file_path
        .into_path()
        .map_err(|e| AppError::new("invalid_path", e))?;
    // Pack output can be tens of MB — keep the write off the async runtime.
    let path = tokio::task::spawn_blocking(move || std::fs::write(&path, contents).map(|()| path))
        .await
        .map_err(join_error_to_app_error)?
        .map_err(|e| AppError::new("save_failed", e))?;
    Ok(Some(path.display().to_string()))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    match app.path().app_data_dir() {
        Ok(dir) => dir.join("settings.json"),
        Err(e) => {
            // Distinctive filename so a temp-dir fallback can't be mistaken
            // for (or collide with) anything else named settings.json.
            let fallback = std::env::temp_dir().join("projectpacker-settings-fallback.json");
            log::warn!(
                "app_data_dir unavailable ({e}); falling back to {}",
                fallback.display()
            );
            fallback
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// GitHub commands
//
// The PAT lives in a user-only (0600) file under the app-data dir — see
// the `github` module docs for why the OS keychain was abandoned. The
// renderer never receives it: status returns a bool, set/clear take/return
// nothing, and the user/repo fetches read the token in-process and only
// return the API response.
// ─────────────────────────────────────────────────────────────────────────

/// Resolve the app-data dir that holds the token file. Uses the same Tauri
/// path resolver as `settings_path` so there is exactly one notion of
/// "our app-data dir" in the codebase.
fn token_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    app.path().app_data_dir().map_err(|e| {
        github::GithubError::Storage(format!("could not resolve app data dir: {e}")).into()
    })
}

/// Read the token on a blocking pool — file-system reads are normally
/// fast enough to call inline, but routing through `spawn_blocking` keeps
/// the async runtime predictable if the disk is busy.
async fn read_token_or_err(app: &AppHandle) -> CmdResult<String> {
    let dir = token_dir(app)?;
    let token = tokio::task::spawn_blocking(move || github::read_token(&dir))
        .await
        .map_err(join_error_to_app_error)?
        .map_err(AppError::from)?;
    token.ok_or_else(|| AppError::from(github::GithubError::NoToken))
}

#[tauri::command]
#[specta::specta]
pub async fn github_set_token(app: AppHandle, token: String) -> CmdResult<()> {
    // Log the length only — the token itself never reaches a log line.
    log::info!("github_set_token called, token_len={}", token.len());
    let dir = token_dir(&app)?;
    let result = tokio::task::spawn_blocking(move || github::store_token(&dir, &token))
        .await
        .map_err(join_error_to_app_error)?
        .map_err(AppError::from);
    match &result {
        Ok(_) => log::info!("github_set_token: stored successfully"),
        Err(e) => log::error!("github_set_token failed: code={} msg={}", e.code, e.message),
    }
    result
}

#[tauri::command]
#[specta::specta]
pub async fn github_clear_token(app: AppHandle) -> CmdResult<()> {
    log::info!("github_clear_token called");
    let dir = token_dir(&app)?;
    let result = tokio::task::spawn_blocking(move || github::clear_token(&dir))
        .await
        .map_err(join_error_to_app_error)?
        .map_err(AppError::from);
    match &result {
        Ok(_) => log::info!("github_clear_token: cleared"),
        Err(e) => log::error!(
            "github_clear_token failed: code={} msg={}",
            e.code,
            e.message
        ),
    }
    result
}

#[tauri::command]
#[specta::specta]
pub async fn github_token_status(app: AppHandle) -> CmdResult<bool> {
    // has_token swallows storage errors — if the token file is unreadable,
    // we treat that as "no token" so the UI shows the connect prompt
    // rather than a scary error. The actual API calls below will
    // surface a real error if storage is genuinely broken.
    let has = match token_dir(&app) {
        Ok(dir) => tokio::task::spawn_blocking(move || github::has_token(&dir))
            .await
            .unwrap_or(false),
        Err(_) => false,
    };
    log::debug!("github_token_status: has_token={has}");
    Ok(has)
}

#[tauri::command]
#[specta::specta]
pub async fn github_get_user(app: AppHandle) -> CmdResult<github::GithubUser> {
    log::info!("github_get_user called");
    let token = read_token_or_err(&app).await?;
    let user = github::fetch_user(&token).await.map_err(|e| {
        log::error!("github_get_user failed: code={} msg={}", e.code(), e);
        e
    })?;
    log::info!("github_get_user ok: login={}", user.login);
    Ok(user)
}

#[tauri::command]
#[specta::specta]
pub async fn github_list_repos(app: AppHandle) -> CmdResult<Vec<github::GithubRepo>> {
    log::info!("github_list_repos called");
    let token = read_token_or_err(&app).await?;
    let repos = github::fetch_all_repos(&token).await.map_err(|e| {
        log::error!("github_list_repos failed: code={} msg={}", e.code(), e);
        e
    })?;
    log::info!("github_list_repos ok: count={}", repos.len());
    Ok(repos)
}
