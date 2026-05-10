//! GitHub integration: PAT storage on disk + REST client.
//!
//! ## Trust boundary
//!
//! The Personal Access Token never leaves the Rust process. The renderer
//! calls thin Tauri commands that take/return `bool` for "do we have a
//! token?", or domain types (`GithubUser`, `GithubRepo`).
//!
//! ## Storage
//!
//! Plain file at `<app_data_dir>/github-token` with `0600` permissions on
//! Unix and the per-user app-data ACL on Windows. We initially tried the
//! OS keychain via `keyring 3.x`, but the macOS data-protection keychain
//! silently demotes unsigned-app writes into a transient store — the
//! Rust call returns success while `security find-generic-password` (and
//! every other process) reports the entry doesn't exist. Code-signing
//! would fix that, but for a personal dev tool the 0600-file approach is
//! the more honest contract: same single-user trust boundary in practice,
//! and it actually persists.
//!
//! ## HTTP
//!
//! `reqwest` with `rustls-tls` (no OpenSSL dependency). All request errors
//! pass through `scrub_token` so the PAT never reaches a log line, error
//! banner, or `Display` impl downstream.
//!
//! ## Auth scope
//!
//! For private repos: GitHub PAT classic with `repo` scope, or a
//! fine-grained PAT with the user-selected repos and `Contents:read`
//! permission. We don't validate scopes proactively — GitHub rejects
//! the request with 403 and we surface that.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Sub-directory under the platform's app-data dir where ProjectPacker
/// keeps its state. Mirrors Tauri's `app_data_dir` for the same
/// `identifier` in `tauri.conf.json`.
const APP_DATA_SUBDIR: &str = "dev.cbailey.projectpacker";
const TOKEN_FILE: &str = "github-token";

/// User-Agent string sent on every GitHub API call. GitHub requires one.
const UA: &str = concat!("ProjectPacker/", env!("CARGO_PKG_VERSION"));

// ─────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("no token stored")]
    NoToken,
    #[error("token rejected: invalid format (expected ghp_, github_pat_, or ghs_ prefix)")]
    InvalidTokenFormat,
    #[error("storage: {0}")]
    Storage(String),
    #[error("network: {0}")]
    Network(String),
    #[error("github api {status}: {body}")]
    Api { status: u16, body: String },
    #[error("github api rate limit (resets at unix {reset})")]
    RateLimit { reset: u64 },
    #[error("github api forbidden: {0}")]
    Forbidden(String),
    #[error("github api unauthorized: token rejected")]
    Unauthorized,
}

impl GithubError {
    /// Map a `GithubError` to the public `code` string used by the
    /// frontend's error display so we get stable, scrutable codes.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoToken => "github_no_token",
            Self::InvalidTokenFormat => "github_invalid_format",
            Self::Storage(_) => "github_storage",
            Self::Network(_) => "github_network",
            Self::Api { .. } => "github_api",
            Self::RateLimit { .. } => "github_rate_limit",
            Self::Forbidden(_) => "github_forbidden",
            Self::Unauthorized => "github_unauthorized",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Token validation + scrubbing
// ─────────────────────────────────────────────────────────────────────────

/// Reject obviously-bad tokens before storing. Three known live formats:
///   - classic:      `ghp_` + 36 alphanumerics
///   - fine-grained: `github_pat_` + at least 70 chars (length varies)
///   - app/install:  `ghs_` + 36 alphanumerics
///
/// We use a permissive length check so future expansions don't break us;
/// the auth check (call to /user) is the real validator.
pub fn validate_token_format(token: &str) -> Result<(), GithubError> {
    let t = token.trim();
    if t.is_empty() {
        return Err(GithubError::InvalidTokenFormat);
    }
    let ok = (t.starts_with("ghp_") && t.len() >= 40)
        || (t.starts_with("ghs_") && t.len() >= 40)
        || (t.starts_with("github_pat_") && t.len() >= 80);
    if !ok {
        return Err(GithubError::InvalidTokenFormat);
    }
    // Reject whitespace anywhere — copy-paste from GH UI strips a trailing
    // newline most of the time, but not always.
    if t.chars().any(|c| c.is_whitespace()) {
        return Err(GithubError::InvalidTokenFormat);
    }
    Ok(())
}

/// Replace the token value with `<redacted>` in any string. Used on every
/// error path that might surface upstream — clone errors, API responses,
/// header dumps — so the PAT never reaches a log/UI/event.
pub fn scrub_token(s: &str, token: Option<&str>) -> String {
    match token {
        Some(t) if !t.is_empty() => s.replace(t, "<redacted>"),
        _ => s.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// File storage
// ─────────────────────────────────────────────────────────────────────────

/// Resolve the absolute path to the token file, creating any missing
/// parent directories as a side effect. The path mirrors Tauri's
/// `app_data_dir` for the same bundle identifier:
///   - macOS: `~/Library/Application Support/dev.cbailey.projectpacker/github-token`
///   - Windows: `%APPDATA%\dev.cbailey.projectpacker\github-token`
fn token_path() -> Result<PathBuf, GithubError> {
    let base = dirs::data_dir()
        .ok_or_else(|| GithubError::Storage("could not resolve OS data dir".into()))?;
    let dir = base.join(APP_DATA_SUBDIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| GithubError::Storage(format!("create_dir_all: {e}")))?;
    Ok(dir.join(TOKEN_FILE))
}

/// Write the token to disk atomically (`tmp + rename`) and lock down
/// permissions to user-only. The 0600 mode on Unix and per-user app-data
/// ACL on Windows together approximate "only this user can read it".
pub fn store_token(token: &str) -> Result<(), GithubError> {
    let trimmed = token.trim();
    let prefix: String = trimmed.chars().take(4).collect();
    log::info!(
        "store_token: validating prefix='{prefix}', len={}",
        trimmed.len()
    );
    validate_token_format(trimmed).inspect_err(|e| {
        log::error!("store_token: format validation failed: {e}");
    })?;

    let path = token_path()?;
    let tmp = path.with_extension("tmp");
    log::info!("store_token: writing to {}", path.display());

    // On Unix, set 0600 at create-time so the token is never visible to
    // other users even briefly. Windows inherits ACL from the parent
    // (per-user AppData) which is already user-only.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| GithubError::Storage(format!("open tmp: {e}")))?;
        f.write_all(trimmed.as_bytes())
            .map_err(|e| GithubError::Storage(format!("write tmp: {e}")))?;
        f.sync_all().ok();
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&tmp, trimmed.as_bytes())
            .map_err(|e| GithubError::Storage(format!("write tmp: {e}")))?;
    }

    std::fs::rename(&tmp, &path)
        .map_err(|e| GithubError::Storage(format!("rename: {e}")))?;
    log::info!("store_token: write succeeded");
    Ok(())
}

/// Read the stored PAT. Returns `Ok(None)` when the file doesn't exist
/// (the normal "not connected" state). Trims whitespace defensively in
/// case the file ever gains a trailing newline.
pub fn read_token() -> Result<Option<String>, GithubError> {
    let path = match token_path() {
        Ok(p) => p,
        Err(e) => {
            log::error!("read_token: cannot resolve path: {e}");
            return Err(e);
        }
    };
    log::debug!("read_token: reading {}", path.display());
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                log::debug!("read_token: file present but empty");
                Ok(None)
            } else {
                log::debug!("read_token: got token, len={}", trimmed.len());
                Ok(Some(trimmed))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::debug!("read_token: file not found");
            Ok(None)
        }
        Err(e) => {
            log::error!("read_token: io error: {e}");
            Err(GithubError::Storage(e.to_string()))
        }
    }
}

/// Delete the stored PAT. Idempotent — a missing file is treated as
/// success.
pub fn clear_token() -> Result<(), GithubError> {
    let path = token_path()?;
    match std::fs::remove_file(&path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(GithubError::Storage(e.to_string())),
    }
}

/// Cheap check used by the renderer's status hook so the GitHub tab can
/// show the empty state vs. fetching repos without round-tripping the
/// token itself.
pub fn has_token() -> bool {
    matches!(read_token(), Ok(Some(_)))
}

// ─────────────────────────────────────────────────────────────────────────
// Wire types — exposed to TypeScript via tauri-specta
// ─────────────────────────────────────────────────────────────────────────

// Each `#[serde(alias = "snake_case_form")]` lets serde accept GitHub's
// snake_case JSON during *deserialization* while `rename_all = "camelCase"`
// continues to govern *serialization* into the TypeScript wire format.
// Without these aliases, serde looks for `avatarUrl`/`htmlUrl`/etc. in
// the GitHub response, never finds them, and reports "error decoding
// response body" — even with a valid token and a 200 OK status.

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GithubUser {
    pub login: String,
    pub name: Option<String>,
    #[serde(alias = "avatar_url")]
    pub avatar_url: String,
    #[serde(alias = "html_url")]
    pub html_url: String,
    #[serde(alias = "public_repos")]
    pub public_repos: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepoOwner {
    pub login: String,
    #[serde(alias = "avatar_url")]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepo {
    pub id: u64,
    pub name: String,
    #[serde(alias = "full_name")]
    pub full_name: String,
    pub description: Option<String>,
    #[serde(alias = "html_url")]
    pub html_url: String,
    pub private: bool,
    pub fork: bool,
    pub archived: bool,
    pub language: Option<String>,
    #[serde(alias = "stargazers_count")]
    pub stargazers_count: u32,
    #[serde(alias = "pushed_at")]
    pub pushed_at: String,
    #[serde(alias = "default_branch")]
    pub default_branch: String,
    pub owner: GithubRepoOwner,
}

// ─────────────────────────────────────────────────────────────────────────
// HTTP client (lazy, shared)
// ─────────────────────────────────────────────────────────────────────────

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(UA)
            // Two timeouts: connect for the TCP/TLS handshake, request
            // for the entire round-trip. Without `connect_timeout` a
            // stalled DNS or unreachable host can hang past the request
            // timeout — `connect_timeout` ensures we surface a network
            // error within 10 seconds even if DNS is broken.
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            // builder() failure here means TLS init failed at startup —
            // unrecoverable, so a panic at the very first call is fine.
            .expect("reqwest client init")
    })
}

/// Translate a non-2xx GitHub response into a typed `GithubError` with
/// 401/403/rate-limit pulled apart so the UI can show the right hint.
async fn map_response_error(res: reqwest::Response) -> GithubError {
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.text().await.unwrap_or_default();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return GithubError::Unauthorized;
    }

    if status == reqwest::StatusCode::FORBIDDEN {
        // GitHub uses 403 for both auth-failure and rate-limit. The
        // `X-RateLimit-Remaining: 0` header (or `Retry-After`) is the
        // reliable signal that this is a rate-limit, not a permission
        // problem.
        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok());
        if remaining == Some(0) {
            let reset = headers
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            return GithubError::RateLimit { reset };
        }
        return GithubError::Forbidden(body);
    }

    GithubError::Api {
        status: status.as_u16(),
        body: body.chars().take(500).collect(),
    }
}

fn auth_headers(token: &str) -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue};
    let mut h = HeaderMap::new();
    // The token itself is the only sensitive header; everything else is
    // public knowledge.
    if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
        h.insert(reqwest::header::AUTHORIZATION, v);
    }
    h.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    h.insert(
        reqwest::header::HeaderName::from_static("x-github-api-version"),
        HeaderValue::from_static("2022-11-28"),
    );
    h
}

/// Authenticated GET against the GitHub API. Used by both `fetch_user`
/// and the per-page `fetch_repos_page` below.
async fn gh_get<T: serde::de::DeserializeOwned>(
    path: &str,
    token: &str,
) -> Result<T, GithubError> {
    log::info!("gh_get: GET {path} starting");
    let res = client()
        .get(format!("https://api.github.com{path}"))
        .headers(auth_headers(token))
        .send()
        .await
        .map_err(|e| {
            let scrubbed = scrub_token(&e.to_string(), Some(token));
            log::error!("gh_get: send failed: {scrubbed}");
            GithubError::Network(scrubbed)
        })?;

    let status = res.status();
    log::info!("gh_get: GET {path} got status={}", status.as_u16());

    if !status.is_success() {
        return Err(map_response_error(res).await);
    }

    res.json::<T>()
        .await
        .map_err(|e| {
            let scrubbed = scrub_token(&e.to_string(), Some(token));
            log::error!("gh_get: json decode failed: {scrubbed}");
            GithubError::Network(scrubbed)
        })
}

/// `GET /user` — also serves as the cheapest auth-check call.
pub async fn fetch_user(token: &str) -> Result<GithubUser, GithubError> {
    gh_get::<GithubUser>("/user", token).await
}

/// `GET /user/repos` paginated. Sorts by latest push, pulls every page
/// until an empty body or until the safety cap (1000) is hit.
pub async fn fetch_all_repos(token: &str) -> Result<Vec<GithubRepo>, GithubError> {
    const PER_PAGE: u32 = 100;
    const MAX_PAGES: u32 = 10; // 1000 repos cap; well past any sane account
    let mut out = Vec::with_capacity(PER_PAGE as usize);
    for page in 1..=MAX_PAGES {
        let path = format!(
            "/user/repos?sort=pushed&per_page={PER_PAGE}&page={page}&affiliation=owner,collaborator"
        );
        let chunk: Vec<GithubRepo> = gh_get(&path, token).await?;
        let len = chunk.len();
        out.extend(chunk);
        if len < PER_PAGE as usize {
            break;
        }
    }
    Ok(out)
}
