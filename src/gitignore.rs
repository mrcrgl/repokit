use std::collections::BTreeMap;

const BEGIN_PREFIX: &str = "# BEGIN repokit profile:";
const END_PREFIX: &str = "# END repokit profile:";

/// Merge user-maintained lines with managed blocks in a deterministic order.
/// Existing managed blocks are removed and replaced.
pub fn merge_gitignore(
    existing: Option<&str>,
    managed_by_profile: &BTreeMap<String, Vec<String>>,
    profile_order: &[String],
) -> String {
    let mut user_lines = Vec::new();
    let mut in_block = false;

    for line in existing.unwrap_or("").lines() {
        if parse_begin(line).is_some() {
            in_block = true;
            continue;
        }

        if in_block {
            if parse_end(line).is_some() {
                in_block = false;
            }
            continue;
        }

        if parse_end(line).is_some() {
            continue;
        }

        user_lines.push(line.to_string());
    }

    while user_lines
        .last()
        .map(|line| line.trim().is_empty())
        .unwrap_or(false)
    {
        user_lines.pop();
    }

    let mut output = String::new();
    for line in user_lines {
        output.push_str(&line);
        output.push('\n');
    }

    let managed = render_managed_blocks(managed_by_profile, profile_order);
    if !managed.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&managed);
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }

    output
}

fn render_managed_blocks(
    managed_by_profile: &BTreeMap<String, Vec<String>>,
    profile_order: &[String],
) -> String {
    let mut output = String::new();
    let mut first_block = true;

    for profile in profile_order {
        let lines = match managed_by_profile.get(profile) {
            Some(lines) if !lines.is_empty() => lines,
            _ => continue,
        };

        if !first_block {
            output.push('\n');
        }
        first_block = false;

        output.push_str(&format!("{BEGIN_PREFIX} {profile}\n"));
        for line in lines {
            output.push_str(line);
            output.push('\n');
        }
        output.push_str(&format!("{END_PREFIX} {profile}\n"));
    }

    output
}

fn parse_begin(line: &str) -> Option<String> {
    line.strip_prefix(BEGIN_PREFIX)
        .map(|rest| rest.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn parse_end(line: &str) -> Option<String> {
    line.strip_prefix(END_PREFIX)
        .map(|rest| rest.trim().to_string())
        .filter(|name| !name.is_empty())
}
