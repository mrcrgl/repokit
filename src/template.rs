use crate::merge::{CompiledMergeRule, MergeConfig, match_strategy};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct OverlayFile {
    pub relative_path: PathBuf,
    pub source_path: PathBuf,
}

#[derive(Debug)]
pub struct TemplateLayout {
    pub profiles: Vec<Profile>,
    pub files: Vec<OverlayFile>,
}

impl TemplateLayout {
    pub fn load(root: impl AsRef<Path>, profiles: &[String]) -> Result<Self> {
        let root = root.as_ref().to_path_buf();

        if profiles.is_empty() {
            bail!("at least one profile is required");
        }
        if !profiles.iter().any(|name| name == "base") {
            bail!("profiles must include base");
        }

        let resolved_profiles = resolve_profiles(&root, profiles)?;
        let files = overlay_files(&resolved_profiles)?;

        Ok(Self {
            profiles: resolved_profiles,
            files,
        })
    }
}

pub fn resolve_profiles(root: &Path, profiles: &[String]) -> Result<Vec<Profile>> {
    let mut resolved = Vec::with_capacity(profiles.len());

    for profile in profiles {
        let profile_dir = root.join("profiles").join(profile);
        if !profile_dir.exists() {
            bail!("profile directory not found: {}", profile_dir.display());
        }
        resolved.push(Profile {
            name: profile.clone(),
            path: profile_dir,
        });
    }

    Ok(resolved)
}

pub fn overlay_files(profiles: &[Profile]) -> Result<Vec<OverlayFile>> {
    let mut overlay_map: BTreeMap<PathBuf, OverlayFile> = BTreeMap::new();

    for profile in profiles {
        let files = collect_profile_files(profile)?;
        for overlay in files {
            overlay_map.insert(overlay.relative_path.clone(), overlay);
        }
    }

    Ok(overlay_map.into_values().collect())
}

pub fn merge_config_by_profile(profiles: &[Profile]) -> Result<BTreeMap<String, MergeConfig>> {
    let mut result = BTreeMap::new();

    for profile in profiles {
        let path = profile.path.join("repokit").join("merge.toml");
        if path.exists() {
            let config = MergeConfig::load_from_path(&path)?;
            result.insert(profile.name.clone(), config);
        } else {
            result.insert(profile.name.clone(), MergeConfig { rules: Vec::new() });
        }
    }

    Ok(result)
}

pub fn file_lines_by_profile(
    profiles: &[Profile],
    relative_path: &Path,
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut result = BTreeMap::new();

    for profile in profiles {
        let path = profile.path.join(relative_path);
        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let lines = content.lines().map(|line| line.to_string()).collect();
            result.insert(profile.name.clone(), lines);
        } else {
            result.insert(profile.name.clone(), Vec::new());
        }
    }

    Ok(result)
}

pub fn template_file_bytes(
    layout: &TemplateLayout,
    compiled_rules: &[CompiledMergeRule],
) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut result = BTreeMap::new();

    for overlay in &layout.files {
        if match_strategy(compiled_rules, &overlay.relative_path).is_some() {
            continue;
        }

        let bytes = fs::read(&overlay.source_path).with_context(|| {
            format!(
                "failed to read template file {}",
                overlay.source_path.display()
            )
        })?;
        result.insert(overlay.relative_path.clone(), bytes);
    }

    Ok(result)
}

fn collect_profile_files(profile: &Profile) -> Result<Vec<OverlayFile>> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(&profile.path).follow_links(false) {
        let entry = entry.with_context(|| {
            format!(
                "failed to read profile directory {}",
                profile.path.display()
            )
        })?;

        if entry.file_type().is_dir() {
            continue;
        }

        let source_path = entry.path().to_path_buf();
        let relative_path = source_path
            .strip_prefix(&profile.path)
            .with_context(|| {
                format!(
                    "failed to compute relative path for {}",
                    source_path.display()
                )
            })?
            .to_path_buf();

        entries.push(OverlayFile {
            relative_path,
            source_path,
        });
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(entries)
}
