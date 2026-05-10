//! Persistent application settings.
//!
//! Stream 8 hardening:
//! * `ValidationError` + `Settings::validate` / `Settings::apply_corrections`
//!   keep persisted JSON within sane bounds even if the file was hand-edited
//!   or written by an older buggy build.
//! * `SettingsMigration` + `MigrationRegistry` give us a place to land schema
//!   bumps without touching every call site.
//! * `load_sync_fallible` exposes the precise failure mode (missing vs. parse
//!   vs. IO) for callers that care; `load_or_default` is the lenient wrapper
//!   used by the Tauri commands.
//! * `save` is atomic (write-tmp → fsync → rename) so a crash mid-write can
//!   never leave a half-written settings.json.
//! * `save_async` ships the blocking IO onto a tokio worker.
//!
//! The on-disk schema is **additive only** — every new field carries
//! `#[serde(default)]` so older JSON deserializes cleanly.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Maximum byte length for `Settings::default_protocol_version` and other
/// protocol-version-shaped strings is implicit in the supported list, but
/// every other free-form field gets a hard ceiling so a runaway renderer
/// can't fill the disk via `save_settings`.
pub const MAX_GOAL_LEN: usize = 8192;
pub const MAX_PRESET_NAME_LEN: usize = 128;
pub const MAX_RECENT_LABEL_LEN: usize = 256;
pub const MAX_PATH_LEN: usize = 4096;
pub const MAX_CUSTOM_IGNORE_LEN: usize = 4096;

/// Protocol versions accepted by `default_protocol_version`. Mirrors the list
/// in `projectpacker_core::protocol`. Kept here as a const so settings
/// validation does not need to take a runtime dep on core's private list.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["grok-to-cc-v1"];

/// Schema version embedded in serialized settings. Bumped together with a
/// new `SettingsMigration` registration.
pub const CURRENT_SETTINGS_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Settings struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub default_protocol_version: String,
    pub default_tokenizer_model: String,
    pub recents: Vec<Recent>,
    pub goal_templates: Vec<GoalTemplate>,
    pub presets: Vec<Preset>,
    /// Stored schema version. Defaults to 0 so settings written before the
    /// migration framework existed are still readable.
    #[serde(default)]
    pub schema_version: u32,
}

impl Settings {
    pub fn defaults() -> Self {
        Self {
            theme: Theme::Dark,
            default_protocol_version: "grok-to-cc-v1".into(),
            default_tokenizer_model: "gpt-4o-mini".into(),
            recents: Vec::new(),
            goal_templates: Vec::new(),
            presets: Vec::new(),
            schema_version: CURRENT_SETTINGS_VERSION,
        }
    }

    /// Validate every field. Returns *all* errors so a UI can surface the
    /// full list rather than fixing one and re-validating in a loop.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errs = Vec::new();

        if self.default_protocol_version.is_empty() {
            errs.push(ValidationError::EmptyProtocol);
        } else if !SUPPORTED_PROTOCOL_VERSIONS.contains(&self.default_protocol_version.as_str()) {
            errs.push(ValidationError::UnsupportedProtocolVersion(
                self.default_protocol_version.clone(),
            ));
        }

        if self.default_tokenizer_model.is_empty() {
            errs.push(ValidationError::TokenizerModelEmpty);
        } else if !is_plausible_tokenizer_model(&self.default_tokenizer_model) {
            errs.push(ValidationError::InvalidTokenizerModel(
                self.default_tokenizer_model.clone(),
            ));
        }

        for r in &self.recents {
            if r.label.len() > MAX_RECENT_LABEL_LEN {
                errs.push(ValidationError::RecentLabelTooLong {
                    actual: r.label.len(),
                    max: MAX_RECENT_LABEL_LEN,
                });
            }
            if r.target.len() > MAX_PATH_LEN {
                errs.push(ValidationError::PathTooLong {
                    actual: r.target.len(),
                    max: MAX_PATH_LEN,
                });
            }
        }

        for t in &self.goal_templates {
            if t.name.len() > MAX_PRESET_NAME_LEN {
                errs.push(ValidationError::PresetNameTooLong {
                    actual: t.name.len(),
                    max: MAX_PRESET_NAME_LEN,
                });
            }
            if t.body.len() > MAX_GOAL_LEN {
                errs.push(ValidationError::GoalTooLong {
                    actual: t.body.len(),
                    max: MAX_GOAL_LEN,
                });
            }
        }

        for p in &self.presets {
            if p.name.len() > MAX_PRESET_NAME_LEN {
                errs.push(ValidationError::PresetNameTooLong {
                    actual: p.name.len(),
                    max: MAX_PRESET_NAME_LEN,
                });
            }
            // options_json blobs are bounded by the preset itself, but a
            // single embedded custom-ignore should not exceed MAX_PATH_LEN.
            if p.options_json.len() > MAX_CUSTOM_IGNORE_LEN * 64 {
                errs.push(ValidationError::CustomIgnoreTooLong {
                    actual: p.options_json.len(),
                    max: MAX_CUSTOM_IGNORE_LEN * 64,
                });
            }
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    /// Best-effort fix-up for fields that fail validation. Returns the list of
    /// corrections actually applied so the caller can log a recovery banner.
    ///
    /// Strategy: clamp lengths, and fall back to the default value for invalid
    /// enums/protocol versions. Never panics, never errors.
    pub fn apply_corrections(&mut self) -> Vec<ValidationError> {
        let mut corrections = Vec::new();
        let defaults = Settings::defaults();

        // Protocol version — fall back to the default if empty or unknown.
        if self.default_protocol_version.is_empty() {
            corrections.push(ValidationError::EmptyProtocol);
            self.default_protocol_version = defaults.default_protocol_version.clone();
        } else if !SUPPORTED_PROTOCOL_VERSIONS.contains(&self.default_protocol_version.as_str()) {
            corrections.push(ValidationError::UnsupportedProtocolVersion(
                self.default_protocol_version.clone(),
            ));
            self.default_protocol_version = defaults.default_protocol_version.clone();
        }

        // Tokenizer model.
        if self.default_tokenizer_model.is_empty() {
            corrections.push(ValidationError::TokenizerModelEmpty);
            self.default_tokenizer_model = defaults.default_tokenizer_model.clone();
        } else if !is_plausible_tokenizer_model(&self.default_tokenizer_model) {
            corrections.push(ValidationError::InvalidTokenizerModel(
                self.default_tokenizer_model.clone(),
            ));
            self.default_tokenizer_model = defaults.default_tokenizer_model.clone();
        }

        // Recents.
        for r in &mut self.recents {
            if r.label.len() > MAX_RECENT_LABEL_LEN {
                corrections.push(ValidationError::RecentLabelTooLong {
                    actual: r.label.len(),
                    max: MAX_RECENT_LABEL_LEN,
                });
                truncate_to(&mut r.label, MAX_RECENT_LABEL_LEN);
            }
            if r.target.len() > MAX_PATH_LEN {
                corrections.push(ValidationError::PathTooLong {
                    actual: r.target.len(),
                    max: MAX_PATH_LEN,
                });
                truncate_to(&mut r.target, MAX_PATH_LEN);
            }
        }

        // Goal templates.
        for t in &mut self.goal_templates {
            if t.name.len() > MAX_PRESET_NAME_LEN {
                corrections.push(ValidationError::PresetNameTooLong {
                    actual: t.name.len(),
                    max: MAX_PRESET_NAME_LEN,
                });
                truncate_to(&mut t.name, MAX_PRESET_NAME_LEN);
            }
            if t.body.len() > MAX_GOAL_LEN {
                corrections.push(ValidationError::GoalTooLong {
                    actual: t.body.len(),
                    max: MAX_GOAL_LEN,
                });
                truncate_to(&mut t.body, MAX_GOAL_LEN);
            }
        }

        // Presets.
        for p in &mut self.presets {
            if p.name.len() > MAX_PRESET_NAME_LEN {
                corrections.push(ValidationError::PresetNameTooLong {
                    actual: p.name.len(),
                    max: MAX_PRESET_NAME_LEN,
                });
                truncate_to(&mut p.name, MAX_PRESET_NAME_LEN);
            }
            let cap = MAX_CUSTOM_IGNORE_LEN * 64;
            if p.options_json.len() > cap {
                corrections.push(ValidationError::CustomIgnoreTooLong {
                    actual: p.options_json.len(),
                    max: cap,
                });
                truncate_to(&mut p.options_json, cap);
            }
        }

        corrections
    }
}

/// Truncate `s` to at most `max` bytes, taking care to land on a UTF-8
/// character boundary so the resulting `String` stays well-formed.
fn truncate_to(s: &mut String, max: usize) {
    if s.len() <= max {
        return;
    }
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
}

/// Tokenizer model strings should be a non-empty ASCII identifier-ish blob.
/// We don't bind to a fixed list — the underlying tokenizer crate evolves —
/// but obvious garbage (control chars, spaces, leading dashes) is rejected.
fn is_plausible_tokenizer_model(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    if name.starts_with('-') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

// ---------------------------------------------------------------------------
// Sub-types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub label: String,
    pub target: String,
    pub last_used_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GoalTemplate {
    pub name: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    pub options_json: String,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("default protocol version must not be empty")]
    EmptyProtocol,
    #[error("unsupported protocol version: {0}")]
    UnsupportedProtocolVersion(String),
    #[error("goal body too long: {actual} > {max}")]
    GoalTooLong { actual: usize, max: usize },
    #[error("preset name too long: {actual} > {max}")]
    PresetNameTooLong { actual: usize, max: usize },
    #[error("default tokenizer model must not be empty")]
    TokenizerModelEmpty,
    #[error("invalid tokenizer model: {0}")]
    InvalidTokenizerModel(String),
    #[error("recent label too long: {actual} > {max}")]
    RecentLabelTooLong { actual: usize, max: usize },
    #[error("path too long: {actual} > {max}")]
    PathTooLong { actual: usize, max: usize },
    #[error("custom-ignore pattern too long: {actual} > {max}")]
    CustomIgnoreTooLong { actual: usize, max: usize },
    #[error("invalid theme value: {0}")]
    InvalidThemeValue(String),
}

#[derive(Debug, Error)]
pub enum SettingsLoadError {
    #[error("settings file not found: {0}")]
    NotFound(PathBuf),
    #[error("settings file IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("settings file parse error: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum SettingsSaveError {
    #[error("settings file IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("settings serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("no migration registered from version {from} to current")]
    MissingMigration { from: u32 },
    #[error("migration {from}->{to} failed: {message}")]
    StepFailed {
        from: u32,
        to: u32,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Migration framework
// ---------------------------------------------------------------------------

pub trait SettingsMigration: Send + Sync {
    // `from_version` reads as a constructor name to clippy, but here it's
    // an accessor describing which schema version this migration upgrades
    // *from*. The spec calls for this exact name; the false-positive is
    // worth the readability.
    #[allow(clippy::wrong_self_convention)]
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn migrate(&self, value: serde_json::Value) -> Result<serde_json::Value, MigrationError>;
}

/// Registry of migrations applied in `from_version` order.
///
/// The registry contains zero entries today — the migration infrastructure
/// is here so the *next* schema bump is a one-liner instead of a refactor.
pub struct MigrationRegistry {
    migrations: Vec<Box<dyn SettingsMigration>>,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    pub fn register(&mut self, m: Box<dyn SettingsMigration>) -> &mut Self {
        self.migrations.push(m);
        self
    }

    pub fn len(&self) -> usize {
        self.migrations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.migrations.is_empty()
    }

    /// Apply every registered migration whose `from_version` is reachable
    /// from `from_version`, in order. If no migration starts at
    /// `from_version` and `from_version != CURRENT_SETTINGS_VERSION`, returns
    /// `MissingMigration`.
    pub fn migrate(
        &self,
        mut value: serde_json::Value,
        from_version: u32,
    ) -> Result<serde_json::Value, MigrationError> {
        let mut current = from_version;
        loop {
            if current == CURRENT_SETTINGS_VERSION {
                return Ok(value);
            }
            let Some(step) = self
                .migrations
                .iter()
                .find(|m| m.from_version() == current)
            else {
                if self.migrations.is_empty() {
                    // No migrations registered at all — accept the value as-is
                    // (Settings derives serde defaults so additive fields are
                    // tolerated without an explicit step).
                    return Ok(value);
                }
                return Err(MigrationError::MissingMigration { from: current });
            };
            let to = step.to_version();
            value = step.migrate(value).map_err(|e| match e {
                MigrationError::StepFailed { message, .. } => MigrationError::StepFailed {
                    from: current,
                    to,
                    message,
                },
                other => other,
            })?;
            current = to;
        }
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

/// Strict load: distinguishes missing file, IO error, and parse error.
pub fn load_sync_fallible(path: &Path) -> Result<Settings, SettingsLoadError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SettingsLoadError::NotFound(path.to_path_buf()))
        }
        Err(e) => return Err(SettingsLoadError::IoError(e)),
    };
    let s: Settings = serde_json::from_slice(&bytes)?;
    Ok(s)
}

/// Lenient load used by the Tauri commands.
///
/// * Missing file → defaults.
/// * Parse error → quarantine the file as `<path>.corrupt-<unix_ts>.json`
///   and return defaults so the app keeps booting.
/// * Successful parse → run `apply_corrections` so out-of-range values
///   silently get fixed instead of crashing the renderer later.
pub fn load_or_default(path: &Path) -> Settings {
    match load_sync_fallible(path) {
        Ok(mut s) => {
            let corrections = s.apply_corrections();
            if !corrections.is_empty() {
                log::warn!(
                    "settings: applied {} correction(s) on load: {:?}",
                    corrections.len(),
                    corrections
                );
            }
            s
        }
        Err(SettingsLoadError::NotFound(_)) => Settings::defaults(),
        Err(SettingsLoadError::ParseError(e)) => {
            log::warn!("settings: parse error, quarantining file: {e}");
            quarantine_corrupt(path);
            Settings::defaults()
        }
        Err(SettingsLoadError::IoError(e)) => {
            log::warn!("settings: IO error reading file: {e}");
            Settings::defaults()
        }
    }
}

fn quarantine_corrupt(path: &Path) {
    // `<filename>.corrupt-<unix_ts>.json` per the Stream 8 contract; we keep
    // a sibling on disk so the user can recover hand-edited content.
    let dest = path.with_extension(format!("corrupt-{}.json", unix_seconds()));
    if let Err(e) = std::fs::rename(path, &dest) {
        log::warn!(
            "settings: could not rename corrupt file to {}: {e}",
            dest.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------

/// Atomic save: write to `<path>.tmp`, fsync, then rename.
///
/// Same-volume rename is atomic on every OS we ship to, so a crash mid-write
/// leaves either the previous good file or the new complete file — never a
/// half-written or zero-byte settings.json.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");

    // Write + fsync the data file before rename, otherwise the rename can
    // commit before the bytes do on power-loss.
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        let _ = f.sync_all();
    }

    std::fs::rename(&tmp, path)
}

/// Async wrapper around `save` for callers on the tokio runtime.
pub fn save_async(
    path: PathBuf,
    settings: Settings,
) -> JoinHandle<Result<(), SettingsSaveError>> {
    tokio::task::spawn_blocking(move || save(&path, &settings).map_err(SettingsSaveError::from))
}

fn unix_seconds() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    // ---- defaults / round-trip --------------------------------------------

    #[test]
    fn defaults_round_trip_through_json() {
        let s = Settings::defaults();
        let j = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(back.theme, Theme::Dark);
        assert_eq!(back.default_tokenizer_model, "gpt-4o-mini");
    }

    #[test]
    fn legacy_json_without_schema_version_loads() {
        // Simulates JSON written by a pre-Stream-8 build.
        let raw = json!({
            "theme": "dark",
            "defaultProtocolVersion": "grok-to-cc-v1",
            "defaultTokenizerModel": "gpt-4o-mini",
            "recents": [],
            "goalTemplates": [],
            "presets": []
        });
        let s: Settings = serde_json::from_value(raw).unwrap();
        assert_eq!(s.schema_version, 0);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
    }

    // ---- validate ----------------------------------------------------------

    #[test]
    fn validate_accepts_defaults() {
        let s = Settings::defaults();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn validate_flags_empty_protocol() {
        let mut s = Settings::defaults();
        s.default_protocol_version.clear();
        let errs = s.validate().unwrap_err();
        assert!(errs.contains(&ValidationError::EmptyProtocol));
    }

    #[test]
    fn validate_flags_unsupported_protocol() {
        let mut s = Settings::defaults();
        s.default_protocol_version = "made-up-v9".into();
        let errs = s.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::UnsupportedProtocolVersion(v) if v == "made-up-v9")));
    }

    #[test]
    fn validate_flags_empty_tokenizer() {
        let mut s = Settings::defaults();
        s.default_tokenizer_model.clear();
        let errs = s.validate().unwrap_err();
        assert!(errs.contains(&ValidationError::TokenizerModelEmpty));
    }

    #[test]
    fn validate_flags_invalid_tokenizer() {
        let mut s = Settings::defaults();
        s.default_tokenizer_model = "has spaces".into();
        let errs = s.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidTokenizerModel(_))));
    }

    #[test]
    fn validate_flags_long_recent_label() {
        let mut s = Settings::defaults();
        s.recents.push(Recent {
            label: "x".repeat(MAX_RECENT_LABEL_LEN + 1),
            target: "/tmp/x".into(),
            last_used_iso: "now".into(),
        });
        let errs = s.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::RecentLabelTooLong { .. })));
    }

    #[test]
    fn validate_flags_long_path() {
        let mut s = Settings::defaults();
        s.recents.push(Recent {
            label: "ok".into(),
            target: "/".to_string() + &"a".repeat(MAX_PATH_LEN + 1),
            last_used_iso: "now".into(),
        });
        let errs = s.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::PathTooLong { .. })));
    }

    #[test]
    fn validate_flags_long_goal_body() {
        let mut s = Settings::defaults();
        s.goal_templates.push(GoalTemplate {
            name: "ok".into(),
            body: "x".repeat(MAX_GOAL_LEN + 1),
        });
        let errs = s.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::GoalTooLong { .. })));
    }

    #[test]
    fn validate_flags_long_preset_name() {
        let mut s = Settings::defaults();
        s.presets.push(Preset {
            name: "x".repeat(MAX_PRESET_NAME_LEN + 1),
            options_json: "{}".into(),
        });
        let errs = s.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::PresetNameTooLong { .. })));
    }

    #[test]
    fn validate_returns_all_errors_at_once() {
        let mut s = Settings::defaults();
        s.default_protocol_version.clear();
        s.default_tokenizer_model.clear();
        let errs = s.validate().unwrap_err();
        assert!(errs.len() >= 2);
    }

    // ---- apply_corrections -------------------------------------------------

    #[test]
    fn apply_corrections_clamps_lengths() {
        let mut s = Settings::defaults();
        s.recents.push(Recent {
            label: "x".repeat(MAX_RECENT_LABEL_LEN + 50),
            target: "/".to_string() + &"a".repeat(MAX_PATH_LEN + 50),
            last_used_iso: "now".into(),
        });
        let fixes = s.apply_corrections();
        assert!(s.recents[0].label.len() <= MAX_RECENT_LABEL_LEN);
        assert!(s.recents[0].target.len() <= MAX_PATH_LEN);
        assert!(fixes
            .iter()
            .any(|e| matches!(e, ValidationError::RecentLabelTooLong { .. })));
        assert!(fixes
            .iter()
            .any(|e| matches!(e, ValidationError::PathTooLong { .. })));
    }

    #[test]
    fn apply_corrections_falls_back_for_invalid_protocol() {
        let mut s = Settings::defaults();
        s.default_protocol_version = "bogus".into();
        let fixes = s.apply_corrections();
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
        assert!(fixes
            .iter()
            .any(|e| matches!(e, ValidationError::UnsupportedProtocolVersion(_))));
    }

    #[test]
    fn apply_corrections_reports_no_changes_for_valid_settings() {
        let mut s = Settings::defaults();
        let fixes = s.apply_corrections();
        assert!(fixes.is_empty());
        assert!(s.validate().is_ok());
    }

    #[test]
    fn apply_corrections_resets_empty_tokenizer() {
        let mut s = Settings::defaults();
        s.default_tokenizer_model.clear();
        let fixes = s.apply_corrections();
        assert_eq!(s.default_tokenizer_model, "gpt-4o-mini");
        assert!(fixes.contains(&ValidationError::TokenizerModelEmpty));
    }

    #[test]
    fn apply_corrections_truncates_at_char_boundary() {
        // Emoji is multi-byte; truncation must not split codepoints.
        let mut s = Settings::defaults();
        let body = "🦀".repeat(MAX_GOAL_LEN); // each crab is 4 bytes
        s.goal_templates.push(GoalTemplate {
            name: "long".into(),
            body,
        });
        let _ = s.apply_corrections();
        assert!(s.goal_templates[0].body.len() <= MAX_GOAL_LEN);
        // String must still be valid UTF-8 (`String` invariant) and not panic.
        let _ = s.goal_templates[0].body.as_str();
    }

    // ---- load_or_default --------------------------------------------------

    #[test]
    fn load_returns_default_when_missing() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        let s = load_or_default(&path);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
    }

    #[test]
    fn load_recovers_from_corrupt_file() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        std::fs::write(&path, "this is not json").unwrap();
        let s = load_or_default(&path);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
    }

    #[test]
    fn corrupt_file_is_renamed_to_quarantine() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        std::fs::write(&path, "{ invalid").unwrap();
        let _ = load_or_default(&path);

        // Original should be gone, a sibling .corrupt-*.json should exist.
        assert!(!path.exists(), "corrupt original must be moved aside");
        let entries: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            entries.iter().any(|n| n.contains(".corrupt-")),
            "expected a .corrupt-*.json file, got {entries:?}"
        );
    }

    #[test]
    fn load_applies_corrections_on_disk_data() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        // Hand-written settings file with an unknown protocol; load should
        // silently rewrite it via apply_corrections.
        std::fs::write(
            &path,
            r#"{
                "theme": "dark",
                "defaultProtocolVersion": "ancient-v0",
                "defaultTokenizerModel": "gpt-4o-mini",
                "recents": [],
                "goalTemplates": [],
                "presets": []
            }"#,
        )
        .unwrap();
        let s = load_or_default(&path);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
    }

    #[test]
    fn save_then_load_returns_same_data() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        let mut s = Settings::defaults();
        s.recents.push(Recent {
            label: "x".into(),
            target: "/tmp/x".into(),
            last_used_iso: "now".into(),
        });
        save(&path, &s).unwrap();
        let back = load_or_default(&path);
        assert_eq!(back.recents.len(), 1);
        assert_eq!(back.recents[0].label, "x");
    }

    // ---- save atomicity ---------------------------------------------------

    #[test]
    fn save_does_not_leave_tmp_file_behind() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        save(&path, &Settings::defaults()).unwrap();
        let tmp = path.with_extension("json.tmp");
        assert!(!tmp.exists(), "atomic save must clean up the .tmp file");
        assert!(path.exists(), "save must produce the destination file");
    }

    #[test]
    fn back_to_back_saves_do_not_corrupt() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        for i in 0..5 {
            let mut s = Settings::defaults();
            s.recents.push(Recent {
                label: format!("run-{i}"),
                target: format!("/tmp/{i}"),
                last_used_iso: "now".into(),
            });
            save(&path, &s).unwrap();
            let back = load_or_default(&path);
            assert_eq!(back.recents.len(), 1, "save iteration {i} dropped data");
            assert_eq!(back.recents[0].label, format!("run-{i}"));
        }
    }

    #[test]
    fn save_creates_parent_directory() {
        let d = tempdir().unwrap();
        let path = d.path().join("nested/dir/settings.json");
        save(&path, &Settings::defaults()).unwrap();
        assert!(path.exists());
    }

    // ---- save_async -------------------------------------------------------

    #[test]
    fn save_async_persists_via_blocking_pool() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        rt.block_on(async {
            let h = save_async(path.clone(), Settings::defaults());
            h.await.unwrap().unwrap();
        });
        assert!(path.exists());
    }

    // ---- load_sync_fallible -----------------------------------------------

    #[test]
    fn load_sync_fallible_success() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        save(&path, &Settings::defaults()).unwrap();
        let s = load_sync_fallible(&path).unwrap();
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
    }

    #[test]
    fn load_sync_fallible_missing() {
        let d = tempdir().unwrap();
        let path = d.path().join("nope.json");
        match load_sync_fallible(&path) {
            Err(SettingsLoadError::NotFound(p)) => assert_eq!(p, path),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn load_sync_fallible_parse_error() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        match load_sync_fallible(&path) {
            Err(SettingsLoadError::ParseError(_)) => {}
            other => panic!("expected ParseError, got {other:?}"),
        }
    }

    // ---- migration framework ----------------------------------------------

    struct NoopMigration {
        from: u32,
        to: u32,
    }

    impl SettingsMigration for NoopMigration {
        fn from_version(&self) -> u32 {
            self.from
        }
        fn to_version(&self) -> u32 {
            self.to
        }
        fn migrate(
            &self,
            value: serde_json::Value,
        ) -> Result<serde_json::Value, MigrationError> {
            Ok(value)
        }
    }

    struct BumpVersionMigration {
        from: u32,
        to: u32,
    }

    impl SettingsMigration for BumpVersionMigration {
        fn from_version(&self) -> u32 {
            self.from
        }
        fn to_version(&self) -> u32 {
            self.to
        }
        fn migrate(
            &self,
            mut value: serde_json::Value,
        ) -> Result<serde_json::Value, MigrationError> {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("schemaVersion".into(), json!(self.to));
            }
            Ok(value)
        }
    }

    #[test]
    fn migration_registry_register_and_lookup() {
        let mut reg = MigrationRegistry::new();
        assert!(reg.is_empty());
        reg.register(Box::new(NoopMigration { from: 0, to: 1 }));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn migration_registry_passthrough_when_empty() {
        // No migrations registered: any input value comes back unchanged
        // (additive serde defaults handle the schema drift).
        let reg = MigrationRegistry::new();
        let v = json!({ "schemaVersion": 0, "theme": "dark" });
        let out = reg.migrate(v.clone(), 0).unwrap();
        assert_eq!(out, v);
    }

    #[test]
    fn migration_registry_already_current_is_passthrough() {
        let mut reg = MigrationRegistry::new();
        reg.register(Box::new(NoopMigration { from: 0, to: 1 }));
        let v = json!({ "schemaVersion": CURRENT_SETTINGS_VERSION });
        let out = reg.migrate(v.clone(), CURRENT_SETTINGS_VERSION).unwrap();
        assert_eq!(out, v);
    }

    #[test]
    fn migration_registry_runs_step() {
        let mut reg = MigrationRegistry::new();
        reg.register(Box::new(BumpVersionMigration { from: 0, to: 1 }));
        let v = json!({ "schemaVersion": 0 });
        let out = reg.migrate(v, 0).unwrap();
        assert_eq!(out["schemaVersion"], json!(1));
    }

    #[test]
    fn migration_registry_missing_migration_errors() {
        let mut reg = MigrationRegistry::new();
        // Registry has *some* migration but none that starts at version 7.
        reg.register(Box::new(NoopMigration { from: 0, to: 1 }));
        let v = json!({});
        match reg.migrate(v, 7) {
            Err(MigrationError::MissingMigration { from }) => assert_eq!(from, 7),
            other => panic!("expected MissingMigration, got {other:?}"),
        }
    }
}
