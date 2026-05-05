use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub default_protocol_version: String,
    pub default_tokenizer_model: String,
    pub recents: Vec<Recent>,
    pub goal_templates: Vec<GoalTemplate>,
    pub presets: Vec<Preset>,
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
        }
    }
}

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

pub fn load_or_default(path: &PathBuf) -> Settings {
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(s) = serde_json::from_str::<Settings>(&text) {
            return s;
        }
        // Quarantine the corrupt file so it doesn't keep tripping us. Use a
        // nanosecond-resolution suffix so two concurrent recoveries can't
        // collide on the same destination filename (Windows rename to an
        // existing path fails silently).
        let bad = path.with_extension(format!("json.bad-{}", unix_nanos_string()));
        let _ = std::fs::rename(path, bad);
    }
    Settings::defaults()
}

/// Save settings atomically: write to `<path>.tmp` then rename over `<path>`.
/// On both NTFS and POSIX, same-volume rename is atomic, so a power-loss
/// or crash mid-write leaves either the previous good file intact or the
/// new complete file — never a half-written/zero-byte settings.json.
pub fn save(path: &PathBuf, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

fn unix_nanos_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_round_trip_through_json() {
        let s = Settings::defaults();
        let j = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(back.theme, Theme::Dark);
        assert_eq!(back.default_tokenizer_model, "gpt-4o-mini");
    }

    #[test]
    fn load_returns_default_when_missing() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
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

    #[test]
    fn load_recovers_from_corrupt_file() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        std::fs::write(&path, "this is not json").unwrap();
        let s = load_or_default(&path);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
        assert!(
            !path.exists()
                || std::fs::read_to_string(&path)
                    .unwrap()
                    .contains("\"theme\"")
        );
    }

    #[test]
    fn save_does_not_leave_tmp_file_behind() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        let s = Settings::defaults();
        save(&path, &s).unwrap();
        let tmp = path.with_extension("json.tmp");
        assert!(
            !tmp.exists(),
            "atomic save must clean up the .tmp file via rename"
        );
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
}
