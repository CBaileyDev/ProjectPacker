use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
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
    let language: tree_sitter::Language = match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    };

    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return source.to_string();
    }
    let Some(tree) = parser.parse(source, None) else {
        return source.to_string();
    };

    let query_src = match lang {
        Lang::Rust => {
            r#"(function_item) @item (impl_item) @item (struct_item) @item (enum_item) @item (trait_item) @item"#
        }
        Lang::Python => r#"(function_definition) @item (class_definition) @item"#,
        Lang::JavaScript | Lang::TypeScript => {
            r#"(function_declaration) @item (class_declaration) @item (method_definition) @item"#
        }
    };
    let Ok(query) = Query::new(&language, query_src) else {
        return source.to_string();
    };

    let mut cursor = QueryCursor::new();
    let mut out = String::new();
    let bytes = source.as_bytes();
    out.push_str("// COMPRESSED skeleton — bodies elided\n");

    let python_style = matches!(lang, Lang::Python);
    let mut matches_iter = cursor.matches(&query, tree.root_node(), bytes);
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
    let language: tree_sitter::Language = match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    };

    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return source.to_string();
    }
    let Some(tree) = parser.parse(source, None) else {
        return source.to_string();
    };

    let query_src = match lang {
        Lang::Rust => "(line_comment) @c (block_comment) @c",
        Lang::Python => "(comment) @c",
        Lang::JavaScript | Lang::TypeScript => "(comment) @c",
    };
    let Ok(query) = Query::new(&language, query_src) else {
        return source.to_string();
    };

    let bytes = source.as_bytes();
    let mut cursor = QueryCursor::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut iter = cursor.matches(&query, tree.root_node(), bytes);
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

    // Collapse runs of whitespace-only lines to at most one blank line so
    // that removing a block of comments doesn't leave a large gap.
    let trailing_newline = stripped.ends_with('\n');
    let mut out_lines: Vec<&str> = Vec::new();
    let mut prev_blank = false;
    for line in stripped.lines() {
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
}
