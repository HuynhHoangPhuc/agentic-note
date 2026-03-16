use zenon_core::error::Result;
use zenon_vault::init::init_vault;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Temporary vault helper that cleans up on drop.
pub struct TempVault {
    _root: TempDir,
    vault_path: PathBuf,
}

impl TempVault {
    /// Create a new temp vault initialized with default folders.
    pub fn new() -> Result<Self> {
        let root =
            TempDir::new().map_err(|e| std::io::Error::other(format!("create temp dir: {e}")))?;
        let vault_path = root.path().join("vault");
        init_vault(&vault_path)?;
        Ok(Self {
            _root: root,
            vault_path,
        })
    }

    /// Create a new temp vault and write a file relative to the vault root.
    pub fn with_note(path: &str, contents: &str) -> Result<Self> {
        let vault = Self::new()?;
        vault.write_note(path, contents)?;
        Ok(vault)
    }

    /// Get the vault root path.
    pub fn path(&self) -> &Path {
        &self.vault_path
    }

    /// Write a note relative to the vault root, creating parent directories.
    pub fn write_note(&self, rel_path: &str, contents: &str) -> Result<()> {
        let path = self.vault_path.join(rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Add a note in the Inbox PARA folder.
    pub fn write_inbox_note(&self, filename: &str, contents: &str) -> Result<()> {
        let rel_path = format!("inbox/{filename}");
        self.write_note(&rel_path, contents)
    }
}
