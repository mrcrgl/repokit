use anyhow::{Result, anyhow};
use std::collections::BTreeMap;

pub const VAR_REPOSITORY_NAME: &str = "REPOSITORY_NAME";
pub const VAR_REPOSITORY_URL: &str = "REPOSITORY_URL";

pub fn vars_from_origin(origin_url: &str) -> Result<BTreeMap<String, String>> {
    let origin_url = origin_url.trim();
    if origin_url.is_empty() {
        return Err(anyhow!("git origin URL is empty"));
    }

    let repository_name = parse_owner_repo(origin_url)?;
    let mut vars = BTreeMap::new();
    vars.insert(VAR_REPOSITORY_URL.to_string(), origin_url.to_string());
    vars.insert(VAR_REPOSITORY_NAME.to_string(), repository_name);
    Ok(vars)
}

pub fn apply_template_vars(input: &str, vars: &BTreeMap<String, String>) -> String {
    let mut output = input.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        output = output.replace(&placeholder, value);
    }
    output
}

fn parse_owner_repo(origin_url: &str) -> Result<String> {
    let trimmed = origin_url.trim();

    if let Some(rest) = trimmed.strip_prefix("git@") {
        let mut parts = rest.splitn(2, ':');
        let _host = parts.next().unwrap_or_default();
        let path = parts
            .next()
            .ok_or_else(|| anyhow!("invalid git origin: {trimmed}"))?;
        return owner_repo_from_path(path);
    }

    if let Some(rest) = trimmed.strip_prefix("https://") {
        return owner_repo_from_http(rest, trimmed);
    }

    if let Some(rest) = trimmed.strip_prefix("http://") {
        return owner_repo_from_http(rest, trimmed);
    }

    Err(anyhow!("unsupported git origin: {trimmed}"))
}

fn owner_repo_from_http(rest: &str, original: &str) -> Result<String> {
    let mut parts = rest.splitn(2, '/');
    let _host = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .ok_or_else(|| anyhow!("invalid git origin: {original}"))?;
    owner_repo_from_path(path)
}

fn owner_repo_from_path(path: &str) -> Result<String> {
    let path = path.trim_end_matches(".git");
    let mut parts = path.splitn(2, '/');
    let owner = parts
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("invalid repo path: {path}"))?;
    let repo = parts
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("invalid repo path: {path}"))?;

    Ok(format!("{owner}/{repo}"))
}
