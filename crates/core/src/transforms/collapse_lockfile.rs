//! Lockfile detection + body collapse.

const LOCKFILE_BASENAMES: &[&str] = &[
    "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "Cargo.lock",
    "Gemfile.lock", "poetry.lock", "composer.lock", "Pipfile.lock", "go.sum",
];

pub fn is_lockfile(path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path);
    let basename = basename.rsplit('\\').next().unwrap_or(basename);
    LOCKFILE_BASENAMES.iter().any(|&n| n == basename)
}

/// Returns `Some(collapsed_body)` if `path` is a lockfile, else `None`.
/// `hash_prefix` is the first 12 chars of the file's BLAKE3 hash.
pub fn collapse(path: &str, content: &str, hash_prefix: &str) -> Option<String> {
    if !is_lockfile(path) { return None; }
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 25 { return None; } // short lockfile — keep as-is
    let head: String = lines.iter().take(20).copied().collect::<Vec<_>>().join("\n");
    let tail: String = lines.iter().rev().take(5).rev().copied().collect::<Vec<_>>().join("\n");
    let omitted = total.saturating_sub(25);
    let original_bytes = content.len();
    Some(format!(
        "{head}\n[COMPRESSED: lockfile | original-bytes: {original_bytes} | sha: {hash_prefix}]\n[{omitted} lines omitted]\n{tail}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_lockfile_basenames() {
        assert!(is_lockfile("package-lock.json"));
        assert!(is_lockfile("apps/foo/pnpm-lock.yaml"));
        assert!(is_lockfile("crates\\Cargo.lock"));
        assert!(!is_lockfile("src/main.rs"));
    }

    #[test]
    fn collapse_keeps_short_lockfile_intact() {
        let content = "{\n  \"name\": \"x\"\n}\n";
        assert!(collapse("package-lock.json", content, "deadbeef0000").is_none());
    }

    #[test]
    fn collapse_replaces_long_lockfile_body() {
        let mut content = String::new();
        for i in 0..200 {
            content.push_str(&format!("  \"dep{i}\": \"1.0.0\"\n"));
        }
        let out = collapse("package-lock.json", &content, "deadbeef0000").unwrap();
        assert!(out.starts_with("  \"dep0\""), "preserves first 20 lines");
        assert!(out.contains("[COMPRESSED: lockfile"));
        assert!(out.contains("175 lines omitted"));
        assert!(out.contains("\"dep199\""), "preserves last 5 lines");
    }

    #[test]
    fn collapse_returns_none_for_non_lockfile() {
        let content = "fn main() {}\n".repeat(50);
        assert!(collapse("src/main.rs", &content, "abc").is_none());
    }
}
