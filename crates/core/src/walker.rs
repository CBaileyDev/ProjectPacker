use crate::ignore::IgnoreMatcher;
use crate::types::{FileFound, SkipReason};
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct WalkOutcome {
    pub included: Vec<FileFound>,
    pub skipped: Vec<(String, SkipReason)>,
}

pub struct WalkOptions {
    pub max_file_size_kb: u32,
}

/// Replace Windows-style backslash separators with forward slashes.
///
/// Unix fast-path: when the input contains no `\` we return the original
/// `String` unchanged (no scan beyond the contains-check, no allocation
/// of a fresh buffer). On Windows-shaped inputs we fall through to the
/// general replace.
pub fn normalize_separators(p: &str) -> String {
    if !p.contains('\\') {
        return p.to_owned();
    }
    p.replace('\\', "/")
}

pub fn walk(root: &Path, matcher: &IgnoreMatcher, opts: &WalkOptions) -> WalkOutcome {
    let mut included: Vec<FileFound> = Vec::with_capacity(2048);
    let mut skipped: Vec<(String, SkipReason)> = Vec::with_capacity(2048);

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let abs = entry.path();
        let rel = match abs.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = normalize_separators(&rel.to_string_lossy());

        if matcher.is_ignored(rel, false) {
            skipped.push((rel_str, SkipReason::Ignored));
            continue;
        }

        let bytes = match entry.metadata() {
            Ok(m) => m.len(),
            Err(_) => {
                skipped.push((rel_str, SkipReason::Inaccessible));
                continue;
            }
        };

        if bytes > (opts.max_file_size_kb as u64) * 1024 {
            skipped.push((rel_str, SkipReason::TooLarge));
            continue;
        }

        if crate::detect::is_binary(abs) {
            skipped.push((rel_str, SkipReason::Binary));
            continue;
        }

        included.push(FileFound {
            path: rel_str,
            bytes,
        });
    }

    WalkOutcome { included, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_fixture() -> tempfile::TempDir {
        let d = tempdir().unwrap();
        fs::write(d.path().join("a.txt"), "hello\n").unwrap();
        fs::write(d.path().join("b.rs"), "fn main() {}\n").unwrap();
        fs::create_dir(d.path().join("node_modules")).unwrap();
        fs::write(d.path().join("node_modules/x.js"), "noop").unwrap();
        fs::write(d.path().join("big.txt"), vec![b'x'; 4096]).unwrap();
        fs::write(d.path().join("binary.bin"), vec![0u8, 1, 2, 3]).unwrap();
        d
    }

    #[test]
    fn walks_and_skips_node_modules() {
        let d = make_fixture();
        let m = IgnoreMatcher::new(d.path(), &[], false);
        let out = walk(
            d.path(),
            &m,
            &WalkOptions {
                max_file_size_kb: 1024,
            },
        );
        let included: Vec<_> = out.included.iter().map(|f| f.path.as_str()).collect();
        assert!(included.contains(&"a.txt"));
        assert!(included.contains(&"b.rs"));
        assert!(!included.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn skips_oversize_files() {
        let d = make_fixture();
        let m = IgnoreMatcher::new(d.path(), &[], false);
        let out = walk(
            d.path(),
            &m,
            &WalkOptions {
                max_file_size_kb: 1,
            },
        );
        let big_skipped = out
            .skipped
            .iter()
            .any(|(p, r)| p == "big.txt" && matches!(r, SkipReason::TooLarge));
        assert!(big_skipped, "big.txt should be skipped as TooLarge");
    }

    #[test]
    fn skips_binary_files() {
        let d = make_fixture();
        let m = IgnoreMatcher::new(d.path(), &[], false);
        let out = walk(
            d.path(),
            &m,
            &WalkOptions {
                max_file_size_kb: 1024,
            },
        );
        let bin_skipped = out
            .skipped
            .iter()
            .any(|(p, r)| p == "binary.bin" && matches!(r, SkipReason::Binary));
        assert!(bin_skipped, "binary.bin should be skipped as Binary");
    }

    #[test]
    fn normalize_separators_unix_fast_path() {
        let p = "src/main.rs";
        // Unix-shaped input must round-trip identically.
        assert_eq!(normalize_separators(p), p);
    }

    #[test]
    fn normalize_separators_replaces_backslashes() {
        assert_eq!(normalize_separators("src\\main.rs"), "src/main.rs");
        assert_eq!(normalize_separators("a\\b\\c"), "a/b/c");
    }
}
