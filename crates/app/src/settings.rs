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
        let bad = path.with_extension(format!("json.bad-{}", chrono_isoish_now()));
        let _ = std::fs::rename(path, bad);
    }
    Settings::defaults()
}

pub fn save(path: &PathBuf, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings).unwrap();
    std::fs::write(path, json)
}

fn chrono_isoish_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{secs}")
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
        s.recents.push(Recent { label: "x".into(), target: "/tmp/x".into(), last_used_iso: "now".into() });
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
        assert!(!path.exists() || std::fs::read_to_string(&path).unwrap().contains("\"theme\""));
    }
}
