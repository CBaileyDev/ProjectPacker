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
