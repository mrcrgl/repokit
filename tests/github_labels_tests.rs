use std::collections::BTreeMap;

use repokit::github::{
    Label, color_mismatches, merge_labels_by_name, missing_labels, parse_repo_from_origin,
};

#[test]
fn parse_repo_from_origin_handles_https() {
    let repo = parse_repo_from_origin("https://github.com/example-org/example-repo.git")
        .expect("parse https origin");
    assert_eq!(repo.owner, "example-org");
    assert_eq!(repo.repo, "example-repo");
}

#[test]
fn parse_repo_from_origin_handles_ssh() {
    let repo = parse_repo_from_origin("git@github.com:example-org/example-repo.git")
        .expect("parse ssh origin");
    assert_eq!(repo.owner, "example-org");
    assert_eq!(repo.repo, "example-repo");
}

#[test]
fn merge_labels_by_name_respects_profile_order_and_reports_duplicates() {
    let mut labels_by_profile = BTreeMap::new();
    labels_by_profile.insert(
        "base".to_string(),
        vec![
            Label {
                name: "type:bug".to_string(),
                color: "D73A4A".to_string(),
                description: Some("Bug".to_string()),
            },
            Label {
                name: "type:feature".to_string(),
                color: "1D76DB".to_string(),
                description: Some("Feature".to_string()),
            },
        ],
    );
    labels_by_profile.insert(
        "extra".to_string(),
        vec![
            Label {
                name: "type:feature".to_string(),
                color: "000000".to_string(),
                description: Some("Duplicate".to_string()),
            },
            Label {
                name: "prio:p1".to_string(),
                color: "B60205".to_string(),
                description: None,
            },
        ],
    );

    let result = merge_labels_by_name(
        &labels_by_profile,
        &["base".to_string(), "extra".to_string()],
    );

    assert_eq!(result.labels.len(), 3);
    assert_eq!(result.labels[0].name, "type:bug");
    assert_eq!(result.labels[1].name, "type:feature");
    assert_eq!(result.labels[2].name, "prio:p1");
    assert_eq!(result.duplicates, vec!["type:feature".to_string()]);
}

#[test]
fn missing_labels_returns_only_absent_names() {
    let desired = vec![
        Label {
            name: "type:bug".to_string(),
            color: "D73A4A".to_string(),
            description: None,
        },
        Label {
            name: "type:feature".to_string(),
            color: "1D76DB".to_string(),
            description: None,
        },
    ];
    let existing = vec![Label {
        name: "type:bug".to_string(),
        color: "D73A4A".to_string(),
        description: Some("Bug".to_string()),
    }];

    let missing = missing_labels(&desired, &existing);

    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].name, "type:feature");
}

#[test]
fn color_mismatches_detects_case_and_hex_differences() {
    let desired = vec![
        Label {
            name: "type:bug".to_string(),
            color: "D73A4A".to_string(),
            description: None,
        },
        Label {
            name: "prio:p1".to_string(),
            color: "B60205".to_string(),
            description: None,
        },
    ];
    let existing = vec![
        Label {
            name: "type:bug".to_string(),
            color: "d73a4a".to_string(),
            description: None,
        },
        Label {
            name: "prio:p1".to_string(),
            color: "FFFFFF".to_string(),
            description: None,
        },
    ];

    let mismatches = color_mismatches(&desired, &existing);

    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].0.name, "prio:p1");
    assert_eq!(mismatches[0].0.color, "B60205");
    assert_eq!(mismatches[0].1.color, "FFFFFF");
}
