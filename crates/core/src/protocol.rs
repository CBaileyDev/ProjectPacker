use crate::error::{CoreError, CoreResult};

const V1: &str = include_str!("../../../docs/protocol/grok-to-cc-v1.md");

pub fn block_for_pack(goal: &str, version: &str) -> CoreResult<String> {
    let template = template_for(version)?;
    let body = extract_section(template, "PACK_PROTOCOL_BLOCK").ok_or_else(|| {
        CoreError::Internal(format!("template {version} missing PACK_PROTOCOL_BLOCK"))
    })?;
    let mut out = String::new();
    out.push_str(&format!("<protocol version=\"{version}\">\n"));
    out.push_str(body);
    out.push_str("\n</protocol>\n");
    out.push_str("<user_task>\n");
    out.push_str(goal.trim());
    out.push_str("\n</user_task>\n");
    Ok(out)
}

pub fn claude_code_prompt(version: &str) -> CoreResult<String> {
    let template = template_for(version)?;
    let body = extract_section(template, "CLAUDE_CODE_PROMPT").ok_or_else(|| {
        CoreError::Internal(format!("template {version} missing CLAUDE_CODE_PROMPT"))
    })?;
    Ok(body.to_string())
}

pub fn build_combined_prompt(plan_md: &str, version: &str) -> CoreResult<String> {
    let prompt = claude_code_prompt(version)?;
    let placeholder = "[The plan from Grok will be inserted here by the Bridge step.]";
    Ok(prompt.replace(placeholder, plan_md.trim()))
}

fn template_for(version: &str) -> CoreResult<&'static str> {
    match version {
        "grok-to-cc-v1" => Ok(V1),
        other => Err(CoreError::Internal(format!(
            "unknown protocol version: {other}"
        ))),
    }
}

fn extract_section<'a>(template: &'a str, name: &str) -> Option<&'a str> {
    let start_marker = format!("==={name}===");
    let end_marker = "===END===";
    let start = template.find(&start_marker)? + start_marker.len();
    let after = &template[start..];
    let end = after.find(end_marker)?;
    Some(after[..end].trim_matches(['\n', '\r']))
}

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanValidation {
    pub ok: bool,
    pub errors: Vec<PlanError>,
}

const REQUIRED_SECTIONS: &[&str] = &["Summary", "Risks", "Steps", "Verification", "Rollback"];
const VALID_ACTIONS: &[&str] = &["edit", "create", "delete", "rename", "run"];

pub fn validate_plan(md: &str, version: &str) -> CoreResult<PlanValidation> {
    if version != "grok-to-cc-v1" {
        return Err(CoreError::Internal(format!(
            "unknown protocol version: {version}"
        )));
    }
    let mut errors = Vec::new();

    let section_positions = find_sections(md);
    for (i, name) in REQUIRED_SECTIONS.iter().enumerate() {
        match section_positions.get(*name) {
            None => errors.push(PlanError {
                code: "missing_section".into(),
                message: format!("Missing section: ### {name}"),
            }),
            Some(&pos) => {
                if let Some((prev_name, &prev_pos)) = REQUIRED_SECTIONS
                    .iter()
                    .take(i)
                    .filter_map(|n| section_positions.get(*n).map(|p| (*n, p)))
                    .next_back()
                {
                    if pos < prev_pos {
                        errors.push(PlanError {
                            code: "out_of_order".into(),
                            message: format!("Section ### {name} appears before ### {prev_name}"),
                        });
                    }
                }
            }
        }
    }

    let steps_text = section_text(md, &section_positions, "Steps");
    let verification_text = section_text(md, &section_positions, "Verification");

    if let Some(steps) = steps_text {
        validate_steps(steps, &mut errors);
    }

    if let Some(v) = verification_text {
        if !v.lines().any(|l| l.trim_start().starts_with("- ")) {
            errors.push(PlanError {
                code: "verification_empty".into(),
                message: "Verification section has no bullet items".into(),
            });
        }
    }

    Ok(PlanValidation {
        ok: errors.is_empty(),
        errors,
    })
}

fn find_sections(md: &str) -> std::collections::HashMap<&'static str, usize> {
    let mut out = std::collections::HashMap::new();
    let mut byte_pos = 0usize;
    for line in md.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            let header = rest.trim_end_matches(['\r', '\n']).trim();
            for &name in REQUIRED_SECTIONS {
                if header.eq_ignore_ascii_case(name) && !out.contains_key(name) {
                    out.insert(name, byte_pos);
                }
            }
        }
        byte_pos += line.len();
    }
    out
}

fn section_text<'a>(
    md: &'a str,
    positions: &std::collections::HashMap<&'static str, usize>,
    name: &str,
) -> Option<&'a str> {
    let start = *positions.get(name)?;
    let next = REQUIRED_SECTIONS
        .iter()
        .filter_map(|n| positions.get(*n).copied())
        .filter(|&p| p > start)
        .min()
        .unwrap_or(md.len());
    Some(&md[start..next])
}

fn validate_steps(steps: &str, errors: &mut Vec<PlanError>) {
    let mut step_num = 0u32;
    let mut current_block = String::new();

    for line in steps.lines() {
        if line.trim_start().starts_with("#### Step ") {
            if step_num > 0 {
                check_step(step_num, &current_block, errors);
            }
            step_num += 1;
            current_block.clear();
        }
        current_block.push_str(line);
        current_block.push('\n');
    }
    if step_num > 0 {
        check_step(step_num, &current_block, errors);
    }
    if step_num == 0 {
        errors.push(PlanError {
            code: "no_steps".into(),
            message: "Steps section has no #### Step N: items".into(),
        });
    }
}

fn check_step(num: u32, block: &str, errors: &mut Vec<PlanError>) {
    let action = field(block, "Action");
    let target = field(block, "Target");
    let rationale = field(block, "Rationale");
    let has_details = block.contains("**Details:**");

    if action.is_none() {
        errors.push(PlanError {
            code: "missing_field".into(),
            message: format!("Step {num}: missing Action"),
        });
    } else if let Some(a) = action {
        let a_norm = a.trim().to_lowercase();
        if !VALID_ACTIONS.iter().any(|v| **v == a_norm) {
            errors.push(PlanError {
                code: "invalid_action".into(),
                message: format!(
                    "Step {num}: invalid Action '{a}' (expected edit|create|delete|rename|run)"
                ),
            });
        }
    }

    if target.is_none() {
        errors.push(PlanError {
            code: "missing_field".into(),
            message: format!("Step {num}: missing Target"),
        });
    }

    match rationale {
        None => errors.push(PlanError {
            code: "missing_field".into(),
            message: format!("Step {num}: missing Rationale"),
        }),
        Some(r) if r.trim().len() < 10 => errors.push(PlanError {
            code: "rationale_too_short".into(),
            message: format!("Step {num}: Rationale must be ≥10 characters"),
        }),
        _ => {}
    }

    if !has_details {
        errors.push(PlanError {
            code: "missing_field".into(),
            message: format!("Step {num}: missing Details"),
        });
    }
}

fn field<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("**{name}:**");
    let idx = block.find(&needle)? + needle.len();
    let after = &block[idx..];
    let line_end = after.find('\n').unwrap_or(after.len());
    Some(after[..line_end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_for_pack_wraps_with_protocol_tag() {
        let s = block_for_pack("Add a feature", "grok-to-cc-v1").unwrap();
        assert!(s.starts_with("<protocol version=\"grok-to-cc-v1\">"));
        assert!(s.contains("</protocol>"));
        assert!(s.contains("<user_task>"));
        assert!(s.contains("Add a feature"));
    }

    #[test]
    fn block_for_pack_includes_strict_format_text() {
        let s = block_for_pack("hi", "grok-to-cc-v1").unwrap();
        assert!(s.contains("Plan format (STRICT)"));
        assert!(s.contains("Rationale"));
    }

    #[test]
    fn claude_code_prompt_starts_correctly() {
        let s = claude_code_prompt("grok-to-cc-v1").unwrap();
        assert!(s.contains("EXECUTOR with veto power"));
        assert!(s.contains("Challenge before executing"));
    }

    #[test]
    fn build_combined_prompt_substitutes_plan() {
        let plan = "### Summary\nA tiny plan.\n";
        let s = build_combined_prompt(plan, "grok-to-cc-v1").unwrap();
        assert!(s.contains("### Summary"));
        assert!(!s.contains("[The plan from Grok will be inserted here"));
    }

    #[test]
    fn unknown_version_errors() {
        let err = block_for_pack("hi", "grok-to-cc-v999").unwrap_err();
        assert!(matches!(err, CoreError::Internal(_)));
    }

    fn good_plan() -> &'static str {
        r#"
### Summary
A short overview.

### Risks
- None.

### Steps

#### Step 1: Add a thing
**Action:** create
**Target:** src/thing.rs
**Rationale:** This module is needed because there is currently no place for the thing logic.
**Details:**
```rust
pub fn thing() {}
```

### Verification
- `cargo test` passes.

### Rollback
- `git revert`.
"#
    }

    #[test]
    fn validates_a_correct_plan() {
        let v = validate_plan(good_plan(), "grok-to-cc-v1").unwrap();
        assert!(v.ok, "errors: {:?}", v.errors);
    }

    #[test]
    fn flags_missing_summary_section() {
        let plan = good_plan().replace("### Summary\nA short overview.", "");
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v
            .errors
            .iter()
            .any(|e| e.code == "missing_section" && e.message.contains("Summary")));
    }

    #[test]
    fn flags_missing_rationale() {
        let plan = good_plan().replace(
            "**Rationale:** This module is needed because there is currently no place for the thing logic.\n",
            "",
        );
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v
            .errors
            .iter()
            .any(|e| e.message.contains("missing Rationale")));
    }

    #[test]
    fn flags_short_rationale() {
        let plan = good_plan().replace(
            "**Rationale:** This module is needed because there is currently no place for the thing logic.",
            "**Rationale:** short",
        );
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "rationale_too_short"));
    }

    #[test]
    fn flags_invalid_action() {
        let plan = good_plan().replace("**Action:** create", "**Action:** delete-everything");
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "invalid_action"));
    }

    #[test]
    fn flags_empty_verification() {
        let plan = good_plan().replace("- `cargo test` passes.", "");
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "verification_empty"));
    }

    #[test]
    fn flags_missing_steps() {
        let plan = "### Summary\nfoo\n### Risks\n- None.\n### Steps\n### Verification\n- yes\n### Rollback\n- yes\n";
        let v = validate_plan(plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "no_steps"));
    }
}
