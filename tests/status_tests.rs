use std::fs;
use std::path::PathBuf;

use repokit_cli::manifest::Manifest;
use repokit_cli::status::compare_to_manifest;

#[test]
fn status_reports_clean_when_all_files_match() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();

    let file_path = root.join("README.md");
    fs::write(&file_path, b"hello\n").expect("write file");

    let mut manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    let hash = repokit_cli::manifest::hash_file(&file_path).expect("hash file");
    manifest.insert_file_hash(PathBuf::from("README.md"), hash);

    let report = compare_to_manifest(root, &manifest).expect("status compare");

    assert!(report.is_clean());
    assert!(report.missing.is_empty());
    assert!(report.modified.is_empty());
    assert!(report.extra.is_empty());
}

#[test]
fn status_reports_missing_and_modified_and_extra() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();

    let keep_path = root.join("keep.txt");
    fs::write(&keep_path, b"original").expect("write file");

    let modified_path = root.join("changed.txt");
    fs::write(&modified_path, b"old").expect("write file");

    let extra_path = root.join("extra.txt");
    fs::write(&extra_path, b"extra").expect("write file");

    let mut manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );

    let keep_hash = repokit_cli::manifest::hash_file(&keep_path).expect("hash file");
    manifest.insert_file_hash(PathBuf::from("keep.txt"), keep_hash);

    let old_hash = repokit_cli::manifest::hash_bytes(b"old");
    manifest.insert_file_hash(PathBuf::from("changed.txt"), old_hash);

    manifest.insert_file_hash(PathBuf::from("missing.txt"), "deadbeef".to_string());

    fs::write(&modified_path, b"new").expect("modify file");

    let report = compare_to_manifest(root, &manifest).expect("status compare");

    assert!(!report.is_clean());
    assert_eq!(report.missing, vec![PathBuf::from("missing.txt")]);
    assert_eq!(report.modified, vec![PathBuf::from("changed.txt")]);
    assert_eq!(report.extra, vec![PathBuf::from("extra.txt")]);
}
