//! Secrets scanner.
//!
//! T5 replaced the hand-rolled 10-rule engine with a vendored gitleaks
//! ruleset (T4) driven by a keyword pre-filter and Shannon-entropy
//! gating. The new entry point is [`scan_and_redact`] (in
//! [`engine`]); the legacy [`scan`] / [`SecretHit`] surface is preserved
//! as a thin shim so the orchestrator's existing call site (which only
//! reads `kind` / `line` to emit progress events and count hits) keeps
//! working. T6 will swap that call site to consume `scan_and_redact`
//! directly so it can also surface the redacted content.

pub mod engine;
pub mod ruleset;

pub use engine::{scan_and_redact, Redaction, ScanResult};

use serde::Serialize;
use specta::Type;

/// Legacy single-hit shape. Pre-T5 this struct was emitted by a
/// hand-rolled engine; post-T5 it's produced from a [`Redaction`] via
/// field mapping. Kept as-is so the orchestrator (and any in-flight
/// progress-event consumers on the frontend) don't need to change until
/// T6 lands.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretHit {
    /// Stable rule id from the vendored gitleaks ruleset
    /// (e.g. `"aws-access-token"`).
    pub kind: String,
    /// 1-based line number in the scanned content.
    pub line: u32,
    /// Redacted excerpt of the matched substring.
    pub matched_excerpt: String,
}

/// Backwards-compatible scan: runs the vendored ruleset over `content`
/// and flattens the result into [`SecretHit`]s. The orchestrator calls
/// this for its `secretsFound` counter and per-match
/// `ProgressEvent::SecretHit` emissions.
///
/// T6 will rewrite the orchestrator to call [`scan_and_redact`]
/// directly so it can also use the redacted content body.
pub fn scan(content: &str) -> Vec<SecretHit> {
    let result = scan_and_redact(content, ruleset::vendored());
    result
        .redactions
        .into_iter()
        .map(|r| SecretHit {
            kind: r.rule_id,
            line: r.line,
            matched_excerpt: r.matched_excerpt,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Backwards-compat tests for the legacy [`scan`] surface.
    //!
    //! Pre-T5, rule ids were custom (`"aws-access-key"`,
    //! `"github-token"`, `"private-key-pem"`). T5 swapped to gitleaks
    //! ids:
    //!
    //! - `aws-access-key`     → `aws-access-token`
    //! - `github-token`       → `github-pat`
    //! - `private-key-pem`    → `private-key`
    //!
    //! The behavioural assertions (a hit was found on the right line,
    //! the excerpt is redacted) carry over directly. Test names are
    //! preserved so git blame stays useful across the rewrite.
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        let hits = scan("token = AKIAIOSFODNN7EXAMPLE\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "aws-access-token");
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn detects_github_token() {
        let hits = scan("ghp_1234567890abcdefghijklmnopqrstuvwxyz1\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "github-pat");
    }

    #[test]
    fn detects_pem_private_key() {
        // The new gitleaks `private-key` rule requires a balanced
        // `BEGIN ... PRIVATE KEY ... KEY-----` block with at least 64
        // chars of payload. The old rule matched on the BEGIN line
        // alone; the new rule is more conservative (and produces
        // fewer false positives), but the behaviour we care about —
        // "a PEM block is detected and reported on the line it starts
        // on" — is preserved.
        let body = "A".repeat(80);
        let content = format!(
            "first line\n-----BEGIN RSA PRIVATE KEY-----\n{body}\n-----END RSA PRIVATE KEY-----\n"
        );
        let hits = scan(&content);
        assert!(
            !hits.is_empty(),
            "expected at least one hit on PEM block, got none"
        );
        let pem = hits
            .iter()
            .find(|h| h.kind == "private-key")
            .expect("private-key hit present");
        assert_eq!(pem.line, 2);
    }

    #[test]
    fn no_false_positive_on_innocuous_string() {
        let hits = scan("let x = \"hello world\";\nfn main() {}\n");
        assert!(
            hits.is_empty(),
            "innocuous code should not match any rule, got {hits:?}"
        );
    }

    #[test]
    fn excerpt_is_redacted() {
        let hits = scan("AKIAIOSFODNN7EXAMPLE\n");
        assert!(!hits.is_empty(), "expected at least one hit");
        let aws = hits
            .iter()
            .find(|h| h.kind == "aws-access-token")
            .expect("aws hit present");
        assert!(aws.matched_excerpt.contains("***"));
        assert!(!aws.matched_excerpt.contains("IOSFODNN"));
    }

    #[test]
    fn legacy_scan_returns_secret_hits_for_aws_key() {
        // New test: pins the contract that the legacy shim emits at
        // least one `SecretHit` carrying the new gitleaks rule id for
        // an AWS-shaped fixture. T6 still relies on this surface.
        let hits = scan("token = AKIAIOSFODNN7EXAMPLE\n");
        assert!(hits.iter().any(|h| h.kind == "aws-access-token"));
    }
}
