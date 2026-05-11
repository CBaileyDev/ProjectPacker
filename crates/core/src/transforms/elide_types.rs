//! TypeScript-only: strip `export type { ... } from "..."` re-export lines.
//! Uses the existing tree-sitter typescript grammar via tree_sitter_compress::PooledParser.
//!
//! Scope: only matches `export type { Name1, Name2 } from "module"` — i.e.
//! type-only RE-exports. Leaves `export type Foo = ...` declarations alone.

use crate::tree_sitter_compress;
use std::sync::OnceLock;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

fn lang() -> tree_sitter::Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn ts_query() -> &'static Query {
    static Q: OnceLock<Query> = OnceLock::new();
    Q.get_or_init(|| {
        // Match `export type { ... } from "..."` re-export statements.
        let src = r#"(export_statement
            (export_clause) @clause
            (string) @source) @stmt"#;
        Query::new(&lang(), src).expect("type-elision query must compile")
    })
}

pub fn apply(path: &str, content: &str) -> Option<String> {
    if !(path.ends_with(".ts") || path.ends_with(".tsx")) {
        return None;
    }
    let mut parser = Parser::new();
    if parser.set_language(&lang()).is_err() {
        return None;
    }
    let tree = parser.parse(content, None)?;
    let bytes = content.as_bytes();
    let stmt_idx = ts_query()
        .capture_index_for_name("stmt")
        .expect("query has @stmt capture");
    let mut cursor = QueryCursor::new();
    // Collect byte-range of each matching export_statement whose source begins
    // with `export type {` (i.e. is a type-only re-export).
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut iter = cursor.matches(ts_query(), tree.root_node(), bytes);
    while let Some(m) = iter.next() {
        if let Some(stmt) = m.captures.iter().find(|c| c.index == stmt_idx) {
            let start = stmt.node.start_byte();
            let end = stmt.node.end_byte();
            let snippet = &content[start..end];
            if snippet.trim_start().starts_with("export type ") {
                // Include the trailing newline if present.
                let end_with_nl = if end < bytes.len() && bytes[end] == b'\n' {
                    end + 1
                } else {
                    end
                };
                ranges.push((start, end_with_nl));
            }
        }
    }
    if ranges.is_empty() {
        return None;
    }
    ranges.sort_by_key(|r| r.0);
    let mut out = String::with_capacity(content.len());
    let mut pos = 0usize;
    for (s, e) in &ranges {
        if *s > pos {
            out.push_str(&content[pos..*s]);
        }
        pos = *e;
    }
    if pos < content.len() {
        out.push_str(&content[pos..]);
    }
    let _ = tree_sitter_compress::detect_language(path); // keep the module link
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_type_only_reexport_line() {
        let src = "export type { Foo, Bar } from \"./types\";\nconst x = 1;\n";
        let out = apply("a.ts", src).expect("should change");
        assert!(!out.contains("export type {"));
        assert!(out.contains("const x = 1;"));
    }

    #[test]
    fn leaves_type_alias_declarations_alone() {
        let src = "export type Foo = string;\nconst x = 1;\n";
        assert!(apply("a.ts", src).is_none());
    }

    #[test]
    fn leaves_value_reexports_alone() {
        let src = "export { Foo } from \"./types\";\nconst x = 1;\n";
        assert!(apply("a.ts", src).is_none());
    }

    #[test]
    fn skips_non_typescript_files() {
        let src = "export type { Foo } from \"./x\";\n";
        assert!(apply("a.js", src).is_none());
    }
}
