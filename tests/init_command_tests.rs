use std::process::Command;

use repokit::manifest::Manifest;

fn write_manifest(dir: &std::path::Path, manifest: &Manifest) {
    manifest
        .write_to_path(dir.join(".kit.toml"))
        .expect("write manifest");
}

fn bin_path() -> String {
    env!("CARGO_BIN_EXE_repokit").to_string()
}

#[test]
fn init_fails_when_manifest_exists() {
    let dir = tempfile::tempdir().expect("temp dir");

    let manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    write_manifest(dir.path(), &manifest);

    let output = Command::new(bin_path())
        .arg("init")
        .arg("--template")
        .arg("https://example.com/template.git")
        .arg("--ref")
        .arg("main")
        .arg("--profiles")
        .arg("base")
        .current_dir(dir.path())
        .output()
        .expect("run repokit init");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("manifest already exists; run repokit config set instead"));
}
