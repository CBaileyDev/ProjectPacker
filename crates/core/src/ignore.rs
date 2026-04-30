use ::ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

const BUILTIN_DEFAULTS: &str = include_str!("ignore_defaults.txt");

pub struct IgnoreMatcher {
    builtin: Gitignore,
    project: Option<Gitignore>,
    custom: Option<Gitignore>,
}

impl IgnoreMatcher {
    pub fn new(project_root: &Path, custom_patterns: &[String], respect_gitignore: bool) -> Self {
        let builtin = build_from_lines(BUILTIN_DEFAULTS.lines(), Path::new(""));

        let project = if respect_gitignore {
            Some(build_project(project_root))
        } else {
            None
        };

        let custom = if custom_patterns.is_empty() {
            None
        } else {
            Some(build_from_lines(
                custom_patterns.iter().map(String::as_str),
                Path::new(""),
            ))
        };

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

fn build_from_lines<'a>(lines: impl IntoIterator<Item = &'a str>, root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let _ = b.add_line(None, line);
    }
    b.build().expect("ignore: builtin pattern compile failure")
}

fn build_project(root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    let _ = b.add(root.join(".gitignore"));
    let _ = b.add(root.join(".codeparserignore"));
    b.build().unwrap_or_else(|_| Gitignore::empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn root(p: &Path) -> IgnoreMatcher {
        IgnoreMatcher::new(p, &[], true)
    }

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

    #[allow(dead_code)]
    fn _unused_pathbuf_to_silence_warning() {
        let _ = PathBuf::new();
    }
}
