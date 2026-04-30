//! ProjectPacker core library — pure Rust packing pipeline. No Tauri deps.

pub mod types;
pub mod error;
pub mod ignore;
pub mod walker;
pub mod tokens;
pub mod secrets;
pub mod tree_sitter_compress;
pub mod github;
