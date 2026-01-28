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
fn config_show_prints_manifest_values() {
    let dir = tempfile::tempdir().expect("temp dir");

    let manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string(), "service".to_string()],
    );
    write_manifest(dir.path(), &manifest);

    let output = Command::new(bin_path())
        .arg("config")
        .arg("show")
        .current_dir(dir.path())
        .output()
        .expect("run repokit config show");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("template_url=https://example.com/template.git"));
    assert!(stdout.contains("ref=main"));
    assert!(stdout.contains("profiles=base,service"));
}

#[test]
fn config_set_updates_manifest_fields() {
    let dir = tempfile::tempdir().expect("temp dir");

    let manifest = Manifest::new(
        "https://example.com/old.git".to_string(),
        Some("old".to_string()),
        vec!["base".to_string()],
    );
    write_manifest(dir.path(), &manifest);

    let status = Command::new(bin_path())
        .arg("config")
        .arg("set")
        .arg("--template")
        .arg("https://example.com/new.git")
        .arg("--ref")
        .arg("v2.0.0")
        .arg("--profiles")
        .arg("base,extra")
        .current_dir(dir.path())
        .status()
        .expect("run repokit config set");

    assert!(status.success());

    let updated = Manifest::load_from_path(dir.path().join(".kit.toml")).expect("load manifest");

    assert_eq!(updated.template_url, "https://example.com/new.git");
    assert_eq!(updated.reference.as_deref(), Some("v2.0.0"));
    assert_eq!(
        updated.profiles,
        vec!["base".to_string(), "extra".to_string()]
    );
}

#[test]
fn config_set_allows_clearing_ref() {
    let dir = tempfile::tempdir().expect("temp dir");

    let manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    write_manifest(dir.path(), &manifest);

    let status = Command::new(bin_path())
        .arg("config")
        .arg("set")
        .arg("--ref")
        .arg("")
        .current_dir(dir.path())
        .status()
        .expect("run repokit config set --ref");

    assert!(status.success());

    let updated = Manifest::load_from_path(dir.path().join(".kit.toml")).expect("load manifest");
    assert!(updated.reference.is_none());
}

#[test]
fn config_set_rejects_empty_template() {
    let dir = tempfile::tempdir().expect("temp dir");

    let manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    write_manifest(dir.path(), &manifest);

    let output = Command::new(bin_path())
        .arg("config")
        .arg("set")
        .arg("--template")
        .arg("")
        .current_dir(dir.path())
        .output()
        .expect("run repokit config set --template");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--template cannot be empty"));
}

#[test]
fn config_set_rejects_empty_profiles() {
    let dir = tempfile::tempdir().expect("temp dir");

    let manifest = Manifest::new(
        "https://example.com/template.git".to_string(),
        Some("main".to_string()),
        vec!["base".to_string()],
    );
    write_manifest(dir.path(), &manifest);

    let output = Command::new(bin_path())
        .arg("config")
        .arg("set")
        .arg("--profiles")
        .arg("")
        .current_dir(dir.path())
        .output()
        .expect("run repokit config set --profiles");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--profiles cannot be empty"));
}
