//! Single source of truth for file-extension → language-name detection.
//!
//! Used by:
//!   - `pack::stats` to count files per language for the stats block.
//!   - `pack::markdown` to choose the code-fence language tag.
//!   - Future Phase 3 work (compress/repo-map) to choose tree-sitter grammars.
//!
//! Keeping this in one place prevents the maps from drifting (which would
//! cause real bugs — e.g. a `.dart` file counted in stats but rendered with
//! an empty markdown fence).

/// Detect a canonical language name from a path's extension.
///
/// Returns the lowercase language slug (e.g. `"rust"`, `"typescript"`,
/// `"json"`) or `None` for unknown extensions.
///
/// The `ext` argument is the path component after the last `.` — callers
/// should strip the leading dot before passing it in.
pub fn detect(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "jsx" => Some("jsx"),
        "json" | "jsonc" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "md" | "markdown" => Some("markdown"),
        "sh" | "bash" | "zsh" => Some("sh"),
        "css" | "scss" | "sass" | "less" => Some("css"),
        "html" | "htm" => Some("html"),
        "sql" => Some("sql"),
        "go" => Some("go"),
        "java" => Some("java"),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some("cpp"),
        "c" | "h" => Some("c"),
        "cs" => Some("csharp"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "scala" | "sc" => Some("scala"),
        "dart" => Some("dart"),
        "lua" => Some("lua"),
        "r" => Some("r"),
        "proto" => Some("proto"),
        "graphql" | "gql" => Some("graphql"),
        "xml" => Some("xml"),
        _ => None,
    }
}

/// Detect a language directly from a path string by extracting its extension.
/// Convenience for callers that have a full path rather than just an extension.
pub fn detect_from_path(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?;
    detect(ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_handles_common_extensions() {
        assert_eq!(detect("rs"), Some("rust"));
        assert_eq!(detect("py"), Some("python"));
        assert_eq!(detect("ts"), Some("typescript"));
    }

    #[test]
    fn detect_normalises_case() {
        assert_eq!(detect("RS"), Some("rust"));
        assert_eq!(detect("Json"), Some("json"));
    }

    #[test]
    fn detect_groups_aliases() {
        // js/mjs/cjs all → javascript
        assert_eq!(detect("js"), Some("javascript"));
        assert_eq!(detect("mjs"), Some("javascript"));
        assert_eq!(detect("cjs"), Some("javascript"));
        // C-family headers and sources
        assert_eq!(detect("hpp"), Some("cpp"));
        assert_eq!(detect("h"), Some("c"));
    }

    #[test]
    fn detect_returns_none_for_unknown() {
        assert_eq!(detect("weird"), None);
        assert_eq!(detect(""), None);
    }

    #[test]
    fn detect_from_path_extracts_extension() {
        assert_eq!(detect_from_path("src/main.rs"), Some("rust"));
        assert_eq!(detect_from_path("README.md"), Some("markdown"));
    }

    #[test]
    fn detect_from_path_returns_none_for_no_extension() {
        // A path with no dot: `rsplit('.').next()` returns the whole path,
        // which is then looked up as an "extension" and won't match.
        assert_eq!(detect_from_path("Makefile"), None);
    }
}
