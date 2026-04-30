use crate::error::{CoreError, CoreResult};

const V1: &str = include_str!("../../../docs/protocol/grok-to-cc-v1.md");

pub fn block_for_pack(goal: &str, version: &str) -> CoreResult<String> {
    let template = template_for(version)?;
    let body = extract_section(template, "PACK_PROTOCOL_BLOCK")
        .ok_or_else(|| CoreError::Internal(format!("template {version} missing PACK_PROTOCOL_BLOCK")))?;
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
    let body = extract_section(template, "CLAUDE_CODE_PROMPT")
        .ok_or_else(|| CoreError::Internal(format!("template {version} missing CLAUDE_CODE_PROMPT")))?;
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
        other => Err(CoreError::Internal(format!("unknown protocol version: {other}"))),
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
}
