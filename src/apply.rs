use crate::manifest;
use crate::template::TemplateLayout;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppliedFile {
    pub relative_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ApplyReport {
    pub entries: Vec<AppliedFile>,
    pub conflicts: Vec<PathBuf>,
}

impl ApplyReport {
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

pub fn apply_init_with_skip(
    layout: &TemplateLayout,
    target_root: &Path,
    skip_paths: &[PathBuf],
    force_overwrite: bool,
) -> Result<ApplyReport> {
    let mut report = ApplyReport::default();

    for overlay in &layout.files {
        let source_path = &overlay.source_path;
        let relative_path = overlay.relative_path.clone();
        if is_skipped(&relative_path, skip_paths) {
            continue;
        }
        let target_path = target_root.join(&relative_path);

        let source_bytes = fs::read(source_path)
            .with_context(|| format!("failed to read template file {}", source_path.display()))?;

        if target_path.exists() {
            let target_bytes = fs::read(&target_path)
                .with_context(|| format!("failed to read target file {}", target_path.display()))?;
            if target_bytes == source_bytes {
                report.entries.push(AppliedFile { relative_path });
                continue;
            }

            if force_overwrite {
                write_bytes(&target_path, &source_bytes)?;
                report.entries.push(AppliedFile { relative_path });
                continue;
            }

            let new_path = conflict_path(&target_path)?;
            write_bytes(&new_path, &source_bytes)?;
            report.conflicts.push(relative_path.clone());
            report.entries.push(AppliedFile { relative_path });
            continue;
        }

        write_bytes(&target_path, &source_bytes)?;
        report.entries.push(AppliedFile { relative_path });
    }

    Ok(report)
}

pub fn apply_update_with_skip(
    layout: &TemplateLayout,
    target_root: &Path,
    manifest: &manifest::Manifest,
    skip_paths: &[PathBuf],
    force_overwrite: bool,
) -> Result<ApplyReport> {
    let mut report = ApplyReport::default();

    for overlay in &layout.files {
        let source_path = &overlay.source_path;
        let relative_path = overlay.relative_path.clone();
        if is_skipped(&relative_path, skip_paths) {
            continue;
        }
        let target_path = target_root.join(&relative_path);

        let source_bytes = fs::read(source_path)
            .with_context(|| format!("failed to read template file {}", source_path.display()))?;

        let manifest_entry = manifest
            .files
            .get(&relative_path)
            .map(|entry| entry.sha256.as_str());

        if !target_path.exists() {
            write_bytes(&target_path, &source_bytes)?;
            report.entries.push(AppliedFile { relative_path });
            continue;
        }

        let target_bytes = fs::read(&target_path)
            .with_context(|| format!("failed to read target file {}", target_path.display()))?;
        let target_hash = manifest::hash_bytes(&target_bytes);

        match manifest_entry {
            Some(expected_hash) if expected_hash == target_hash => {
                if target_bytes == source_bytes {
                    report.entries.push(AppliedFile { relative_path });
                } else {
                    write_bytes(&target_path, &source_bytes)?;
                    report.entries.push(AppliedFile { relative_path });
                }
            }
            Some(_) | None => {
                if target_bytes == source_bytes {
                    report.entries.push(AppliedFile { relative_path });
                } else if force_overwrite {
                    write_bytes(&target_path, &source_bytes)?;
                    report.entries.push(AppliedFile { relative_path });
                } else {
                    let new_path = conflict_path(&target_path)?;
                    write_bytes(&new_path, &source_bytes)?;
                    report.conflicts.push(relative_path.clone());
                    report.entries.push(AppliedFile { relative_path });
                }
            }
        }
    }

    Ok(report)
}

fn is_skipped(relative_path: &Path, skip_paths: &[PathBuf]) -> bool {
    skip_paths.iter().any(|path| path == relative_path)
}

fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    fs::write(path, data).with_context(|| format!("failed to write file {}", path.display()))?;
    Ok(())
}

fn conflict_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("path has no file name: {}", path.display()))?
        .to_string_lossy();
    let new_name = format!("{file_name}.new");
    Ok(path.with_file_name(new_name))
}
