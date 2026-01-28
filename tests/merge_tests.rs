use std::collections::BTreeMap;

use repokit_cli::gitignore::merge_gitignore;
use repokit_cli::markdown::merge_markdown_blocks;

#[test]
fn merge_gitignore_preserves_user_lines_and_replaces_blocks() {
    let existing = r#"
target/
# BEGIN repokit profile: base
old
# END repokit profile: base
# user keep
"#;

    let mut managed = BTreeMap::new();
    managed.insert(
        "base".to_string(),
        vec!["/dist".to_string(), "/out".to_string()],
    );
    managed.insert("dev".to_string(), vec!["/tmp".to_string()]);

    let output = merge_gitignore(
        Some(existing),
        &managed,
        &["base".to_string(), "dev".to_string()],
    );

    let expected = r#"target/
# user keep

# BEGIN repokit profile: base
/dist
/out
# END repokit profile: base

# BEGIN repokit profile: dev
/tmp
# END repokit profile: dev
"#;

    assert_eq!(output, expected);
}

#[test]
fn merge_gitignore_no_existing_content() {
    let mut managed = BTreeMap::new();
    managed.insert("base".to_string(), vec!["/dist".to_string()]);

    let output = merge_gitignore(None, &managed, &["base".to_string()]);

    let expected = r#"# BEGIN repokit profile: base
/dist
# END repokit profile: base
"#;

    assert_eq!(output, expected);
}

#[test]
fn merge_markdown_preserves_user_content_and_replaces_blocks() {
    let existing = r#"# Title

Intro text.

<!-- BEGIN repokit profile: base -->
old
<!-- END repokit profile: base -->

Footer.
"#;

    let mut managed = BTreeMap::new();
    managed.insert(
        "base".to_string(),
        vec!["- DoD: tests".to_string(), "- DoD: fmt".to_string()],
    );
    managed.insert("dev".to_string(), vec!["- Note: local only".to_string()]);

    let output = merge_markdown_blocks(
        Some(existing),
        &managed,
        &["base".to_string(), "dev".to_string()],
    );

    let expected = r#"# Title

Intro text.

Footer.

<!-- BEGIN repokit profile: base -->
- DoD: tests
- DoD: fmt
<!-- END repokit profile: base -->

<!-- BEGIN repokit profile: dev -->
- Note: local only
<!-- END repokit profile: dev -->
"#;

    assert_eq!(output, expected);
}

#[test]
fn merge_markdown_no_existing_content() {
    let mut managed = BTreeMap::new();
    managed.insert("base".to_string(), vec!["- Rule: keep".to_string()]);

    let output = merge_markdown_blocks(None, &managed, &["base".to_string()]);

    let expected = r#"<!-- BEGIN repokit profile: base -->
- Rule: keep
<!-- END repokit profile: base -->
"#;

    assert_eq!(output, expected);
}
