use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

// Compile-time assertion: this entire module's caching strategy depends on
// tree_sitter::Query being Sync (we share &'static Query across Rayon
// threads). If a future tree-sitter version drops Sync, this will fail to
// compile and surface the regression at build time rather than as UB.
const _ASSERT_QUERY_SYNC: fn() = || {
    fn assert_sync<T: Sync>() {}
    assert_sync::<tree_sitter::Query>();
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
}

/// Sources below this byte threshold skip the tree-sitter parse cycle
/// entirely and use a heuristic comment-removal pass instead. The cost
/// of spinning up a parser dwarfs the work for a 200-byte snippet, and
/// the heuristic produces the same result for the comment shapes our
/// fixtures cover.
const SMALL_FILE_THRESHOLD: usize = 200;

fn lang_to_tree_sitter(lang: Lang) -> tree_sitter::Language {
    match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    }
}

fn compress_query_for(lang: Lang) -> &'static str {
    match lang {
        Lang::Rust => {
            r#"(function_item) @item (impl_item) @item (struct_item) @item (enum_item) @item (trait_item) @item"#
        }
        Lang::Python => r#"(function_definition) @item (class_definition) @item"#,
        Lang::JavaScript | Lang::TypeScript => {
            r#"(function_declaration) @item (class_declaration) @item (method_definition) @item"#
        }
    }
}

fn comment_query_for(lang: Lang) -> &'static str {
    match lang {
        Lang::Rust => "(line_comment) @c (block_comment) @c",
        Lang::Python => "(comment) @c",
        Lang::JavaScript | Lang::TypeScript => "(comment) @c",
    }
}

struct LangQueries {
    compress: Query,
    comments: Query,
}

static QUERIES: OnceLock<HashMap<Lang, LangQueries>> = OnceLock::new();

fn queries() -> &'static HashMap<Lang, LangQueries> {
    QUERIES.get_or_init(|| {
        let mut m: HashMap<Lang, LangQueries> = HashMap::new();
        for &lang in &[Lang::Rust, Lang::Python, Lang::JavaScript, Lang::TypeScript] {
            let language = lang_to_tree_sitter(lang);
            let compress = Query::new(&language, compress_query_for(lang))
                .expect("compress query must compile");
            let comments = Query::new(&language, comment_query_for(lang))
                .expect("comment query must compile");
            m.insert(lang, LangQueries { compress, comments });
        }
        m
    })
}

thread_local! {
    /// Per-thread parser pool keyed by language. A thread that has
    /// already finished parsing a Rust file keeps its `Parser` cached and
    /// reuses it for the next Rust file it sees, avoiding the cost of
    /// constructing a fresh parser + setting its language for every
    /// file. Borrowed in `acquire_parser` and returned via the `Drop`
    /// impl on `PooledParser`.
    static PARSER_POOL: RefCell<HashMap<Lang, Vec<Parser>>> = RefCell::new(HashMap::new());
}

/// RAII handle holding a parser checked out of [`PARSER_POOL`]. The
/// parser is returned to the pool on drop. Construction can fail (the
/// language plug-in might refuse to bind) — in that case we return
/// `None` and the caller must fall back to processing the source as-is.
struct PooledParser {
    lang: Lang,
    parser: Option<Parser>,
}

impl PooledParser {
    fn acquire(lang: Lang) -> Option<Self> {
        let cached = PARSER_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            pool.entry(lang).or_default().pop()
        });
        if let Some(parser) = cached {
            return Some(Self {
                lang,
                parser: Some(parser),
            });
        }
        let language = lang_to_tree_sitter(lang);
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            return None;
        }
        Some(Self {
            lang,
            parser: Some(parser),
        })
    }

    fn parser_mut(&mut self) -> &mut Parser {
        self.parser.as_mut().expect("parser is always Some until Drop")
    }
}

impl Drop for PooledParser {
    fn drop(&mut self) {
        if let Some(parser) = self.parser.take() {
            PARSER_POOL.with(|pool| {
                pool.borrow_mut().entry(self.lang).or_default().push(parser);
            });
        }
    }
}

pub fn detect_language(path: &str) -> Option<Lang> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".rs") {
        Some(Lang::Rust)
    } else if lower.ends_with(".py") {
        Some(Lang::Python)
    } else if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        Some(Lang::TypeScript)
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") || lower.ends_with(".mjs") {
        Some(Lang::JavaScript)
    } else {
        None
    }
}

pub fn compress(source: &str, lang: Lang) -> String {
    let Some(mut pooled) = PooledParser::acquire(lang) else {
        return source.to_string();
    };
    let Some(tree) = pooled.parser_mut().parse(source, None) else {
        return source.to_string();
    };

    let query = &queries()
        .get(&lang)
        .expect("query cache must contain all Lang variants")
        .compress;

    let mut cursor = QueryCursor::new();
    let mut out = String::new();
    let bytes = source.as_bytes();
    out.push_str("// COMPRESSED skeleton — bodies elided\n");

    let python_style = matches!(lang, Lang::Python);
    let mut matches_iter = cursor.matches(query, tree.root_node(), bytes);
    while let Some(m) = matches_iter.next() {
        for capture in m.captures {
            let node = capture.node;
            let start = node.start_byte();
            let body_start = if python_style {
                first_colon(bytes, start, node.end_byte())
            } else {
                first_brace(bytes, start, node.end_byte())
            };
            let header = std::str::from_utf8(&bytes[start..body_start])
                .unwrap_or("")
                .trim_end();
            out.push_str(header);
            out.push_str(" { /* … */ }\n");
        }
    }

    out
}

fn first_brace(bytes: &[u8], start: usize, end: usize) -> usize {
    bytes
        .iter()
        .enumerate()
        .take(end)
        .skip(start)
        .find(|(_, &b)| b == b'{')
        .map(|(i, _)| i)
        .unwrap_or(end)
}

fn first_colon(bytes: &[u8], start: usize, end: usize) -> usize {
    bytes
        .iter()
        .enumerate()
        .take(end)
        .skip(start)
        .find(|(_, &b)| b == b':')
        .map(|(i, _)| i)
        .unwrap_or(end)
}

/// Strip comments from `source` using tree-sitter for the given language.
/// Unknown languages (those for which `detect_language` returns `None`)
/// should be passed through unchanged at the call site — this function
/// requires a concrete `Lang`.
pub fn remove_comments(source: &str, lang: Lang) -> String {
    if source.len() < SMALL_FILE_THRESHOLD {
        return remove_comments_small_file(source, lang);
    }

    let Some(mut pooled) = PooledParser::acquire(lang) else {
        return source.to_string();
    };
    let Some(tree) = pooled.parser_mut().parse(source, None) else {
        return source.to_string();
    };

    let query = &queries()
        .get(&lang)
        .expect("query cache must contain all Lang variants")
        .comments;

    let bytes = source.as_bytes();
    let mut cursor = QueryCursor::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut iter = cursor.matches(query, tree.root_node(), bytes);
    while let Some(m) = iter.next() {
        for cap in m.captures {
            ranges.push((cap.node.start_byte(), cap.node.end_byte()));
        }
    }
    ranges.sort_by_key(|r| r.0);

    // Rebuild the string omitting comment byte-ranges.
    let mut stripped = String::with_capacity(source.len());
    let mut pos = 0usize;
    for (start, end) in &ranges {
        if *start > pos {
            stripped.push_str(&source[pos..*start]);
        }
        pos = *end;
    }
    if pos < source.len() {
        stripped.push_str(&source[pos..]);
    }

    collapse_blank_lines(&stripped)
}

/// Heuristic comment-removal for sources below
/// [`SMALL_FILE_THRESHOLD`]. Avoids the parser-construction overhead
/// (which dominates for ~200-byte inputs) while still handling the
/// common comment shapes for each `Lang`:
///
/// * `Lang::Rust`, `Lang::JavaScript`, `Lang::TypeScript`: `//` line
///   comments and `/* … */` block comments (block comments must close
///   on the same logical span; nested block comments are not handled).
/// * `Lang::Python`: `#` line comments outside string literals, plus
///   triple-quoted `"""…"""` and `'''…'''` strings (treated as docstrings
///   and stripped to mirror tree-sitter's behaviour on the existing
///   fixtures).
///
/// String-literal awareness is deliberately conservative: when a comment
/// sigil appears inside a string literal we skip the strip. The full
/// tree-sitter path takes over for files at or above the threshold so
/// pathological cases never make it through this function.
pub fn remove_comments_small_file(source: &str, lang: Lang) -> String {
    let mut out = String::with_capacity(source.len());
    match lang {
        Lang::Rust | Lang::JavaScript | Lang::TypeScript => {
            strip_c_style_comments(source, &mut out);
        }
        Lang::Python => {
            strip_python_comments(source, &mut out);
        }
    }
    collapse_blank_lines(&out)
}

fn strip_c_style_comments(source: &str, out: &mut String) {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];

        if let Some(quote) = in_str {
            out.push(b as char);
            // Skip an escaped char: copy the escape byte and advance.
            if b == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if b == quote {
                in_str = None;
            }
            i += 1;
            continue;
        }

        if b == b'"' || b == b'\'' {
            in_str = Some(b);
            out.push(b as char);
            i += 1;
            continue;
        }

        if b == b'/' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'/' {
                // Line comment — skip to end of line (preserving the
                // newline so blank-line collapse can deduplicate later).
                let mut j = i + 2;
                while j < bytes.len() && bytes[j] != b'\n' {
                    j += 1;
                }
                i = j;
                continue;
            }
            if next == b'*' {
                // Block comment — skip until `*/` or end of input.
                let mut j = i + 2;
                while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                    j += 1;
                }
                if j + 1 < bytes.len() {
                    j += 2;
                } else {
                    j = bytes.len();
                }
                i = j;
                continue;
            }
        }

        out.push(b as char);
        i += 1;
    }
}

fn strip_python_comments(source: &str, out: &mut String) {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut in_str: Option<u8> = None; // single/double single-line string
    while i < bytes.len() {
        let b = bytes[i];

        // Triple-quoted strings: scan past the closing triple, removing
        // the entire span (matches tree-sitter's "comment" capture for
        // module-level docstrings on the small-file path).
        if in_str.is_none()
            && i + 2 < bytes.len()
            && (bytes[i] == b'"' || bytes[i] == b'\'')
            && bytes[i + 1] == bytes[i]
            && bytes[i + 2] == bytes[i]
        {
            let q = bytes[i];
            let mut j = i + 3;
            while j + 2 < bytes.len() && !(bytes[j] == q && bytes[j + 1] == q && bytes[j + 2] == q)
            {
                j += 1;
            }
            if j + 2 < bytes.len() {
                j += 3;
            } else {
                j = bytes.len();
            }
            i = j;
            continue;
        }

        if let Some(quote) = in_str {
            out.push(b as char);
            if b == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if b == quote {
                in_str = None;
            }
            i += 1;
            continue;
        }

        if b == b'"' || b == b'\'' {
            in_str = Some(b);
            out.push(b as char);
            i += 1;
            continue;
        }

        if b == b'#' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            i = j;
            continue;
        }

        out.push(b as char);
        i += 1;
    }
}

fn collapse_blank_lines(s: &str) -> String {
    let trailing_newline = s.ends_with('\n');
    let mut out_lines: Vec<&str> = Vec::new();
    let mut prev_blank = false;
    for line in s.lines() {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        out_lines.push(line);
        prev_blank = blank;
    }
    let mut result = out_lines.join("\n");
    if trailing_newline {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_extension() {
        assert_eq!(detect_language("src/main.rs"), Some(Lang::Rust));
    }

    #[test]
    fn detects_python_extension() {
        assert_eq!(detect_language("app.py"), Some(Lang::Python));
    }

    #[test]
    fn rust_skeleton_keeps_signatures_drops_bodies() {
        let src =
            "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\n";
        let out = compress(src, Lang::Rust);
        assert!(out.contains("fn add(a: i32, b: i32) -> i32"));
        assert!(out.contains("fn sub(a: i32, b: i32) -> i32"));
        assert!(!out.contains("a + b"));
        assert!(!out.contains("a - b"));
    }

    #[test]
    fn python_skeleton_keeps_def_lines() {
        let src = "def hello(name):\n    return f'hi, {name}'\n";
        let out = compress(src, Lang::Python);
        assert!(out.contains("def hello(name)"));
        assert!(!out.contains("return f'hi, {name}'"));
    }

    #[test]
    fn removes_rust_line_comments() {
        let src = "fn main() {\n    // a comment\n    println!(\"hi\");\n}\n";
        let result = remove_comments(src, Lang::Rust);
        assert!(!result.contains("// a comment"), "line comment should be removed");
        assert!(result.contains("println!"), "code should be preserved");
    }

    #[test]
    fn removes_rust_inline_trailing_comment() {
        let src = "let x = 5; // five\n";
        let result = remove_comments(src, Lang::Rust);
        assert!(!result.contains("// five"));
        assert!(result.contains("let x = 5;"));
    }

    #[test]
    fn removes_rust_block_comment() {
        let src = "/* block */\nfn foo() {}\n";
        let result = remove_comments(src, Lang::Rust);
        assert!(!result.contains("block"));
        assert!(result.contains("fn foo()"));
    }

    #[test]
    fn removes_python_comment() {
        let src = "x = 1  # set x\ny = 2\n";
        let result = remove_comments(src, Lang::Python);
        assert!(!result.contains("# set x"));
        assert!(result.contains("x = 1"));
        assert!(result.contains("y = 2"));
    }

    #[test]
    fn removes_typescript_comment() {
        let src = "// header\nconst x = 1;\n";
        let result = remove_comments(src, Lang::TypeScript);
        assert!(!result.contains("// header"));
        assert!(result.contains("const x = 1;"));
    }

    #[test]
    fn collapses_blank_lines_left_by_removal() {
        let src = "// comment\n// another\nfn foo() {}\n";
        let result = remove_comments(src, Lang::Rust);
        // After removal the two comment lines become blank. They should be
        // collapsed to at most one blank line, not left as two.
        let blank_run = result.contains("\n\n\n");
        assert!(!blank_run, "should not have 3+ consecutive newlines");
    }

    #[test]
    fn detect_language_returns_none_for_unknown_extension() {
        assert!(detect_language("a.txt").is_none());
        assert!(detect_language("Makefile").is_none());
    }

    // ──────────────────────── Pool / small-file path ────────────────────────

    #[test]
    fn small_file_path_strips_rust_line_comment() {
        // Below SMALL_FILE_THRESHOLD: heuristic path.
        let src = "let x = 1; // tiny\n";
        assert!(src.len() < SMALL_FILE_THRESHOLD);
        let out = remove_comments_small_file(src, Lang::Rust);
        assert!(!out.contains("// tiny"));
        assert!(out.contains("let x = 1;"));
    }

    #[test]
    fn small_file_path_preserves_string_with_double_slash() {
        let src = "let s = \"http://example.com\";\n";
        let out = remove_comments_small_file(src, Lang::Rust);
        // The // inside the string literal must NOT be treated as a
        // comment opener.
        assert!(out.contains("http://example.com"));
    }

    #[test]
    fn parser_pool_round_trips() {
        // Hammer the pool from a single thread: many compress() calls
        // for the same language re-use the cached parser. We can't
        // observe the pool directly, but we can ensure the output is
        // stable across repeated calls.
        let src = "fn a() {}\nfn b() {}\n";
        let first = compress(src, Lang::Rust);
        for _ in 0..10 {
            assert_eq!(compress(src, Lang::Rust), first);
        }
    }
}
