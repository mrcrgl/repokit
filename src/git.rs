use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct GitCloneOptions<'a> {
    pub template_url: &'a str,
    pub reference: Option<&'a str>,
}

#[derive(Debug)]
pub struct ClonedRepo {
    pub path: PathBuf,
}

impl ClonedRepo {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn clone_template_to_temp(opts: GitCloneOptions<'_>) -> Result<ClonedRepo> {
    ensure_git_available()?;

    let dir = tempfile::tempdir().context("create temp dir for git clone")?;
    let dest = dir.path().to_path_buf();

    run_git(
        None,
        &[
            "clone",
            "--quiet",
            "--depth",
            "1",
            opts.template_url,
            dest.to_string_lossy().as_ref(),
        ],
    )
    .with_context(|| format!("git clone failed for {}", opts.template_url))?;

    if let Some(reference) = opts.reference {
        checkout_ref(&dest, reference)
            .with_context(|| format!("git checkout failed for ref {}", reference))?;
    }

    // Keep tempdir alive by leaking it; caller is responsible for cleanup by
    // managing process lifetime or explicit cleanup strategy later.
    // For now, we convert to PathBuf and forget the tempdir handle.
    std::mem::forget(dir);

    Ok(ClonedRepo { path: dest })
}

pub fn checkout_ref(repo_dir: &Path, reference: &str) -> Result<()> {
    run_git(Some(repo_dir), &["checkout", "--quiet", reference]).context("git checkout failed")?;

    Ok(())
}

pub fn ensure_git_available() -> Result<()> {
    let output = Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow!("git is not available on PATH: {}", err)),
    }
}

fn run_git(cwd: Option<&Path>, args: &[&str]) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd.output().context("failed to spawn git")?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("git failed: {}", stderr.trim()))
    }
}
