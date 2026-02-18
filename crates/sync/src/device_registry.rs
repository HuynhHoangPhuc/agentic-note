use agentic_note_core::error::{AgenticError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A known peer device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownDevice {
    pub peer_id: String,
    pub name: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Persistent registry of known peer devices stored as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistry {
    pub devices: Vec<KnownDevice>,
    #[serde(skip)]
    path: PathBuf,
}

impl DeviceRegistry {
    /// Load from JSON file, or create empty if missing.
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| AgenticError::Sync(format!("read devices: {e}")))?;
            let mut registry: Self = serde_json::from_str(&content)
                .map_err(|e| AgenticError::Sync(format!("parse devices: {e}")))?;
            registry.path = path.to_path_buf();
            Ok(registry)
        } else {
            Ok(Self {
                devices: Vec::new(),
                path: path.to_path_buf(),
            })
        }
    }

    /// Persist to disk.
    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self)
            .map_err(|e| AgenticError::Sync(format!("serialize devices: {e}")))?;
        std::fs::write(&self.path, content)
            .map_err(|e| AgenticError::Sync(format!("write devices: {e}")))?;
        Ok(())
    }

    /// Add a new device. No-op if peer_id already exists.
    pub fn add_device(&mut self, peer_id: String, name: Option<String>) {
        if !self.devices.iter().any(|d| d.peer_id == peer_id) {
            self.devices.push(KnownDevice {
                peer_id,
                name,
                last_sync: None,
            });
        }
    }

    /// Remove a device by peer_id.
    pub fn remove_device(&mut self, peer_id: &str) {
        self.devices.retain(|d| d.peer_id != peer_id);
    }

    /// Update last_sync timestamp for a peer.
    pub fn update_last_sync(&mut self, peer_id: &str) {
        if let Some(dev) = self.devices.iter_mut().find(|d| d.peer_id == peer_id) {
            dev.last_sync = Some(Utc::now());
        }
    }

    pub fn list(&self) -> &[KnownDevice] {
        &self.devices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_registry_when_file_missing() -> Result<()> {
        let dir = TempDir::new().map_err(AgenticError::Io)?;
        let path = dir.path().join("devices.json");
        let reg = DeviceRegistry::load(&path)?;
        assert!(reg.list().is_empty());
        Ok(())
    }

    #[test]
    fn add_and_list_devices() -> Result<()> {
        let dir = TempDir::new().map_err(AgenticError::Io)?;
        let path = dir.path().join("devices.json");
        let mut reg = DeviceRegistry::load(&path)?;

        reg.add_device("peer-aaa".to_string(), Some("Laptop".to_string()));
        reg.add_device("peer-bbb".to_string(), None);
        assert_eq!(reg.list().len(), 2);
        assert_eq!(reg.list()[0].peer_id, "peer-aaa");
        assert_eq!(reg.list()[0].name.as_deref(), Some("Laptop"));
        assert_eq!(reg.list()[1].peer_id, "peer-bbb");
        Ok(())
    }

    #[test]
    fn add_device_no_duplicate() -> Result<()> {
        let dir = TempDir::new().map_err(AgenticError::Io)?;
        let path = dir.path().join("devices.json");
        let mut reg = DeviceRegistry::load(&path)?;

        reg.add_device("peer-aaa".to_string(), None);
        reg.add_device("peer-aaa".to_string(), Some("Renamed".to_string()));
        assert_eq!(reg.list().len(), 1, "duplicate peer_id must not be added");
        Ok(())
    }

    #[test]
    fn remove_device() -> Result<()> {
        let dir = TempDir::new().map_err(AgenticError::Io)?;
        let path = dir.path().join("devices.json");
        let mut reg = DeviceRegistry::load(&path)?;

        reg.add_device("peer-aaa".to_string(), None);
        reg.add_device("peer-bbb".to_string(), None);
        reg.remove_device("peer-aaa");
        assert_eq!(reg.list().len(), 1);
        assert_eq!(reg.list()[0].peer_id, "peer-bbb");
        Ok(())
    }

    #[test]
    fn save_and_load_round_trip() -> Result<()> {
        let dir = TempDir::new().map_err(AgenticError::Io)?;
        let path = dir.path().join("devices.json");

        let mut reg = DeviceRegistry::load(&path)?;
        reg.add_device("peer-ccc".to_string(), Some("Desktop".to_string()));
        reg.save()?;

        let loaded = DeviceRegistry::load(&path)?;
        assert_eq!(loaded.list().len(), 1);
        assert_eq!(loaded.list()[0].peer_id, "peer-ccc");
        assert_eq!(loaded.list()[0].name.as_deref(), Some("Desktop"));
        Ok(())
    }

    #[test]
    fn update_last_sync_sets_timestamp() -> Result<()> {
        let dir = TempDir::new().map_err(AgenticError::Io)?;
        let path = dir.path().join("devices.json");
        let mut reg = DeviceRegistry::load(&path)?;

        reg.add_device("peer-ddd".to_string(), None);
        assert!(reg.list()[0].last_sync.is_none());

        reg.update_last_sync("peer-ddd");
        assert!(reg.list()[0].last_sync.is_some());
        Ok(())
    }
}
