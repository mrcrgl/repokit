use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

mod git;

mod apply;
mod gitignore;
mod manifest;
mod markdown;
mod merge;
mod status;
mod template;

#[derive(Parser, Debug)]
#[command(name = "repokit")]
#[command(about = "Apply and update repo templates safely", long_about = None)]
struct Cli {
    /// Template git URL
    #[arg(long)]
    template: Option<String>,

    /// Template ref (tag, branch, or commit)
    #[arg(long)]
    ref_: Option<String>,

    /// Profiles to apply (comma-separated), in order
    #[arg(long)]
    profiles: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Apply template to current repo and write manifest
    Init,
    /// Update managed files to a new template ref
    Update,
    /// Show drift status vs manifest (and optionally vs new ref)
    Status,
    /// Hygiene checks (optional, P1)
    Doctor,
}

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}

fn resolve_profile_order(raw: Option<&str>) -> Vec<String> {
    let mut profiles = vec!["base".to_string()];

    if let Some(raw) = raw {
        for item in raw.split(',') {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                profiles.push(trimmed.to_string());
            }
        }
    }

    profiles
}

fn run(cli: Cli) -> Result<()> {
    let profiles = resolve_profile_order(cli.profiles.as_deref());
    let target_root = std::env::current_dir()?;

    // Placeholder command handling for iterative build-out.
    match cli.command {
        Commands::Init => {
            println!("repokit init (scaffold)");
            println!(
                "template={:?} ref={:?} profiles={:?}",
                cli.template, cli.ref_, cli.profiles
            );
            let repo = fetch_template_required(&cli)?;
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
            let report = apply::apply_init_with_skip(&layout, &target_root, &skip_paths)?;
            let mut manifest = manifest::Manifest::new(
                cli.template.clone().unwrap_or_default(),
                cli.ref_.clone(),
                profiles.clone(),
            );
            for entry in &report.entries {
                manifest.insert_file_hash(entry.relative_path.clone(), entry.source_hash.clone());
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
                fs::write(&target_path, merged.as_bytes())?;
                let hash = manifest::hash_bytes(merged.as_bytes());
                manifest.insert_file_hash(relative_path.clone(), hash);
            }
            manifest.write_to_path(target_root.join(".kit.toml"))?;
            println!("resolved profiles={profiles:?}");
            println!("overlay files={}", layout.files.len());
            println!("manifest entries={}", manifest.files.len());
            if report.has_conflicts() {
                eprintln!("conflicts detected; wrote .new files");
                std::process::exit(2);
            }
        }
        Commands::Update => {
            println!("repokit update (scaffold)");
            println!(
                "template={:?} ref={:?} profiles={:?}",
                cli.template, cli.ref_, cli.profiles
            );
            let repo = fetch_template_required(&cli)?;
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
            let manifest_path = target_root.join(".kit.toml");
            let existing = manifest::Manifest::load_from_path(&manifest_path)?;
            let report =
                apply::apply_update_with_skip(&layout, &target_root, &existing, &skip_paths)?;
            let mut manifest = manifest::Manifest::new(
                cli.template.clone().unwrap_or_default(),
                cli.ref_.clone(),
                profiles.clone(),
            );
            for entry in &report.entries {
                manifest.insert_file_hash(entry.relative_path.clone(), entry.source_hash.clone());
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
                fs::write(&target_path, merged.as_bytes())?;
                let hash = manifest::hash_bytes(merged.as_bytes());
                manifest.insert_file_hash(relative_path.clone(), hash);
            }
            manifest.write_to_path(&manifest_path)?;
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

            if !report.is_clean() {
                std::process::exit(2);
            }
        }
        Commands::Doctor => {
            println!("repokit doctor (scaffold)");
        }
    }

    Ok(())
}

fn fetch_template_required(cli: &Cli) -> Result<git::ClonedRepo> {
    let template_url = cli
        .template
        .as_deref()
        .ok_or_else(|| anyhow!("--template is required for this command"))?;

    git::clone_template_to_temp(git::GitCloneOptions {
        template_url,
        reference: cli.ref_.as_deref(),
    })
}

fn fetch_template_optional(cli: &Cli) -> Result<Option<git::ClonedRepo>> {
    let template_url = match cli.template.as_deref() {
        Some(url) => url,
        None => return Ok(None),
    };

    let repo = git::clone_template_to_temp(git::GitCloneOptions {
        template_url,
        reference: cli.ref_.as_deref(),
    })?;

    Ok(Some(repo))
}
