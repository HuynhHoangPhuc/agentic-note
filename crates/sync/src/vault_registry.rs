/// Vault registry: manifest of registered vaults for multi-vault sync.
///
/// Manifest is persisted at `~/.agentic-note/vaults.toml`.
use agentic_note_core::config::VaultEntry;
use agentic_note_core::error::{AgenticError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level manifest holding all registered vaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultManifest {
    #[serde(default)]
    pub vaults: Vec<VaultEntry>,
}

/// Registry that loads/saves the manifest and exposes mutation helpers.
pub struct VaultRegistry {
    pub manifest: VaultManifest,
    manifest_path: PathBuf,
}

impl VaultRegistry {
    /// Load (or create) the vault registry from `~/.agentic-note/vaults.toml`.
    pub fn load() -> Result<Self> {
        let manifest_path = Self::default_manifest_path()?;

        // Create parent directory if it doesn't exist yet.
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AgenticError::MultiVault(format!("create ~/.agentic-note dir: {e}"))
            })?;
        }

        let manifest = if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
                AgenticError::MultiVault(format!("read {}: {e}", manifest_path.display()))
            })?;
            toml::from_str(&content)
                .map_err(|e| AgenticError::MultiVault(format!("parse vaults.toml: {e}")))?
        } else {
            VaultManifest::default()
        };

        Ok(Self {
            manifest,
            manifest_path,
        })
    }

    /// Persist the current manifest to disk.
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.manifest)
            .map_err(|e| AgenticError::MultiVault(format!("serialize manifest: {e}")))?;
        std::fs::write(&self.manifest_path, content).map_err(|e| {
            AgenticError::MultiVault(format!("write {}: {e}", self.manifest_path.display()))
        })
    }

    /// Register a vault at `path` with the given `name`.
    ///
    /// If a vault with the same canonical path already exists, it is updated.
    pub fn register(&mut self, path: PathBuf, name: String) -> Result<()> {
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

        if let Some(entry) = self.manifest.vaults.iter_mut().find(|v| {
            std::fs::canonicalize(&v.path).unwrap_or_else(|_| v.path.clone()) == canonical
        }) {
            entry.name = name;
        } else {
            self.manifest.vaults.push(VaultEntry {
                path,
                name,
                sync_enabled: true,
                default_peers: vec![],
            });
        }
        Ok(())
    }

    /// Remove a vault identified by `path` from the registry.
    pub fn unregister(&mut self, path: &Path) -> Result<()> {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        let before = self.manifest.vaults.len();
        self.manifest.vaults.retain(|v| {
            std::fs::canonicalize(&v.path).unwrap_or_else(|_| v.path.clone()) != canonical
        });

        if self.manifest.vaults.len() == before {
            return Err(AgenticError::MultiVault(format!(
                "vault not registered: {}",
                path.display()
            )));
        }
        Ok(())
    }

    /// Return all registered vaults.
    pub fn list(&self) -> &[VaultEntry] {
        &self.manifest.vaults
    }

    /// Return only vaults that have sync enabled.
    pub fn sync_enabled(&self) -> Vec<&VaultEntry> {
        self.manifest
            .vaults
            .iter()
            .filter(|v| v.sync_enabled)
            .collect()
    }

    fn default_manifest_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| AgenticError::MultiVault("cannot resolve home directory".into()))?;
        Ok(home.join(".agentic-note").join("vaults.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn registry_in(dir: &TempDir) -> VaultRegistry {
        let manifest_path = dir.path().join("vaults.toml");
        VaultRegistry {
            manifest: VaultManifest::default(),
            manifest_path,
        }
    }

    #[test]
    fn register_and_list() {
        let tmp = TempDir::new().expect("temp dir");
        let mut reg = registry_in(&tmp);
        reg.register(PathBuf::from("/tmp/vault1"), "v1".into())
            .expect("register vault1");
        reg.register(PathBuf::from("/tmp/vault2"), "v2".into())
            .expect("register vault2");
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn save_and_reload() {
        let tmp = TempDir::new().expect("temp dir");
        let manifest_path = tmp.path().join("vaults.toml");

        let mut reg = VaultRegistry {
            manifest: VaultManifest::default(),
            manifest_path: manifest_path.clone(),
        };
        reg.register(PathBuf::from("/tmp/vault1"), "v1".into())
            .expect("register vault1");
        reg.save().expect("save registry");

        let content = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let loaded: VaultManifest = toml::from_str(&content).expect("parse manifest");
        assert_eq!(loaded.vaults.len(), 1);
        assert_eq!(loaded.vaults[0].name, "v1");
    }

    #[test]
    fn sync_enabled_filter() {
        let tmp = TempDir::new().expect("temp dir");
        let mut reg = registry_in(&tmp);
        reg.register(PathBuf::from("/tmp/vault1"), "v1".into())
            .expect("register vault1");
        reg.manifest.vaults[0].sync_enabled = false;
        reg.register(PathBuf::from("/tmp/vault2"), "v2".into())
            .expect("register vault2");
        assert_eq!(reg.sync_enabled().len(), 1);
        assert_eq!(reg.sync_enabled()[0].name, "v2");
    }
}
