use crate::template::Profile;
use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LabelFile {
    pub label: Vec<Label>,
}

#[derive(Debug, Clone)]
pub struct MergeResult {
    pub labels: Vec<Label>,
    pub duplicates: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone)]
pub enum Auth {
    Token(String),
    GhCli,
}

#[derive(Debug, Clone)]
pub enum AuthState {
    Available(Auth),
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct GitHubClient {
    auth: Auth,
}

impl GitHubClient {
    pub fn new(auth: Auth) -> Self {
        Self { auth }
    }

    pub fn list_labels(&self, repo: &GitHubRepo) -> Result<Vec<Label>> {
        let endpoint = format!("/repos/{}/{}/labels", repo.owner, repo.repo);
        let lines = self.run_gh_api_lines(
            &[
                "api",
                "--paginate",
                endpoint.as_str(),
                "--jq",
                ".[] | [.name, .color, (.description // \"\")] | @tsv",
            ],
            None,
        )?;

        let mut labels = Vec::new();
        for line in lines {
            let mut parts = line.split('\t');
            let name = parts.next().map(str::to_string).filter(|s| !s.is_empty());
            let color = parts.next().map(str::to_string).filter(|s| !s.is_empty());
            let description = parts.next().map(str::to_string).filter(|s| !s.is_empty());

            let Some(name) = name else { continue };
            let Some(color) = color else { continue };
            labels.push(Label {
                name,
                color,
                description,
            });
        }

        Ok(labels)
    }

    pub fn create_label(&self, repo: &GitHubRepo, label: &Label) -> Result<()> {
        let endpoint = format!("/repos/{}/{}/labels", repo.owner, repo.repo);
        let mut args = vec![
            "api".to_string(),
            "-X".to_string(),
            "POST".to_string(),
            endpoint,
            "-f".to_string(),
            format!("name={}", label.name),
            "-f".to_string(),
            format!("color={}", normalize_color(&label.color)?),
        ];

        if let Some(description) = &label.description
            && !description.trim().is_empty()
        {
            args.push("-f".to_string());
            args.push(format!("description={}", description));
        }

        self.run_gh_api(&args, None)?;

        Ok(())
    }

    fn run_gh_api(&self, args: &[String], extra_env: Option<(&str, &str)>) -> Result<()> {
        let mut cmd = Command::new("gh");
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        if let Some((key, value)) = extra_env {
            cmd.env(key, value);
        }

        match &self.auth {
            Auth::Token(token) => {
                cmd.env("GITHUB_TOKEN", token);
            }
            Auth::GhCli => {}
        }

        let output = cmd.output().context("failed to execute gh api")?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("gh api failed: {}", stderr.trim()))
        }
    }

    fn run_gh_api_lines(
        &self,
        args: &[&str],
        extra_env: Option<(&str, &str)>,
    ) -> Result<Vec<String>> {
        let mut cmd = Command::new("gh");
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some((key, value)) = extra_env {
            cmd.env(key, value);
        }

        match &self.auth {
            Auth::Token(token) => {
                cmd.env("GITHUB_TOKEN", token);
            }
            Auth::GhCli => {}
        }

        let output = cmd.output().context("failed to execute gh api")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("gh api failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect())
    }
}

pub fn detect_auth() -> Result<AuthState> {
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.trim().is_empty()
    {
        return Ok(AuthState::Available(Auth::Token(token)));
    }

    if gh_auth_available()? {
        return Ok(AuthState::Available(Auth::GhCli));
    }

    Ok(AuthState::Unavailable)
}

pub fn gh_auth_available() -> Result<bool> {
    let output = Command::new("gh")
        .args(["auth", "status"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

pub fn parse_repo_from_origin(origin_url: &str) -> Result<GitHubRepo> {
    let trimmed = origin_url.trim();

    if let Some(rest) = trimmed.strip_prefix("git@") {
        let mut parts = rest.splitn(2, ':');
        let host = parts.next().unwrap_or_default();
        let path = parts
            .next()
            .ok_or_else(|| anyhow!("invalid git origin: {trimmed}"))?;
        if host != "github.com" {
            bail!("unsupported git host: {}", host);
        }
        return parse_owner_repo(path);
    }

    if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        return parse_owner_repo(rest);
    }

    if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        return parse_owner_repo(rest);
    }

    bail!("unsupported git origin: {}", trimmed)
}

fn parse_owner_repo(path: &str) -> Result<GitHubRepo> {
    let path = path.trim_end_matches(".git");
    let mut parts = path.splitn(2, '/');
    let owner = parts
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("invalid repo path: {}", path))?;
    let repo = parts
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("invalid repo path: {}", path))?;

    Ok(GitHubRepo { owner, repo })
}

pub fn labels_by_profile(profiles: &[Profile]) -> Result<BTreeMap<String, Vec<Label>>> {
    let mut result = BTreeMap::new();

    for profile in profiles {
        let path = profile.path.join("github").join("labels.toml");
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read labels {}", path.display()))?;
            let parsed: LabelFile = toml::from_str(&content)
                .with_context(|| format!("failed to parse labels {}", path.display()))?;
            result.insert(profile.name.clone(), parsed.label);
        } else {
            result.insert(profile.name.clone(), Vec::new());
        }
    }

    Ok(result)
}

pub fn merge_labels_by_name(
    labels_by_profile: &BTreeMap<String, Vec<Label>>,
    profile_order: &[String],
) -> MergeResult {
    let mut seen = BTreeSet::new();
    let mut labels = Vec::new();
    let mut duplicates = Vec::new();

    for profile in profile_order {
        if let Some(profile_labels) = labels_by_profile.get(profile) {
            for label in profile_labels {
                if seen.insert(label.name.clone()) {
                    labels.push(label.clone());
                } else {
                    duplicates.push(label.name.clone());
                }
            }
        }
    }

    MergeResult { labels, duplicates }
}

pub fn missing_labels(desired: &[Label], existing: &[Label]) -> Vec<Label> {
    let existing_names: BTreeSet<&str> = existing.iter().map(|label| label.name.as_str()).collect();
    desired
        .iter()
        .filter(|label| !existing_names.contains(label.name.as_str()))
        .cloned()
        .collect()
}

pub fn color_mismatches(desired: &[Label], existing: &[Label]) -> Vec<(Label, Label)> {
    let mut existing_by_name = BTreeMap::new();
    for label in existing {
        existing_by_name.insert(label.name.as_str(), label);
    }

    let mut mismatches = Vec::new();
    for desired_label in desired {
        if let Some(existing_label) = existing_by_name.get(desired_label.name.as_str())
            && normalize_color(&desired_label.color).ok()
                != normalize_color(&existing_label.color).ok()
        {
            mismatches.push((desired_label.clone(), (*existing_label).clone()));
        }
    }

    mismatches
}

fn normalize_color(color: &str) -> Result<String> {
    let color = color.trim().trim_start_matches('#');
    if color.len() != 6 {
        bail!("label color must be 6 hex chars, got {}", color);
    }
    if !color.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("label color must be hex, got {}", color);
    }
    Ok(color.to_ascii_uppercase())
}
