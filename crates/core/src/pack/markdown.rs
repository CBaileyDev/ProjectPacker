use crate::pack::FileEntry;
use crate::types::{PackFormat, PackOptions, PackStats};

pub fn render(
    root_label: &str,
    opts: &PackOptions,
    stats: &PackStats,
    entries: &[FileEntry],
) -> String {
    let mut out = String::new();

    out.push_str("# Repository Pack\n\n");
    out.push_str(&format!("**Target:** `{root_label}`\n\n"));
    if !opts.goal.is_empty() {
        out.push_str(&format!("**Goal:** {}\n\n", opts.goal));
    }

    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n|--------|-------|\n");
    out.push_str(&format!("| Files included | {} |\n", stats.files_included));
    out.push_str(&format!("| Files skipped  | {} |\n", stats.files_skipped));
    out.push_str(&format!("| Total bytes    | {} |\n", stats.bytes_total));
    if let Some(t) = stats.tokens_total {
        out.push_str(&format!("| Tokens ({})  | {t} |\n", opts.tokenizer_model));
    }
    out.push_str(&format!("| Duration       | {}ms |\n\n", stats.duration_ms));

    out.push_str("## Directory Structure\n\n```\n");
    for e in entries {
        out.push_str(&e.path);
        out.push('\n');
    }
    out.push_str("```\n\n## Files\n\n");

    for e in entries {
        out.push_str(&format!("### `{}`\n\n", e.path));
        let lang = ext_fence_lang(&e.path);
        out.push_str(&format!("```{lang}\n"));
        out.push_str(&e.content);
        if !e.content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }

    out
}

fn ext_fence_lang(path: &str) -> &str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "sh" | "bash" => "bash",
        "css" => "css",
        "html" | "htm" => "html",
        "sql" => "sql",
        "go" => "go",
        "java" => "java",
        "cpp" | "cc" | "cxx" => "cpp",
        "c" => "c",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> PackOptions {
        PackOptions {
            goal: "add a feature".into(),
            tokenizer_model: "gpt-4o-mini".into(),
            format: PackFormat::Markdown,
            ..PackOptions::default()
        }
    }

    fn stats() -> PackStats {
        PackStats {
            files_total: 2,
            files_included: 2,
            files_skipped: 0,
            bytes_total: 100,
            tokens_total: Some(50),
            secrets_found: 0,
            duration_ms: 10,
        }
    }

    #[test]
    fn renders_header_and_summary() {
        let out = render("my-repo", &opts(), &stats(), &[]);
        assert!(out.contains("# Repository Pack"));
        assert!(out.contains("`my-repo`"));
        assert!(out.contains("add a feature"));
        assert!(out.contains("| 2 |"));
        assert!(out.contains("gpt-4o-mini"));
        assert!(out.contains("| 50 |"));
    }

    #[test]
    fn renders_rust_file_with_correct_fence() {
        let entries = vec![FileEntry {
            path: "src/main.rs".into(),
            content: "fn main() {}\n".into(),
            bytes: 13,
            tokens: None,
            hash: "abc".into(),
        }];
        let out = render("repo", &opts(), &stats(), &entries);
        assert!(out.contains("### `src/main.rs`"));
        assert!(out.contains("```rust\n"));
        assert!(out.contains("fn main() {}"));
    }

    #[test]
    fn renders_directory_structure_section() {
        let entries = vec![
            FileEntry { path: "a.rs".into(), content: "".into(), bytes: 0, tokens: None, hash: "".into() },
            FileEntry { path: "b.py".into(), content: "".into(), bytes: 0, tokens: None, hash: "".into() },
        ];
        let out = render("repo", &opts(), &stats(), &entries);
        assert!(out.contains("## Directory Structure"));
        assert!(out.contains("a.rs"));
        assert!(out.contains("b.py"));
    }

    #[test]
    fn appends_trailing_newline_to_content_that_lacks_one() {
        let entries = vec![FileEntry {
            path: "x.ts".into(),
            content: "export {}".into(),
            bytes: 9,
            tokens: None,
            hash: "".into(),
        }];
        let out = render("r", &opts(), &stats(), &entries);
        assert!(out.contains("export {}\n```"));
    }
}
