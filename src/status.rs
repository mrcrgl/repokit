use crate::manifest;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Default)]
pub struct StatusReport {
    pub missing: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.modified.is_empty() && self.extra.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct TemplateStatusReport {
    pub missing: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

pub fn compare_to_manifest(
    target_root: &Path,
    manifest: &manifest::Manifest,
) -> Result<StatusReport> {
    let mut report = StatusReport::default();

    for (relative_path, entry) in &manifest.files {
        let target_path = target_root.join(relative_path);
        if !target_path.exists() {
            report.missing.push(relative_path.clone());
            continue;
        }

        let current_hash = manifest::hash_file(&target_path)
            .with_context(|| format!("failed to hash {}", target_path.display()))?;
        if current_hash != entry.sha256 {
            report.modified.push(relative_path.clone());
        }
    }

    let manifest_paths: BTreeSet<PathBuf> = manifest.files.keys().cloned().collect();
    let mut extra = Vec::new();

    for entry in WalkDir::new(target_root).follow_links(false) {
        let entry = entry.with_context(|| format!("failed to walk {}", target_root.display()))?;
        if entry.file_type().is_dir() {
            continue;
        }

        let absolute_path = entry.path();
        let relative_path = absolute_path
            .strip_prefix(target_root)
            .with_context(|| format!("failed to relativize {}", absolute_path.display()))?
            .to_path_buf();

        if is_ignored_path(&relative_path) {
            continue;
        }

        if !manifest_paths.contains(&relative_path) {
            extra.push(relative_path);
        }
    }

    report.extra = extra;
    Ok(report)
}

pub fn compare_to_template(
    target_root: &Path,
    template_files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<TemplateStatusReport> {
    let mut report = TemplateStatusReport::default();

    for (relative_path, bytes) in template_files {
        let target_path = target_root.join(relative_path);
        if !target_path.exists() {
            report.missing.push(relative_path.clone());
            continue;
        }

        let target_hash = manifest::hash_file(&target_path)
            .with_context(|| format!("failed to hash {}", target_path.display()))?;
        let template_hash = manifest::hash_bytes(bytes);
        if target_hash != template_hash {
            report.modified.push(relative_path.clone());
        }
    }

    let template_paths: BTreeSet<PathBuf> = template_files.keys().cloned().collect();
    let mut extra = Vec::new();

    for entry in WalkDir::new(target_root).follow_links(false) {
        if let Err(err) = &entry
            && err
                .io_error()
                .map(|e| e.kind())
                .is_some_and(|kind| matches!(kind, io::ErrorKind::PermissionDenied))
        {
            continue;
        }
        let entry = entry.with_context(|| format!("failed to walk {}", target_root.display()))?;

        if entry.file_type().is_dir() {
            continue;
        }

        let absolute_path = entry.path();
        let relative_path = absolute_path
            .strip_prefix(target_root)
            .with_context(|| format!("failed to relativize {}", absolute_path.display()))?
            .to_path_buf();

        if is_ignored_path(&relative_path) {
            continue;
        }

        if !template_paths.contains(&relative_path) {
            extra.push(relative_path);
        }
    }

    report.extra = extra;
    Ok(report)
}

fn is_ignored_path(relative_path: &Path) -> bool {
    if relative_path == Path::new(".kit.toml") {
        return true;
    }

    let mut components = relative_path.components();
    let Some(first) = components.next() else {
        return true;
    };

    match first {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            name == ".git" || name == "target"
        }
        _ => true,
    }
}
