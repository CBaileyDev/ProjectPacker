//! ProjectPacker core library — pure Rust packing pipeline. No Tauri deps.

pub mod detect;
pub mod error;
pub mod github;
pub mod ignore;
pub mod lang;
pub mod pack;
pub mod protocol;
pub mod secrets;
pub mod tokens;
pub mod transforms;
pub mod tree_sitter_compress;
pub mod types;
pub mod walker;
