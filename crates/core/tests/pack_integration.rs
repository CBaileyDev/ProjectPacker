use projectpacker_core::pack;
use projectpacker_core::types::*;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

fn fixture_path(name: &str) -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn tiny_fixture_packs_with_expected_files() {
    let root = fixture_path("tiny");
    let opts = PackOptions {
        goal: "test".into(),
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(&PackTarget::Folder(root.clone()), &opts, tx, "test-job", CancellationToken::new()).unwrap();

    assert!(result.output.contains("README.md"));
    assert!(result.output.contains("src/main.rs"));
    assert!(result.output.contains("src/util.rs"));
    assert!(result.output.contains("docs/intro.md"));
    assert!(
        !result.output.contains("build/output.txt"),
        "build/ should be gitignored"
    );
}

#[test]
fn tiny_fixture_detects_secret() {
    let root = fixture_path("tiny");
    let opts = PackOptions {
        goal: "test".into(),
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(&PackTarget::Folder(root.clone()), &opts, tx, "test-job", CancellationToken::new()).unwrap();
    assert!(
        result.stats.secrets_found >= 1,
        "expected at least one secret hit"
    );
    assert!(
        !result.redactions.is_empty(),
        "expected redactions list to be non-empty"
    );
    assert_eq!(
        result.redactions[0].rule_id, "aws-access-token",
        "expected the AWS rule id from gitleaks ruleset"
    );
    assert!(
        result.redactions[0].file.contains("danger.txt"),
        "expected redaction.file to reference danger.txt, got: {}",
        result.redactions[0].file,
    );
    assert!(
        result.output.contains("[REDACTED:aws-access-token]"),
        "pack output must contain the redaction marker"
    );
    assert!(
        !result.output.contains("AKIAIOSFODNN7EXAMPLE"),
        "pack output must NOT contain the plaintext AWS key"
    );
    assert!(
        result.output.contains("<security_report"),
        "pack output (XML default) must contain the <security_report block"
    );
}

/// Orchestrator-level round-trip: a fixture with one secret, packed in XML
/// format, must surface the secret in the structured `redactions` list AND
/// in the `<security_report>` block of the output text.
#[test]
fn pack_emits_security_report_and_redactions_for_xml() {
    use std::fs;
    use tempfile::tempdir;

    let d = tempdir().unwrap();
    fs::write(
        d.path().join("danger.txt"),
        "key = AKIAIOSFODNN7EXAMPLE\n",
    )
    .unwrap();

    let opts = PackOptions {
        goal: "x".into(),
        count_tokens: false,
        secret_scan: true,
        respect_gitignore: false,
        format: PackFormat::Xml,
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(
        &PackTarget::Folder(d.path().to_path_buf()),
        &opts,
        tx,
        "job-secrep-xml",
        CancellationToken::new(),
    )
    .unwrap();

    // Structured surface
    assert!(
        result.redactions.iter().any(|r| r.rule_id == "aws-access-token"
            && r.file == "danger.txt"),
        "expected aws-access-token redaction on danger.txt, got: {:?}",
        result.redactions,
    );

    // XML surface
    assert!(
        result.output.contains("<security_report"),
        "missing <security_report block: {}",
        result.output,
    );
    assert!(result.output.contains("</security_report>"));
    assert!(
        result.output.contains("rule_id=\"aws-access-token\""),
        "missing rule_id attribute in security_report: {}",
        result.output,
    );
    assert!(
        result.output.contains("[REDACTED:aws-access-token]"),
        "missing redaction marker in pack content"
    );
    assert!(
        !result.output.contains("AKIAIOSFODNN7EXAMPLE"),
        "plaintext AWS key still present in pack output"
    );
}

#[test]
fn tiny_fixture_includes_protocol_block() {
    let root = fixture_path("tiny");
    let opts = PackOptions {
        goal: "Add docs".into(),
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(&PackTarget::Folder(root.clone()), &opts, tx, "test-job", CancellationToken::new()).unwrap();
    assert!(result.output.contains("<protocol version=\"grok-to-cc-v1\">"));
    assert!(result.output.contains("<user_task>"));
    assert!(result.output.contains("Add docs"));
}

/// End-to-end: PackFormat::Markdown produces a Markdown document with the
/// expected header, file fences, and at least one of the fixture's files.
#[test]
fn tiny_fixture_packs_as_markdown() {
    let root = fixture_path("tiny");
    let opts = PackOptions {
        goal: "test".into(),
        format: PackFormat::Markdown,
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(
        &PackTarget::Folder(root.clone()),
        &opts,
        tx,
        "test-job-md",
        CancellationToken::new(),
    )
    .unwrap();

    assert!(
        result.output.starts_with("# Repository Pack"),
        "Markdown output must start with '# Repository Pack', got: {:?}",
        &result.output[..result.output.len().min(120)],
    );
    assert!(result.output.contains("## Files"), "missing '## Files' section");
    assert!(
        result.output.contains("```rust"),
        "expected a ```rust fenced code block (src/main.rs is Rust)",
    );
    assert!(result.output.contains("README.md"));
    assert!(result.output.contains("src/main.rs"));
    // No XML envelopes in Markdown output.
    assert!(!result.output.contains("<protocol version="));
    assert!(!result.output.contains("<documents>"));
}

/// End-to-end: PackFormat::PlainText produces a plain-text document with the
/// expected separator-style heading and file blocks.
#[test]
fn tiny_fixture_packs_as_plain_text() {
    let root = fixture_path("tiny");
    let opts = PackOptions {
        goal: "test".into(),
        format: PackFormat::PlainText,
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(
        &PackTarget::Folder(root.clone()),
        &opts,
        tx,
        "test-job-plain",
        CancellationToken::new(),
    )
    .unwrap();

    assert!(
        result.output.starts_with("=== STATS ===\n"),
        "Plain output must start with '=== STATS ===', got: {:?}",
        &result.output[..result.output.len().min(120)],
    );
    assert!(
        result.output.contains("=== END STATS ==="),
        "missing '=== END STATS ===' delimiter",
    );
    assert!(
        result.output.contains("=== src/main.rs ==="),
        "missing per-file '=== <path> ===' separator",
    );
    assert!(result.output.contains("README.md"));
    // No XML or Markdown envelopes in plain output.
    assert!(!result.output.contains("<protocol version="));
    assert!(!result.output.contains("```rust"));
    assert!(!result.output.contains("# Repository Pack"));
}
