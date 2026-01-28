use std::fs;

use repokit::git::{GitCloneOptions, clone_template_to_temp};

#[test]
fn local_template_path_is_returned() {
    let dir = tempfile::tempdir().expect("temp dir");
    let template_dir = dir.path().join("template");
    fs::create_dir_all(&template_dir).expect("create template dir");

    let result = clone_template_to_temp(GitCloneOptions {
        template_url: template_dir.to_string_lossy().as_ref(),
        reference: None,
    })
    .expect("clone template");

    let expected = fs::canonicalize(&template_dir).expect("canonicalize template dir");
    assert_eq!(result.path(), expected.as_path());
}

#[test]
fn local_template_rejects_ref() {
    let dir = tempfile::tempdir().expect("temp dir");
    let template_dir = dir.path().join("template");
    fs::create_dir_all(&template_dir).expect("create template dir");

    let err = clone_template_to_temp(GitCloneOptions {
        template_url: template_dir.to_string_lossy().as_ref(),
        reference: Some("main"),
    })
    .expect_err("ref should be rejected for local template paths");

    assert!(err.to_string().contains("--ref is not supported"));
}
