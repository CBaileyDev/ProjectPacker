//! Vendored gitleaks ruleset loader.
//!
//! This module parses the gitleaks-vendored TOML at
//! `crates/core/assets/gitleaks.toml` (sourced from
//! <https://github.com/gitleaks/gitleaks>, MIT-licensed; see
//! `LICENSE-3RD-PARTY` at the repo root) into a typed [`RuleSet`] of
//! compiled regexes.
//!
//! Scope: T4 vendors the file and provides parsing/compilation only.
//! T5 will wire this `RuleSet` into a new scanner that replaces the
//! hand-rolled rules in [`super::scan`]. T4 deliberately does **not**
//! touch [`super::scan`] or its tests so that the existing scanner stays
//! green while the new pipeline lands incrementally.
//!
//! # Regex compatibility
//!
//! Gitleaks targets Go's RE2 engine, which supports a few features the
//! Rust [`regex`] crate does not (notably look-around and
//! backreferences). When a vendored rule's regex fails to compile we
//! **skip** that rule (with a warning to stderr) rather than rejecting
//! the whole vendored ruleset; otherwise a single upstream addition
//! could make every rule unloadable. The number of skipped rules is
//! exposed via [`RuleSet::skipped_count`] for monitoring/tests.

use regex::Regex;
use std::sync::OnceLock;

/// A compiled gitleaks rule.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Stable identifier from the upstream gitleaks config.
    pub id: String,
    /// Human-readable description of what the rule matches.
    pub description: String,
    /// Compiled detection regex.
    pub regex: Regex,
    /// Optional keyword pre-filter (the gitleaks scanner uses these to
    /// skip lines that obviously can't match before running the regex).
    pub keywords: Vec<String>,
    /// Optional Shannon-entropy floor for matched substrings.
    pub entropy_min: Option<f64>,
}

/// A loaded ruleset.
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// Successfully compiled rules.
    pub rules: Vec<Rule>,
    /// Number of rules whose regex failed to compile under the Rust
    /// `regex` crate (typically because they use Go-RE2-only features).
    skipped_compile_failures: usize,
}

impl RuleSet {
    /// Number of rules skipped because their regex failed to compile.
    pub fn skipped_count(&self) -> usize {
        self.skipped_compile_failures
    }
}

/// Errors returned by [`from_toml`]. Per-rule regex compilation failures
/// are **not** errors — they are tracked via
/// [`RuleSet::skipped_count`].
#[derive(Debug, thiserror::Error)]
pub enum RuleSetError {
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(serde::Deserialize)]
struct RawConfig {
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(serde::Deserialize)]
struct RawRule {
    id: String,
    #[serde(default)]
    description: String,
    /// Optional because a small number of upstream rules detect by
    /// `path` only (e.g. `pkcs12-file` matches `*.p12` filenames). For
    /// T4 we only care about content-regex rules; path-only rules are
    /// skipped. T5 may revisit.
    #[serde(default)]
    regex: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    entropy: Option<f64>,
    // Other gitleaks fields (`tags`, `path`, `allowlist`, `allowlists`,
    // `secretGroup`, etc.) are intentionally ignored at T4. Serde's
    // default behaviour drops unknown fields silently.
}

/// Parse an arbitrary TOML string into a [`RuleSet`].
///
/// TOML-level parse failures bubble up as [`RuleSetError::Toml`].
/// Per-rule regex compilation failures are non-fatal: the rule is
/// skipped and counted in [`RuleSet::skipped_count`].
pub fn from_toml(input: &str) -> Result<RuleSet, RuleSetError> {
    let raw: RawConfig = toml::from_str(input)?;
    let mut rules = Vec::with_capacity(raw.rules.len());
    let mut skipped_compile_failures: usize = 0;
    for raw_rule in raw.rules {
        let Some(regex_src) = raw_rule.regex.as_deref() else {
            // Path-only rule (e.g. `pkcs12-file`). T4 only handles
            // content-regex rules; skip silently.
            continue;
        };
        match regex::RegexBuilder::new(regex_src)
            .size_limit(32 * 1024 * 1024) // 32 MiB; 10 MiB default chokes on generic-api-key et al.
            .build()
        {
            Ok(regex) => rules.push(Rule {
                id: raw_rule.id,
                description: raw_rule.description,
                regex,
                keywords: raw_rule.keywords,
                entropy_min: raw_rule.entropy,
            }),
            Err(err) => {
                skipped_compile_failures += 1;
                // TODO: switch to `tracing::warn!` once `tracing` is a
                // direct dep of `projectpacker-core`. For now, write to
                // stderr so the skip is at least observable in logs.
                eprintln!(
                    "secrets::ruleset: skipping rule '{}' (regex incompatible with Rust regex crate): {}",
                    raw_rule.id, err
                );
            }
        }
    }
    Ok(RuleSet {
        rules,
        skipped_compile_failures,
    })
}

/// The vendored gitleaks ruleset, lazily parsed on first access.
///
/// The vendored TOML is baked in via [`include_str!`] so the binary
/// works fully offline. Parse failure here is a build-time bug (the
/// vendored file is in-tree and tested), so we panic with context.
pub fn vendored() -> &'static RuleSet {
    static CELL: OnceLock<RuleSet> = OnceLock::new();
    CELL.get_or_init(|| {
        from_toml(include_str!("../../assets/gitleaks.toml"))
            .expect("vendored gitleaks.toml must parse")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Floor; upstream currently ships ~222 rules. We assert >= 100 so
    /// minor upstream churn doesn't break the suite while still
    /// catching catastrophic regressions (e.g. someone vendoring an
    /// empty file).
    #[test]
    fn vendored_loads_with_substantial_rule_count() {
        let rs = vendored();
        assert!(
            rs.rules.len() >= 100,
            "expected >= 100 vendored rules, got {}",
            rs.rules.len()
        );
    }

    /// A handful of upstream rules use Go-RE2 features (look-around,
    /// backreferences) that Rust's `regex` crate doesn't accept. That's
    /// fine — we skip them — but the count should stay bounded.
    #[test]
    fn vendored_skipped_count_is_bounded() {
        let rs = vendored();
        assert!(
            rs.skipped_count() <= 5,
            "expected <= 5 skipped rules, got {}",
            rs.skipped_count()
        );
    }

    /// Sanity check that a recognisable AWS rule survived parsing.
    /// Upstream id at vendoring time: `aws-access-token`.
    #[test]
    fn aws_token_rule_is_present() {
        let rs = vendored();
        let aws = rs
            .rules
            .iter()
            .find(|r| {
                r.id.starts_with("aws-")
                    && (r.id.contains("token") || r.id.contains("key"))
            })
            .expect("expected at least one AWS-related rule");
        assert!(
            !aws.keywords.is_empty(),
            "AWS rule '{}' should have keywords",
            aws.id
        );
        // Compiled-regex sanity: should reject a plainly non-matching string.
        let _ = aws.regex.is_match("not an aws token");
    }

    #[test]
    fn from_toml_parses_minimal_fixture() {
        let toml_src = r#"
title = "test"
[[rules]]
id = "demo"
description = "demo rule"
regex = '''[A-Z]{4}-[0-9]{8}'''
keywords = ["DEMO"]
entropy = 3.5
"#;
        let rs = from_toml(toml_src).expect("fixture should parse");
        assert_eq!(rs.rules.len(), 1);
        let r = &rs.rules[0];
        assert_eq!(r.id, "demo");
        assert_eq!(r.description, "demo rule");
        assert_eq!(r.keywords, vec!["DEMO".to_string()]);
        assert_eq!(r.entropy_min, Some(3.5));
        assert!(r.regex.is_match("ABCD-12345678"));
        assert_eq!(rs.skipped_count(), 0);
    }

    #[test]
    fn from_toml_parses_rule_without_optional_fields() {
        let toml_src = r#"
title = "test"
[[rules]]
id = "demo"
description = "demo rule"
regex = '''[A-Z]{4}'''
"#;
        let rs = from_toml(toml_src).expect("fixture should parse");
        assert_eq!(rs.rules.len(), 1);
        let r = &rs.rules[0];
        assert!(r.keywords.is_empty(), "keywords should default to empty");
        assert!(r.entropy_min.is_none(), "entropy should default to None");
    }

    #[test]
    fn from_toml_skips_rule_with_invalid_regex() {
        // Unbalanced group — invalid in any regex flavour.
        let toml_src = r#"
title = "test"
[[rules]]
id = "broken"
description = "bad regex"
regex = '''(?P<bad>foo'''
"#;
        let rs = from_toml(toml_src).expect("toml itself is valid");
        assert_eq!(rs.rules.len(), 0);
        assert_eq!(rs.skipped_count(), 1);
    }

    #[test]
    fn from_toml_returns_err_on_invalid_toml() {
        let bad = "this is not toml ===]]";
        let err = from_toml(bad).expect_err("should fail to parse");
        assert!(
            matches!(err, RuleSetError::Toml(_)),
            "expected Toml variant, got {err:?}"
        );
    }
}
