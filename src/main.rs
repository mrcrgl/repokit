use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

mod git;

mod apply;
mod github;
mod gitignore;
mod manifest;
mod markdown;
mod merge;
mod status;
mod template;
mod template_vars;

#[derive(Parser, Debug)]
#[command(name = "repokit")]
#[command(about = "Apply and update repo templates safely", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Apply template to current repo and write manifest
    Init {
        /// Template git URL
        #[arg(long)]
        template: Option<String>,

        /// Template ref (tag, branch, or commit)
        #[arg(long)]
        ref_: Option<String>,

        /// Profiles to apply (comma-separated), in order
        #[arg(long)]
        profiles: Option<String>,

        /// Overwrite conflicting files instead of writing .new
        #[arg(long)]
        force: bool,
    },
    /// Update managed files to a new template ref
    Update {
        /// Overwrite conflicting files instead of writing .new
        #[arg(long)]
        force: bool,
    },
    /// Show drift status vs manifest
    Status,
    /// Manage repokit configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Hygiene checks (optional, P1)
    Doctor,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Print current config
    Show,
    /// Update config values
    Set {
        /// Template git URL
        #[arg(long)]
        template: Option<String>,
        /// Template ref (tag, branch, or commit)
        #[arg(long)]
        ref_: Option<String>,
        /// Profiles to apply (comma-separated), in order
        #[arg(long)]
        profiles: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}

fn resolve_profile_order(raw: Option<&str>) -> Vec<String> {
    let mut profiles = Vec::new();
    let mut seen = BTreeSet::new();

    let base = "base".to_string();
    if seen.insert(base.clone()) {
        profiles.push(base);
    }

    if let Some(raw) = raw {
        for item in raw.split(',') {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                let value = trimmed.to_string();
                if seen.insert(value.clone()) {
                    profiles.push(value);
                }
            }
        }
    }

    profiles
}

fn run(cli: Cli) -> Result<()> {
    let target_root = std::env::current_dir()?;

    // Placeholder command handling for iterative build-out.
    match cli.command {
        Commands::Init {
            template,
            ref_,
            profiles,
            force,
        } => {
            println!("repokit init (scaffold)");
            println!(
                "template={:?} ref={:?} profiles={:?}",
                template, ref_, profiles
            );
            let template_url = template
                .as_deref()
                .ok_or_else(|| anyhow!("--template is required for this command"))?;
            let manifest_path = target_root.join(".kit.toml");
            if manifest_path.exists() {
                return Err(anyhow!(
                    "manifest already exists; run repokit config set instead"
                ));
            }
            let profiles = resolve_profile_order(profiles.as_deref());
            let template_vars = resolve_template_vars(&target_root);
            let repo = fetch_template_required(template_url, ref_.as_deref())?;
            println!("template repo cloned to {:?}", repo.path());
            let layout = template::TemplateLayout::load(repo.path(), &profiles)?;
            let merge_configs = template::merge_config_by_profile(&layout.profiles)?;
            let mut compiled_rules = Vec::new();
            for config in merge_configs.values() {
                compiled_rules.extend(config.compile()?);
            }
            let mut merged_paths = BTreeSet::new();
            for overlay in &layout.files {
                if merge::match_strategy(&compiled_rules, &overlay.relative_path).is_some() {
                    merged_paths.insert(overlay.relative_path.clone());
                }
            }
            let skip_paths: Vec<PathBuf> = merged_paths.iter().cloned().collect();
            let report = apply::apply_init_with_skip(&layout, &target_root, &skip_paths, force)?;
            let mut manifest = manifest::Manifest::new(
                template.clone().unwrap_or_default(),
                ref_.clone(),
                profiles.clone(),
            );
            for entry in &report.entries {
                let target_path = target_root.join(&entry.relative_path);
                let bytes = fs::read(&target_path).with_context(|| {
                    format!("failed to read target file {}", target_path.display())
                })?;
                let final_bytes =
                    if let Some(updated) = apply_template_vars_to_bytes(&bytes, &template_vars)? {
                        fs::write(&target_path, &updated).with_context(|| {
                            format!("failed to write target file {}", target_path.display())
                        })?;
                        updated
                    } else {
                        bytes
                    };
                let hash = manifest::hash_bytes(&final_bytes);
                manifest.insert_file_hash(entry.relative_path.clone(), hash);
            }
            for relative_path in &merged_paths {
                let strategy = match merge::match_strategy(&compiled_rules, relative_path) {
                    Some(strategy) => strategy,
                    None => continue,
                };
                let managed_lines =
                    template::file_lines_by_profile(&layout.profiles, relative_path)?;
                let target_path = target_root.join(relative_path);
                let existing = fs::read_to_string(&target_path).ok();
                let merged = match strategy {
                    merge::MergeStrategy::Gitignore => {
                        gitignore::merge_gitignore(existing.as_deref(), &managed_lines, &profiles)
                    }
                    merge::MergeStrategy::MarkdownBlock => markdown::merge_markdown_blocks(
                        existing.as_deref(),
                        &managed_lines,
                        &profiles,
                    ),
                };
                let merged = template_vars::apply_template_vars(&merged, &template_vars);
                fs::write(&target_path, merged.as_bytes())?;
                let hash = manifest::hash_bytes(merged.as_bytes());
                manifest.insert_file_hash(relative_path.clone(), hash);
            }
            manifest.write_to_path(target_root.join(".kit.toml"))?;
            sync_github_labels(&target_root, &layout, &profiles)?;
            println!("resolved profiles={profiles:?}");
            println!("overlay files={}", layout.files.len());
            println!("manifest entries={}", manifest.files.len());
            if report.has_conflicts() {
                eprintln!("conflicts detected; wrote .new files");
                std::process::exit(2);
            }
        }
        Commands::Update { force } => {
            println!("repokit update (scaffold)");
            let manifest_path = target_root.join(".kit.toml");
            let existing = manifest::Manifest::load_from_path(&manifest_path)?;
            if existing.template_url.trim().is_empty() {
                return Err(anyhow!(
                    "manifest template_url is empty; run repokit init or repokit config set"
                ));
            }
            let profiles = existing.profiles.clone();
            let template_vars = resolve_template_vars(&target_root);
            let repo = git::clone_template_to_temp(git::GitCloneOptions {
                template_url: existing.template_url.as_str(),
                reference: existing.reference.as_deref(),
            })?;
            println!("template repo cloned to {:?}", repo.path());
            let layout = template::TemplateLayout::load(repo.path(), &profiles)?;
            let merge_configs = template::merge_config_by_profile(&layout.profiles)?;
            let mut compiled_rules = Vec::new();
            for config in merge_configs.values() {
                compiled_rules.extend(config.compile()?);
            }
            let mut merged_paths = BTreeSet::new();
            for overlay in &layout.files {
                if merge::match_strategy(&compiled_rules, &overlay.relative_path).is_some() {
                    merged_paths.insert(overlay.relative_path.clone());
                }
            }
            let skip_paths: Vec<PathBuf> = merged_paths.iter().cloned().collect();
            let report = apply::apply_update_with_skip(
                &layout,
                &target_root,
                &existing,
                &skip_paths,
                force,
            )?;
            let mut manifest = manifest::Manifest::new(
                existing.template_url.clone(),
                existing.reference.clone(),
                profiles.clone(),
            );
            for entry in &report.entries {
                let target_path = target_root.join(&entry.relative_path);
                let bytes = fs::read(&target_path).with_context(|| {
                    format!("failed to read target file {}", target_path.display())
                })?;
                let final_bytes =
                    if let Some(updated) = apply_template_vars_to_bytes(&bytes, &template_vars)? {
                        fs::write(&target_path, &updated).with_context(|| {
                            format!("failed to write target file {}", target_path.display())
                        })?;
                        updated
                    } else {
                        bytes
                    };
                let hash = manifest::hash_bytes(&final_bytes);
                manifest.insert_file_hash(entry.relative_path.clone(), hash);
            }
            for relative_path in &merged_paths {
                let strategy = match merge::match_strategy(&compiled_rules, relative_path) {
                    Some(strategy) => strategy,
                    None => continue,
                };
                let managed_lines =
                    template::file_lines_by_profile(&layout.profiles, relative_path)?;
                let target_path = target_root.join(relative_path);
                let existing = fs::read_to_string(&target_path).ok();
                let merged = match strategy {
                    merge::MergeStrategy::Gitignore => {
                        gitignore::merge_gitignore(existing.as_deref(), &managed_lines, &profiles)
                    }
                    merge::MergeStrategy::MarkdownBlock => markdown::merge_markdown_blocks(
                        existing.as_deref(),
                        &managed_lines,
                        &profiles,
                    ),
                };
                let merged = template_vars::apply_template_vars(&merged, &template_vars);
                fs::write(&target_path, merged.as_bytes())?;
                let hash = manifest::hash_bytes(merged.as_bytes());
                manifest.insert_file_hash(relative_path.clone(), hash);
            }
            manifest.write_to_path(&manifest_path)?;
            sync_github_labels(&target_root, &layout, &profiles)?;
            println!("resolved profiles={profiles:?}");
            println!("overlay files={}", layout.files.len());
            println!("manifest entries={}", manifest.files.len());
            if report.has_conflicts() {
                eprintln!("conflicts detected; wrote .new files");
                std::process::exit(2);
            }
        }
        Commands::Status => {
            println!("repokit status");
            let manifest_path = target_root.join(".kit.toml");
            let manifest = manifest::Manifest::load_from_path(&manifest_path)?;
            let report = status::compare_to_manifest(&target_root, &manifest)?;

            println!("missing={}", report.missing.len());
            println!("modified={}", report.modified.len());
            println!("extra={}", report.extra.len());

            for path in &report.missing {
                println!("missing: {}", path.display());
            }
            for path in &report.modified {
                println!("modified: {}", path.display());
            }
            for path in &report.extra {
                println!("extra: {}", path.display());
            }

            report_github_label_status(&target_root, &manifest)?;
            report_template_status(&target_root, &manifest)?;

            if !report.is_clean() {
                std::process::exit(2);
            }
        }
        Commands::Config { command } => {
            let manifest_path = target_root.join(".kit.toml");
            let mut manifest = manifest::Manifest::load_from_path(&manifest_path)?;
            match command {
                ConfigCommand::Show => {
                    println!("template_url={}", manifest.template_url);
                    println!("ref={}", manifest.reference.as_deref().unwrap_or_default());
                    println!("profiles={}", manifest.profiles.join(","));
                }
                ConfigCommand::Set {
                    template,
                    ref_,
                    profiles,
                } => {
                    if let Some(value) = template {
                        if value.trim().is_empty() {
                            return Err(anyhow!("--template cannot be empty"));
                        }
                        manifest.template_url = value;
                    }
                    if let Some(value) = ref_ {
                        if value.trim().is_empty() {
                            manifest.reference = None;
                        } else {
                            manifest.reference = Some(value);
                        }
                    }
                    if let Some(value) = profiles {
                        if value.trim().is_empty() {
                            return Err(anyhow!("--profiles cannot be empty"));
                        }
                        manifest.profiles = resolve_profile_order(Some(value.as_str()));
                    }
                    manifest.write_to_path(&manifest_path)?;
                    println!("config updated");
                }
            }
        }
        Commands::Doctor => {
            println!("repokit doctor (scaffold)");
        }
    }

    Ok(())
}

fn sync_github_labels(
    target_root: &Path,
    layout: &template::TemplateLayout,
    profiles: &[String],
) -> Result<()> {
    let merged = load_template_labels(layout, profiles)?;
    if merged.labels.is_empty() {
        return Ok(());
    }

    warn_duplicate_labels(&merged.duplicates);

    let repo = match repo_from_origin(target_root) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("warning: skipping label sync: {:#}", err);
            return Ok(());
        }
    };

    let auth = match github::detect_auth() {
        Ok(github::AuthState::Available(auth)) => auth,
        Ok(github::AuthState::Unavailable) => {
            eprintln!("warning: no GitHub auth available; skipping label sync");
            return Ok(());
        }
        Err(err) => {
            eprintln!("warning: failed to detect GitHub auth: {:#}", err);
            return Ok(());
        }
    };

    let client = github::GitHubClient::new(auth);
    let existing = client.list_labels(&repo)?;
    let missing = github::missing_labels(&merged.labels, &existing);

    if missing.is_empty() {
        println!("labels: up to date");
        return Ok(());
    }

    for label in &missing {
        client.create_label(&repo, label)?;
    }

    println!("labels: added {}", missing.len());
    Ok(())
}

fn report_github_label_status(target_root: &Path, manifest: &manifest::Manifest) -> Result<()> {
    let Some(layout) = load_template_from_manifest(manifest)? else {
        return Ok(());
    };

    let merged = load_template_labels(&layout, &manifest.profiles)?;
    if merged.labels.is_empty() {
        return Ok(());
    }

    warn_duplicate_labels(&merged.duplicates);

    let repo = match repo_from_origin(target_root) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("warning: skipping label status: {:#}", err);
            return Ok(());
        }
    };

    let auth = match github::detect_auth() {
        Ok(github::AuthState::Available(auth)) => auth,
        Ok(github::AuthState::Unavailable) => {
            eprintln!("warning: no GitHub auth available; skipping label status");
            return Ok(());
        }
        Err(err) => {
            eprintln!("warning: failed to detect GitHub auth: {:#}", err);
            return Ok(());
        }
    };

    let client = github::GitHubClient::new(auth);
    let existing = client.list_labels(&repo)?;
    let missing = github::missing_labels(&merged.labels, &existing);
    let mismatches = github::color_mismatches(&merged.labels, &existing);

    println!("labels: missing={}", missing.len());
    for label in &missing {
        println!("label missing: {}", label.name);
    }

    if !mismatches.is_empty() {
        println!("labels: color mismatches={}", mismatches.len());
        for (desired, actual) in mismatches {
            println!(
                "label color mismatch: {} desired={} actual={}",
                desired.name, desired.color, actual.color
            );
        }
    }

    Ok(())
}

fn load_template_labels(
    layout: &template::TemplateLayout,
    profiles: &[String],
) -> Result<github::MergeResult> {
    let labels_by_profile = github::labels_by_profile(&layout.profiles)?;
    Ok(github::merge_labels_by_name(&labels_by_profile, profiles))
}

fn load_template_from_manifest(
    manifest: &manifest::Manifest,
) -> Result<Option<template::TemplateLayout>> {
    if manifest.template_url.trim().is_empty() || manifest.profiles.is_empty() {
        return Ok(None);
    }

    let repo = git::clone_template_to_temp(git::GitCloneOptions {
        template_url: manifest.template_url.as_str(),
        reference: manifest.reference.as_deref(),
    })?;
    let layout = template::TemplateLayout::load(repo.path(), &manifest.profiles)?;
    Ok(Some(layout))
}

fn repo_from_origin(target_root: &Path) -> Result<github::GitHubRepo> {
    let origin = git::origin_url(target_root)?;
    github::parse_repo_from_origin(&origin)
}

fn warn_duplicate_labels(duplicates: &[String]) {
    if !duplicates.is_empty() {
        eprintln!(
            "warning: duplicate label definitions for {}",
            duplicates.join(", ")
        );
    }
}

fn resolve_template_vars(target_root: &Path) -> BTreeMap<String, String> {
    match git::origin_url(target_root).and_then(|origin| template_vars::vars_from_origin(&origin)) {
        Ok(vars) => vars,
        Err(err) => {
            eprintln!("warning: template vars disabled: {:#}", err);
            BTreeMap::new()
        }
    }
}

fn apply_template_vars_to_bytes(
    bytes: &[u8],
    vars: &BTreeMap<String, String>,
) -> Result<Option<Vec<u8>>> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    let rendered = template_vars::apply_template_vars(text, vars);
    if rendered == text {
        return Ok(None);
    }
    Ok(Some(rendered.into_bytes()))
}

fn report_template_status(target_root: &Path, manifest: &manifest::Manifest) -> Result<()> {
    if manifest.template_url.trim().is_empty() {
        return Ok(());
    }

    let repo = git::clone_template_to_temp(git::GitCloneOptions {
        template_url: manifest.template_url.as_str(),
        reference: manifest.reference.as_deref(),
    })?;
    let layout = template::TemplateLayout::load(repo.path(), &manifest.profiles)?;
    let merge_configs = template::merge_config_by_profile(&layout.profiles)?;
    let mut compiled_rules = Vec::new();
    for config in merge_configs.values() {
        compiled_rules.extend(config.compile()?);
    }

    let desired =
        build_desired_template_bytes(target_root, &layout, &compiled_rules, &manifest.profiles)?;
    let report = status::compare_to_template(target_root, &desired)?;

    println!("template missing={}", report.missing.len());
    println!("template modified={}", report.modified.len());
    println!("template extra={}", report.extra.len());

    for path in &report.missing {
        println!("template missing: {}", path.display());
    }
    for path in &report.modified {
        println!("template modified: {}", path.display());
    }
    for path in &report.extra {
        println!("template extra: {}", path.display());
    }

    Ok(())
}

fn build_desired_template_bytes(
    target_root: &Path,
    layout: &template::TemplateLayout,
    compiled_rules: &[merge::CompiledMergeRule],
    profiles: &[String],
) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let template_vars = resolve_template_vars(target_root);
    let mut desired = template::template_file_bytes(layout, compiled_rules)?;
    for bytes in desired.values_mut() {
        if let Some(updated) = apply_template_vars_to_bytes(bytes, &template_vars)? {
            *bytes = updated;
        }
    }

    for overlay in &layout.files {
        let Some(strategy) = merge::match_strategy(compiled_rules, &overlay.relative_path) else {
            continue;
        };

        let managed_lines =
            template::file_lines_by_profile(&layout.profiles, &overlay.relative_path)?;
        let target_path = target_root.join(&overlay.relative_path);
        let existing = fs::read_to_string(&target_path).ok();
        let merged = match strategy {
            merge::MergeStrategy::Gitignore => {
                gitignore::merge_gitignore(existing.as_deref(), &managed_lines, profiles)
            }
            merge::MergeStrategy::MarkdownBlock => {
                markdown::merge_markdown_blocks(existing.as_deref(), &managed_lines, profiles)
            }
        };
        let merged = template_vars::apply_template_vars(&merged, &template_vars);

        desired.insert(overlay.relative_path.clone(), merged.into_bytes());
    }

    Ok(desired)
}

fn fetch_template_required(template_url: &str, reference: Option<&str>) -> Result<git::ClonedRepo> {
    git::clone_template_to_temp(git::GitCloneOptions {
        template_url,
        reference,
    })
}
