use projectpacker_core::pack;
use projectpacker_core::types::*;
use std::path::PathBuf;

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
    let result = pack::pack(&root, &opts, tx, "test-job").unwrap();

    assert!(result.xml.contains("README.md"));
    assert!(result.xml.contains("src/main.rs"));
    assert!(result.xml.contains("src/util.rs"));
    assert!(result.xml.contains("docs/intro.md"));
    assert!(
        !result.xml.contains("build/output.txt"),
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
    let result = pack::pack(&root, &opts, tx, "test-job").unwrap();
    assert!(
        result.stats.secrets_found >= 1,
        "expected at least one secret hit"
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
    let result = pack::pack(&root, &opts, tx, "test-job").unwrap();
    assert!(result.xml.contains("<protocol version=\"grok-to-cc-v1\">"));
    assert!(result.xml.contains("<user_task>"));
    assert!(result.xml.contains("Add docs"));
}
