use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    #[serde(default)]
    pub rules: Vec<MergeRule>,
}

impl MergeConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read merge config {}", path.display()))?;
        let config = toml::from_str(&content)
            .with_context(|| format!("failed to parse merge config {}", path.display()))?;
        Ok(config)
    }

    pub fn compile(&self) -> Result<Vec<CompiledMergeRule>> {
        self.rules
            .iter()
            .map(CompiledMergeRule::from_rule)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRule {
    #[serde(default)]
    pub name: Option<String>,
    pub strategy: MergeStrategy,
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    Gitignore,
    MarkdownBlock,
}

#[derive(Debug, Clone)]
pub struct CompiledMergeRule {
    pub strategy: MergeStrategy,
    matcher: GlobSet,
}

impl CompiledMergeRule {
    pub fn from_rule(rule: &MergeRule) -> Result<Self> {
        let matcher = build_globset(&rule.patterns)?;
        Ok(Self {
            strategy: rule.strategy.clone(),
            matcher,
        })
    }

    pub fn matches(&self, path: &Path) -> bool {
        self.matcher.is_match(path)
    }
}

pub fn match_strategy<'a>(
    compiled: &'a [CompiledMergeRule],
    path: &Path,
) -> Option<&'a MergeStrategy> {
    compiled
        .iter()
        .rev()
        .find(|rule| rule.matches(path))
        .map(|rule| &rule.strategy)
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).with_context(|| format!("invalid merge pattern: {pattern}"))?;
        builder.add(glob);
    }
    builder.build().context("failed to compile merge patterns")
}
