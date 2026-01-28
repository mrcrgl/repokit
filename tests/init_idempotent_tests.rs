use std::fs;
use std::path::PathBuf;

use repokit::manifest::Manifest;
use repokit::status::compare_to_manifest;

#[test]
fn manifest_metadata_matches_exactly() {
    let manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string(), "service".to_string()],
    );

    assert!(manifest.matches_metadata(
        "https://example.com/template.git",
        Some("main"),
        &vec!["base".to_string(), "service".to_string()],
    ));
    assert!(!manifest.matches_metadata(
        "https://example.com/other.git",
        Some("main"),
        &vec!["base".to_string(), "service".to_string()],
    ));
    assert!(!manifest.matches_metadata(
        "https://example.com/template.git",
        Some("dev"),
        &vec!["base".to_string(), "service".to_string()],
    ));
    assert!(!manifest.matches_metadata(
        "https://example.com/template.git",
        Some("main"),
        &vec!["base".to_string()],
    ));
}

#[test]
fn init_idempotence_precheck_is_clean_when_manifest_matches_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();

    let file_path = root.join("README.md");
    fs::write(&file_path, b"hello\n").expect("write file");

    let mut manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    let hash = repokit::manifest::hash_file(&file_path).expect("hash file");
    manifest.insert_file_hash(PathBuf::from("README.md"), hash);

    let report = compare_to_manifest(root, &manifest).expect("status compare");

    assert!(report.is_clean());
    assert!(report.missing.is_empty());
    assert!(report.modified.is_empty());
    assert!(report.extra.is_empty());
}

#[test]
fn init_idempotence_precheck_detects_drift() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();

    let file_path = root.join("README.md");
    fs::write(&file_path, b"hello\n").expect("write file");

    let mut manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    manifest.insert_file_hash(
        PathBuf::from("README.md"),
        repokit::manifest::hash_bytes(b"old\n"),
    );

    let report = compare_to_manifest(root, &manifest).expect("status compare");

    assert!(!report.is_clean());
    assert_eq!(report.modified, vec![PathBuf::from("README.md")]);
}
