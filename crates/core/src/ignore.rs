use ::ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

const BUILTIN_DEFAULTS: &str = include_str!("ignore_defaults.txt");

/// Three-tier ignore stack:
///
/// Tier 1 — Builtin defaults (always active, lowest priority).
///   Source: embedded `ignore_defaults.txt`.
///
/// Tier 2 — Project-level (only when `respect_gitignore` is true).
///   Source: `<root>/.gitignore` and `<root>/.git/info/exclude`.
///
/// Tier 3 — User-level (always active when any user input exists).
///   Source: `<root>/.repomixignore` (if present) merged with
///            the caller-supplied `custom_patterns` slice.
///   `.repomixignore` lines come first so custom_patterns can override them.
pub struct IgnoreMatcher {
    builtin: Gitignore,
    project: Option<Gitignore>,
    custom: Option<Gitignore>,
}

impl IgnoreMatcher {
    pub fn new(project_root: &Path, custom_patterns: &[String], respect_gitignore: bool) -> Self {
        let builtin = build_builtin_tier();

        let project = if respect_gitignore {
            Some(build_project_tier(project_root))
        } else {
            None
        };

        let custom = build_user_tier(project_root, custom_patterns);

        Self {
            builtin,
            project,
            custom,
        }
    }

    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let m = self.builtin.matched_path_or_any_parents(path, is_dir);
        if m.is_ignore() {
            return true;
        }

        if let Some(p) = &self.project {
            let m = p.matched_path_or_any_parents(path, is_dir);
            if m.is_ignore() {
                return true;
            }
            if m.is_whitelist() {
                return false;
            }
        }

        if let Some(c) = &self.custom {
            let m = c.matched_path_or_any_parents(path, is_dir);
            if m.is_ignore() {
                return true;
            }
        }

        false
    }
}

/// Tier 1: compile the embedded builtin defaults.
fn build_builtin_tier() -> Gitignore {
    build_from_lines(BUILTIN_DEFAULTS.lines(), Path::new(""))
}

/// Tier 2: project-level gitignore files.
/// Reads `.gitignore` and `.git/info/exclude`; tolerates either being absent.
fn build_project_tier(root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    let _ = b.add(root.join(".gitignore"));
    let _ = b.add(root.join(".git").join("info").join("exclude"));
    b.build().unwrap_or_else(|_| Gitignore::empty())
}

/// Tier 3: user-level patterns.
/// Merges `.repomixignore` (if present) with caller-supplied `custom_patterns`.
/// `.repomixignore` lines come first so `custom_patterns` can override them.
fn build_user_tier(root: &Path, custom_patterns: &[String]) -> Option<Gitignore> {
    let repomix_path = root.join(".repomixignore");
    let repomix_lines: Vec<String> = std::fs::read_to_string(&repomix_path)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect();

    let has_repomix = !repomix_lines.is_empty();
    let has_custom = !custom_patterns.is_empty();

    if !has_repomix && !has_custom {
        return None;
    }

    let all_lines = repomix_lines
        .iter()
        .map(String::as_str)
        .chain(custom_patterns.iter().map(String::as_str));

    Some(build_from_lines(all_lines, Path::new("")))
}

/// Low-level helper: compile an iterator of pattern lines into a `Gitignore`.
/// Skips blank lines and comments (lines starting with `#`).
fn build_from_lines<'a>(lines: impl IntoIterator<Item = &'a str>, root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let _ = b.add_line(None, line);
    }
    b.build().expect("ignore: pattern compile failure")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn root(p: &Path) -> IgnoreMatcher {
        IgnoreMatcher::new(p, &[], true)
    }

    // ── existing tests ─────────────────────────────────────────────────────────

    #[test]
    fn builtin_ignores_node_modules() {
        let m = root(Path::new("/tmp/empty"));
        assert!(m.is_ignored(Path::new("node_modules/foo.js"), false));
        assert!(m.is_ignored(Path::new("node_modules"), true));
    }

    #[test]
    fn builtin_ignores_lockfiles() {
        let m = root(Path::new("/tmp/empty"));
        assert!(m.is_ignored(Path::new("package-lock.json"), false));
        assert!(m.is_ignored(Path::new("Cargo.lock"), false));
        assert!(m.is_ignored(Path::new("pnpm-lock.yaml"), false));
    }

    #[test]
    fn does_not_ignore_arbitrary_source_files() {
        let m = root(Path::new("/tmp/empty"));
        assert!(!m.is_ignored(Path::new("src/main.rs"), false));
        assert!(!m.is_ignored(Path::new("README.md"), false));
    }

    #[test]
    fn project_gitignore_takes_effect() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "secret/\n*.bak\n").unwrap();
        let m = root(dir.path());
        assert!(m.is_ignored(Path::new("secret/x.txt"), false));
        assert!(m.is_ignored(Path::new("foo.bak"), false));
        assert!(!m.is_ignored(Path::new("foo.txt"), false));
    }

    #[test]
    fn custom_patterns_layer_on_top() {
        let m = IgnoreMatcher::new(Path::new("/tmp/empty"), &["docs/private/".into()], false);
        assert!(m.is_ignored(Path::new("docs/private/secret.md"), false));
    }

    #[test]
    fn respect_gitignore_false_disables_project_rules() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "src/\n").unwrap();
        let m = IgnoreMatcher::new(dir.path(), &[], false);
        assert!(!m.is_ignored(Path::new("src/main.rs"), false));
    }

    // ── new tests (Task B) ─────────────────────────────────────────────────────

    /// Tier 2: `.git/info/exclude` must be consulted alongside `.gitignore`.
    #[test]
    fn git_info_exclude_takes_effect() {
        let dir = tempdir().unwrap();
        // Create a minimal .git/info/exclude structure.
        let git_info = dir.path().join(".git").join("info");
        std::fs::create_dir_all(&git_info).unwrap();
        std::fs::write(git_info.join("exclude"), "secret/\n").unwrap();

        let m = root(dir.path());
        assert!(
            m.is_ignored(Path::new("secret/credentials.txt"), false),
            "path under secret/ should be ignored via .git/info/exclude"
        );
        assert!(
            m.is_ignored(Path::new("secret"), true),
            "secret/ directory itself should be ignored via .git/info/exclude"
        );
        assert!(
            !m.is_ignored(Path::new("src/main.rs"), false),
            "unrelated file must not be ignored"
        );
    }

    /// Tier 3: `.repomixignore` applies even when `respect_gitignore = false`,
    /// proving it is user-tier, not project-tier.
    #[test]
    fn repomixignore_takes_effect_as_user_layer() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".repomixignore"), "*.bak\n").unwrap();

        // Crucially: respect_gitignore = false — project tier is disabled.
        let m = IgnoreMatcher::new(dir.path(), &[], false);
        assert!(
            m.is_ignored(Path::new("foo.bak"), false),
            "foo.bak must be ignored via .repomixignore even with respect_gitignore=false"
        );
        assert!(
            !m.is_ignored(Path::new("foo.txt"), false),
            "foo.txt must not be ignored"
        );
    }

    /// Tier 3: `.repomixignore` and `custom_patterns` both contribute to the
    /// user-level matcher; patterns from both sources must fire.
    #[test]
    fn repomixignore_and_custom_patterns_combine() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".repomixignore"), "dist/\n").unwrap();

        let m = IgnoreMatcher::new(dir.path(), &["build/".into()], false);
        assert!(
            m.is_ignored(Path::new("dist/bundle.js"), false),
            "dist/bundle.js must be ignored via .repomixignore"
        );
        assert!(
            m.is_ignored(Path::new("build/output.js"), false),
            "build/output.js must be ignored via custom_patterns"
        );
        assert!(
            !m.is_ignored(Path::new("src/main.js"), false),
            "src/main.js must not be ignored"
        );
    }

    /// Legacy `.codeparserignore` must no longer be consulted.
    #[test]
    fn codeparserignore_no_longer_consulted() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".codeparserignore"), "*.foo\n").unwrap();

        // Use respect_gitignore = true so the project tier is active — if the
        // old code were still present, it would pick up .codeparserignore there.
        let m = root(dir.path());
        assert!(
            !m.is_ignored(Path::new("bar.foo"), false),
            "bar.foo must NOT be ignored — .codeparserignore is no longer read"
        );
    }

    #[allow(dead_code)]
    fn _unused_pathbuf_to_silence_warning() {
        let _ = PathBuf::new();
    }
}
