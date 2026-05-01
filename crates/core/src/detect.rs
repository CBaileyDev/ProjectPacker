//! Layered 5-stage binary detection pipeline.
//!
//! Applies layers in order, returning as soon as one yields a confident answer:
//!   1. Extension allow-list  → always text   (fast happy path)
//!   2. Extension deny-list   → always binary  (fast reject)
//!   3. NUL-byte + BOM sniff  → cheap content check
//!   4. `infer` magic-number  → ~100 formats
//!   5. `file-format` fallback → ~700 formats

use std::path::Path;

// ---------------------------------------------------------------------------
// Layer 1 — extension allow-list (these extensions are always treated as text)
// ---------------------------------------------------------------------------
static TEXT_EXTENSIONS: &[&str] = &[
    "bash",
    "bib",
    "c",
    "cc",
    "cfg",
    "conf",
    "cpp",
    "cs",
    "css",
    "csv",
    "dart",
    "dockerfile",
    "editorconfig",
    "env",
    "eslintrc",
    "fish",
    "gitattributes",
    "gitignore",
    "go",
    "gql",
    "graphql",
    "h",
    "hpp",
    "html",
    "ini",
    "java",
    "jl",
    "js",
    "json",
    "jsx",
    "kt",
    "lock",
    "lua",
    "md",
    "markdown",
    "php",
    "prettierrc",
    "properties",
    "proto",
    "py",
    "r",
    "rb",
    "rs",
    "scss",
    "sh",
    "sql",
    "svelte",
    "svg",
    "swift",
    "tex",
    "toml",
    "ts",
    "tsv",
    "tsx",
    "txt",
    "vue",
    "xml",
    "yaml",
    "yml",
    "zsh",
];

// ---------------------------------------------------------------------------
// Layer 2 — extension deny-list (these extensions are always treated as binary)
// ---------------------------------------------------------------------------
static BINARY_EXTENSIONS: &[&str] = &[
    "7z",
    "a",
    "avi",
    "bmp",
    "bz2",
    "class",
    "dll",
    "dylib",
    "eot",
    "exe",
    "flac",
    "gif",
    "gz",
    "ico",
    "icns",
    "jar",
    "jpeg",
    "jpg",
    "lib",
    "mkv",
    "mov",
    "mp3",
    "mp4",
    "o",
    "obj",
    "ogg",
    "otf",
    "pdf",
    "png",
    "pyc",
    "rar",
    "so",
    "tar",
    "tgz",
    "tif",
    "tiff",
    "ttf",
    "wasm",
    "wav",
    "webm",
    "webp",
    "woff",
    "woff2",
    "xz",
    "zip",
];

/// Returns `true` if the file at `path` is likely binary, `false` if likely text.
///
/// Applies a 5-stage pipeline in order; returns as soon as a confident answer
/// is reached. On any I/O failure in the later stages, returns `false`
/// (be permissive — let the encoding-fallback in pack handle edge cases).
pub fn is_binary(path: &Path) -> bool {
    // ------------------------------------------------------------------
    // Layer 1: extension allow-list → definitely text
    // ------------------------------------------------------------------
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lc = ext.to_lowercase();
        if TEXT_EXTENSIONS.contains(&ext_lc.as_str()) {
            return false;
        }

        // --------------------------------------------------------------
        // Layer 2: extension deny-list → definitely binary
        // --------------------------------------------------------------
        if BINARY_EXTENSIONS.contains(&ext_lc.as_str()) {
            return true;
        }
    }

    // ------------------------------------------------------------------
    // Layer 3: NUL-byte + BOM sniff (read up to 8192 bytes)
    // ------------------------------------------------------------------
    if let Some(result) = sniff_content(path) {
        return result;
    }

    // ------------------------------------------------------------------
    // Layer 4: `infer` magic-number recognition (~100 formats)
    // ------------------------------------------------------------------
    if let Some(result) = infer_layer(path) {
        return result;
    }

    // ------------------------------------------------------------------
    // Layer 5: `file-format` fallback (~700 formats)
    // ------------------------------------------------------------------
    file_format_layer(path)
}

/// Layer 3 helper: reads up to 8192 bytes, checks BOM and NUL bytes.
/// Returns `Some(true)` for binary, `Some(false)` for text, `None` to fall through.
fn sniff_content(path: &Path) -> Option<bool> {
    use std::io::Read;

    let mut buf = [0u8; 8192];
    let mut f = std::fs::File::open(path).ok()?;
    let n = f.read(&mut buf).ok()?;
    let buf = &buf[..n];

    // Empty file → treat as text
    if buf.is_empty() {
        return Some(false);
    }

    // Known text BOMs — strip offset before NUL scan
    let content_start = if buf.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        // UTF-32 LE BOM (4 bytes) — text
        return Some(false);
    } else if buf.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        // UTF-32 BE BOM (4 bytes) — text
        return Some(false);
    } else if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM (3 bytes)
        3
    } else if buf.starts_with(&[0xFF, 0xFE]) || buf.starts_with(&[0xFE, 0xFF]) {
        // UTF-16 LE / BE BOM (2 bytes)
        2
    } else {
        0
    };

    // NUL byte after any BOM → binary
    if buf[content_start..].contains(&0x00) {
        return Some(true);
    }

    // No NUL found — inconclusive; fall through to Layer 4
    None
}

/// Layer 4 helper: use `infer` crate magic-number recognition.
/// Returns `Some(true)` for binary, `Some(false)` for text, `None` if unknown.
fn infer_layer(path: &Path) -> Option<bool> {
    use infer::MatcherType;

    let kind = infer::get_from_path(path).ok()??;
    match kind.matcher_type() {
        MatcherType::Text => Some(false),
        _ => Some(true),
    }
}

/// Layer 5 helper: use `file-format` crate for ~700 format recognition.
/// Returns `false` on I/O error (be permissive).
///
/// `file_format::Kind` has no `Text` variant (v0.29), so binary-indicating kinds
/// are explicitly enumerated. Intentionally omitted as text-permissive:
///   - `Kind::Other` — unknown formats; default to text rather than guess.
///   - `Kind::Playlist` — M3U/XSPF/MPD are text-based.
///   - `Kind::Subtitle` — SRT/VTT/TTML are text-based; binary subtitle formats
///     (MatroskaSubtitles etc.) sit inside binary containers Layer 3 catches via NUL bytes.
fn file_format_layer(path: &Path) -> bool {
    use file_format::{FileFormat, Kind};

    match FileFormat::from_file(path) {
        Ok(fmt) => matches!(
            fmt.kind(),
            Kind::Archive
                | Kind::Audio
                | Kind::Compressed
                | Kind::Database
                | Kind::Diagram
                | Kind::Disk
                | Kind::Document
                | Kind::Ebook
                | Kind::Executable
                | Kind::Font
                | Kind::Formula
                | Kind::Geospatial
                | Kind::Image
                | Kind::Metadata
                | Kind::Model
                | Kind::Package
                | Kind::Presentation
                | Kind::Rom
                | Kind::Spreadsheet
                | Kind::Video
        ),
        Err(_) => false, // unreadable / unknown → treat as text (permissive)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// 1. Layer 1 wins: .rs extension → text, even if contents look binary.
    #[test]
    fn extension_allow_list_short_circuits_text() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("source.rs");
        // Contents contain NUL bytes that would make Layer 3 say "binary"
        fs::write(&path, b"fn main() {}\x00binary-looking\x00data").unwrap();
        assert!(!is_binary(&path), ".rs files must be treated as text regardless of content");
    }

    /// 2. Layer 2 wins: .png extension → binary, even if contents are pure ASCII.
    #[test]
    fn extension_deny_list_short_circuits_binary() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("image.png");
        // Contents are pure printable ASCII — Layers 3/4/5 would say "text"
        fs::write(&path, b"This is definitely plain text content, no magic numbers here").unwrap();
        assert!(is_binary(&path), ".png files must be treated as binary regardless of content");
    }

    /// 3. UTF-8 BOM prefix → text, even if subsequent bytes are high/non-ASCII.
    #[test]
    fn bom_marks_text_even_with_high_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("with_bom.data");
        // UTF-8 BOM followed by high bytes (not ASCII, but not NUL)
        let mut content = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        content.extend_from_slice(&[0xC3, 0xA9, 0xC3, 0xBC, 0xC3, 0xB6]); // é ü ö in UTF-8
        fs::write(&path, &content).unwrap();
        assert!(!is_binary(&path), "UTF-8 BOM prefix should mark file as text");
    }

    /// 4. NUL byte in content with no special extension → binary.
    #[test]
    fn nul_byte_marks_binary() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mystery.data");
        fs::write(&path, b"hello\x00world").unwrap();
        assert!(is_binary(&path), "NUL byte in content should mark file as binary");
    }

    /// 5. PNG magic bytes without a recognised extension → Layer 4 catches it.
    #[test]
    fn infer_recognizes_png_signature_when_no_extension() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mystery.unknown");
        // PNG magic number: 89 50 4E 47 0D 0A 1A 0A
        let mut content = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        // Pad with some non-NUL bytes so Layer 3 doesn't trigger on NULs
        content.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        fs::write(&path, &content).unwrap();
        assert!(is_binary(&path), "PNG magic bytes should be recognised as binary by Layer 4");
    }

    /// 6. Empty file → not binary.
    #[test]
    fn empty_file_is_not_binary() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.data");
        fs::write(&path, b"").unwrap();
        assert!(!is_binary(&path), "Empty file should be treated as text");
    }

    /// 7. Non-existent path → permissive false.
    #[test]
    fn unreadable_path_returns_false() {
        let path = Path::new("/this/path/does/not/exist/at/all.data");
        assert!(!is_binary(path), "Non-existent path should return false (permissive)");
    }

    /// 8. Plain ASCII text with unknown extension → not binary.
    #[test]
    fn plain_text_unknown_extension_passes_through() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notes.unknown");
        fs::write(&path, b"This is just plain ASCII text with no magic bytes or NUL bytes.").unwrap();
        assert!(!is_binary(&path), "Plain ASCII text with unknown extension should not be flagged as binary");
    }
}
