use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin_path() -> String {
    env!("CARGO_BIN_EXE_repokit").to_string()
}

fn write_template_file(template_root: &Path, content: &str) -> PathBuf {
    let profile_dir = template_root.join("profiles").join("base");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    let file_path = profile_dir.join("README.md");
    fs::write(&file_path, content).expect("write template file");
    file_path
}

fn read_file(path: &Path) -> String {
    fs::read_to_string(path).expect("read file")
}

#[test]
fn init_force_overwrites_conflicting_files() {
    let template_dir = tempfile::tempdir().expect("template dir");
    let target_dir = tempfile::tempdir().expect("target dir");

    write_template_file(template_dir.path(), "template v1\n");

    let target_readme = target_dir.path().join("README.md");
    fs::write(&target_readme, "local change\n").expect("write target file");

    let status = Command::new(bin_path())
        .arg("init")
        .arg("--template")
        .arg(template_dir.path())
        .arg("--profiles")
        .arg("base")
        .arg("--force")
        .current_dir(target_dir.path())
        .status()
        .expect("run repokit init");

    assert!(status.success());
    assert_eq!(read_file(&target_readme), "template v1\n");
    assert!(!target_readme.with_file_name("README.md.new").exists());
}

#[test]
fn update_force_overwrites_conflicting_files() {
    let template_dir = tempfile::tempdir().expect("template dir");
    let target_dir = tempfile::tempdir().expect("target dir");

    write_template_file(template_dir.path(), "template v1\n");

    let status = Command::new(bin_path())
        .arg("init")
        .arg("--template")
        .arg(template_dir.path())
        .arg("--profiles")
        .arg("base")
        .current_dir(target_dir.path())
        .status()
        .expect("run repokit init");

    assert!(status.success());

    let target_readme = target_dir.path().join("README.md");
    fs::write(&target_readme, "local change\n").expect("write local change");

    write_template_file(template_dir.path(), "template v2\n");

    let status = Command::new(bin_path())
        .arg("update")
        .arg("--force")
        .current_dir(target_dir.path())
        .status()
        .expect("run repokit update");

    assert!(status.success());
    assert_eq!(read_file(&target_readme), "template v2\n");
    assert!(!target_readme.with_file_name("README.md.new").exists());
}
