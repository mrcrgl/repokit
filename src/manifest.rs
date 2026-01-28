use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub template_url: String,
    pub reference: Option<String>,
    pub profiles: Vec<String>,
    pub files: BTreeMap<PathBuf, FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub sha256: String,
}

impl Manifest {
    pub fn new(template_url: String, reference: Option<String>, profiles: Vec<String>) -> Self {
        Self {
            template_url,
            reference,
            profiles,
            files: BTreeMap::new(),
        }
    }

    pub fn matches_metadata(
        &self,
        template_url: &str,
        reference: Option<&str>,
        profiles: &[String],
    ) -> bool {
        self.template_url == template_url
            && self.reference.as_deref() == reference
            && self.profiles == profiles
    }

    pub fn insert_file_hash(&mut self, relative_path: PathBuf, sha256: String) {
        self.files.insert(relative_path, FileEntry { sha256 });
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest {}", path.display()))?;
        let manifest = toml::from_str(&content)
            .with_context(|| format!("failed to parse manifest {}", path.display()))?;
        Ok(manifest)
    }

    pub fn write_to_path(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let content = toml::to_string_pretty(self).context("failed to serialize manifest")?;
        fs::write(path, content)
            .with_context(|| format!("failed to write manifest {}", path.display()))?;
        Ok(())
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn hash_file(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let data = fs::read(path)
        .with_context(|| format!("failed to read file for hashing {}", path.display()))?;
    Ok(hash_bytes(&data))
}
