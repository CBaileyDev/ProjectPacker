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
        let src = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\n";
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
}
