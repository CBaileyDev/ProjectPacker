pub mod ruleset;

use regex::Regex;
use serde::Serialize;
use specta::Type;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretHit {
    pub kind: String,
    pub line: u32,
    pub matched_excerpt: String,
}

struct Rule {
    name: &'static str,
    pattern: Regex,
}

static RULES: OnceLock<Vec<Rule>> = OnceLock::new();

fn rules() -> &'static [Rule] {
    RULES.get_or_init(|| {
        vec![
            Rule { name: "aws-access-key", pattern: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap() },
            Rule { name: "aws-secret-key", pattern: Regex::new(r#"(?i)aws(.{0,20})?(secret|access)?(.{0,20})?[=:][\s"']*[A-Za-z0-9/+=]{40}"#).unwrap() },
            Rule { name: "github-token", pattern: Regex::new(r"\bghp_[A-Za-z0-9]{36,}\b").unwrap() },
            Rule { name: "github-fine-grained-token", pattern: Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{82}\b").unwrap() },
            Rule { name: "openai-key", pattern: Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_\-]{20,}\b").unwrap() },
            Rule { name: "anthropic-key", pattern: Regex::new(r"\bsk-ant-(?:api03-)?[A-Za-z0-9_\-]{20,}\b").unwrap() },
            Rule { name: "slack-token", pattern: Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").unwrap() },
            Rule { name: "private-key-pem", pattern: Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY-----").unwrap() },
            Rule { name: "google-api-key", pattern: Regex::new(r"\bAIza[0-9A-Za-z_\-]{35}\b").unwrap() },
            Rule { name: "stripe-live-key", pattern: Regex::new(r"\bsk_live_[0-9a-zA-Z]{24,}\b").unwrap() },
        ]
    }).as_slice()
}

pub fn scan(content: &str) -> Vec<SecretHit> {
    let mut hits = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        for rule in rules() {
            if let Some(m) = rule.pattern.find(line) {
                hits.push(SecretHit {
                    kind: rule.name.to_string(),
                    line: (line_idx + 1) as u32,
                    matched_excerpt: redact_excerpt(m.as_str()),
                });
            }
        }
    }
    hits
}

fn redact_excerpt(s: &str) -> String {
    if s.len() <= 8 {
        return "***".to_string();
    }
    let head: String = s.chars().take(4).collect();
    let tail_rev: String = s.chars().rev().take(4).collect();
    let tail: String = tail_rev.chars().rev().collect();
    format!("{head}***{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        let hits = scan("token = AKIAIOSFODNN7EXAMPLE\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "aws-access-key");
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn detects_github_token() {
        let hits = scan("ghp_1234567890abcdefghijklmnopqrstuvwxyz1\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "github-token");
    }

    #[test]
    fn detects_pem_private_key() {
        let hits = scan("first line\n-----BEGIN RSA PRIVATE KEY-----\nblah\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "private-key-pem");
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    fn no_false_positive_on_innocuous_string() {
        let hits = scan("let x = \"hello world\";\nfn main() {}\n");
        assert!(hits.is_empty());
    }

    #[test]
    fn excerpt_is_redacted() {
        let hits = scan("AKIAIOSFODNN7EXAMPLE\n");
        assert!(hits[0].matched_excerpt.contains("***"));
        assert!(!hits[0].matched_excerpt.contains("IOSFODNN"));
    }
}
