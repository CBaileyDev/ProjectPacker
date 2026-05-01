use std::path::Path;
use walkdir::WalkDir;

/// Single-file pins — checked by exact relative path.
pub const PIN_FILE_PATHS: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
    ".github/copilot-instructions.md",
    ".aider.conf.yml",
    ".windsurfrules",
    ".context/index.md",
    "README.md",
];

/// Glob/directory pins — expanded at runtime rather than baked into a constant.
/// Format: each entry is a pattern tag (matched by `pinned_files`).
pub const PIN_GLOB_PATTERNS: &[&str] = &[
    ".cursor/rules/*.mdc",
    ".claude/**",
];

/// Resolve all pinned files that exist under `root`.
///
/// Returns paths relative to `root`, normalised with forward slashes, in stable
/// iteration order:
///   1. Single-file pins in declaration order (`PIN_FILE_PATHS`).
///   2. Glob results in pin-list declaration order, then alphabetical within
///      each glob pattern.
///
/// A path is included at most once (deduplication by insertion order).
pub fn pinned_files(root: &Path) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut push = |path: String| {
        if seen.insert(path.clone()) {
            results.push(path);
        }
    };

    // ── 1. Single-file pins ────────────────────────────────────────────────
    for &rel in PIN_FILE_PATHS {
        let abs = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if abs.is_file() {
            push(rel.to_string());
        }
    }

    // ── 2. Glob: .cursor/rules/*.mdc ──────────────────────────────────────
    let cursor_rules = root.join(".cursor").join("rules");
    if cursor_rules.is_dir() {
        let mut matches: Vec<String> = WalkDir::new(&cursor_rules)
            .min_depth(1)
            .max_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_type().is_file()
                    && e.path()
                        .extension()
                        .map(|x| x.eq_ignore_ascii_case("mdc"))
                        .unwrap_or(false)
            })
            .filter_map(|e| {
                e.path()
                    .strip_prefix(root)
                    .ok()
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        matches.sort();
        for m in matches {
            push(m);
        }
    }

    // ── 3. Glob: .claude/** (all files, recursive) ─────────────────────────
    let claude_dir = root.join(".claude");
    if claude_dir.is_dir() {
        let mut matches: Vec<String> = WalkDir::new(&claude_dir)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                e.path()
                    .strip_prefix(root)
                    .ok()
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        matches.sort();
        for m in matches {
            push(m);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn pinned_files_finds_root_level_singles() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("AGENTS.md"), "agents").unwrap();
        fs::write(d.path().join("README.md"), "readme").unwrap();
        let result = pinned_files(d.path());
        assert!(result.contains(&"AGENTS.md".to_string()));
        assert!(result.contains(&"README.md".to_string()));
    }

    #[test]
    fn pinned_files_skips_when_absent() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("README.md"), "readme").unwrap();
        let result = pinned_files(d.path());
        assert_eq!(result, vec!["README.md".to_string()]);
    }

    #[test]
    fn pinned_files_finds_cursor_rules_glob() {
        let d = tempdir().unwrap();
        let rules_dir = d.path().join(".cursor").join("rules");
        fs::create_dir_all(&rules_dir).unwrap();
        fs::write(rules_dir.join("a.mdc"), "rule a").unwrap();
        fs::write(rules_dir.join("b.mdc"), "rule b").unwrap();
        fs::write(rules_dir.join("c.txt"), "ignored").unwrap();
        let result = pinned_files(d.path());
        assert!(result.contains(&".cursor/rules/a.mdc".to_string()));
        assert!(result.contains(&".cursor/rules/b.mdc".to_string()));
        assert!(!result.contains(&".cursor/rules/c.txt".to_string()));
    }

    #[test]
    fn pinned_files_finds_claude_recursive() {
        let d = tempdir().unwrap();
        let claude_dir = d.path().join(".claude");
        let sub_dir = claude_dir.join("sub");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(claude_dir.join("instructions.md"), "inst").unwrap();
        fs::write(sub_dir.join("notes.md"), "notes").unwrap();
        let result = pinned_files(d.path());
        assert!(result.contains(&".claude/instructions.md".to_string()));
        assert!(result.contains(&".claude/sub/notes.md".to_string()));
    }

    #[test]
    fn pinned_files_returns_in_declaration_order() {
        let d = tempdir().unwrap();
        // Both AGENTS.md (index 0) and CLAUDE.md (index 1) exist.
        fs::write(d.path().join("CLAUDE.md"), "claude").unwrap();
        fs::write(d.path().join("AGENTS.md"), "agents").unwrap();
        let result = pinned_files(d.path());
        let agents_pos = result.iter().position(|s| s == "AGENTS.md").unwrap();
        let claude_pos = result.iter().position(|s| s == "CLAUDE.md").unwrap();
        assert!(
            agents_pos < claude_pos,
            "AGENTS.md (index 0 in PIN_FILE_PATHS) must come before CLAUDE.md (index 1)"
        );
    }

    #[test]
    fn pinned_files_ignores_unrelated_files() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("random.md"), "random").unwrap();
        let src = d.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        let result = pinned_files(d.path());
        assert!(result.is_empty(), "expected empty result, got {:?}", result);
    }
}
