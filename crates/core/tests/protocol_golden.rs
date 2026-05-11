use projectpacker_core::protocol;

#[test]
fn protocol_block_for_pack_v1_is_frozen() {
    let s = protocol::block_for_pack("Add a hello endpoint", "grok-to-cc-v1").unwrap();
    insta::assert_snapshot!("v1_pack_block", s);
}

#[test]
fn protocol_claude_code_prompt_v1_is_frozen() {
    let s = protocol::claude_code_prompt("grok-to-cc-v1").unwrap();
    insta::assert_snapshot!("v1_cc_prompt", s);
}

#[test]
fn protocol_combined_prompt_with_known_plan_is_frozen() {
    let plan = r#"### Summary
A tiny plan.

### Risks
- None.

### Steps

#### Step 1: Do the thing
**Action:** create
**Target:** src/thing.rs
**Rationale:** This is needed for the feature to exist.
**Details:**
```rust
pub fn thing() {}
```

### Verification
- `cargo test` passes.

### Rollback
- `git revert`.
"#;
    let s = protocol::build_combined_prompt(plan, "grok-to-cc-v1").unwrap();
    insta::assert_snapshot!("v1_combined_prompt", s);
}

/// Snapshot the exact XML shape of the `<compression_report>` block so any
/// accidental format drift (attribute names, ordering, indent, self-closing
/// vs paired tags) breaks the build and surfaces in code review. Downstream
/// AI consumers parse this block.
#[test]
fn compression_report_block_shape() {
    use projectpacker_core::pack::xml::XmlBuilder;
    use projectpacker_core::types::{PackStats, TransformReport};

    let stats = PackStats {
        files_total: 1,
        files_included: 1,
        files_skipped: 0,
        bytes_total: 100,
        tokens_total: None,
        tokens_per_model: None,
        secrets_found: 0,
        duration_ms: 10,
        walk_ms: 1,
        process_ms: 1,
        secret_scan_ms: None,
        tokenize_ms: None,
        emit_ms: 1,
        transform_phase_ms: 2,
        transforms: vec![
            TransformReport {
                id: "trim_trailing_ws".into(),
                bytes_saved: 12,
                files_touched: 1,
                elapsed_ms: 0,
            },
            TransformReport {
                id: "dedup_files".into(),
                bytes_saved: 4096,
                files_touched: 2,
                elapsed_ms: 1,
            },
        ],
    };
    let mut b = XmlBuilder::with_capacity(128);
    b.compression_report_block(&stats);
    insta::assert_snapshot!(b.finish());
}
