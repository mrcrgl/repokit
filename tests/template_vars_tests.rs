use std::collections::BTreeMap;

use repokit::template_vars::{
    VAR_REPOSITORY_NAME, VAR_REPOSITORY_URL, apply_template_vars, vars_from_origin,
};

#[test]
fn vars_from_origin_parses_https_url() {
    let vars = vars_from_origin("https://github.com/acme/widgets.git").expect("vars");
    assert_eq!(
        vars.get(VAR_REPOSITORY_NAME).map(String::as_str),
        Some("acme/widgets")
    );
    assert_eq!(
        vars.get(VAR_REPOSITORY_URL).map(String::as_str),
        Some("https://github.com/acme/widgets.git")
    );
}

#[test]
fn vars_from_origin_parses_ssh_url() {
    let vars = vars_from_origin("git@github.com:acme/widgets.git").expect("vars");
    assert_eq!(
        vars.get(VAR_REPOSITORY_NAME).map(String::as_str),
        Some("acme/widgets")
    );
    assert_eq!(
        vars.get(VAR_REPOSITORY_URL).map(String::as_str),
        Some("git@github.com:acme/widgets.git")
    );
}

#[test]
fn vars_from_origin_rejects_empty() {
    let err = vars_from_origin("   ").expect_err("should error");
    assert!(err.to_string().contains("git origin URL is empty"));
}

#[test]
fn vars_from_origin_rejects_unsupported_scheme() {
    let err = vars_from_origin("ssh://github.com/acme/widgets.git").expect_err("should error");
    assert!(err.to_string().contains("unsupported git origin"));
}

#[test]
fn apply_template_vars_replaces_placeholders() {
    let mut vars = BTreeMap::new();
    vars.insert(VAR_REPOSITORY_NAME.to_string(), "acme/widgets".to_string());
    vars.insert(
        VAR_REPOSITORY_URL.to_string(),
        "git@github.com:acme/widgets.git".to_string(),
    );

    let input = "Name={{REPOSITORY_NAME}}\nUrl={{REPOSITORY_URL}}\n";
    let output = apply_template_vars(input, &vars);

    assert_eq!(
        output,
        "Name=acme/widgets\nUrl=git@github.com:acme/widgets.git\n"
    );
}

#[test]
fn apply_template_vars_leaves_unknown_placeholders() {
    let vars = BTreeMap::new();
    let input = "Name={{REPOSITORY_NAME}}\n";
    let output = apply_template_vars(input, &vars);

    assert_eq!(output, input);
}
