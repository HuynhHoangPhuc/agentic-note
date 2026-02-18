use crate::blob::BlobStore;
use agentic_note_core::Result;
use std::path::{Path, PathBuf};

/// Top-level CAS facade.
/// All state lives under `{vault}/.agentic/cas/`.
pub struct Cas {
    pub blob_store: BlobStore,
    pub snapshots_dir: PathBuf,
    pub device_id: String,
}

impl Cas {
    /// Open (or initialise) a CAS instance rooted at `vault_path`.
    /// Creates `.agentic/cas/objects/` and `.agentic/cas/snapshots/` if absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the CAS directories cannot be created or the device
    /// ID cannot be persisted.
    pub fn open(vault_path: &Path) -> Result<Self> {
        let cas_dir = vault_path.join(".agentic").join("cas");
        let objects_dir = cas_dir.join("objects");
        let snapshots_dir = cas_dir.join("snapshots");

        std::fs::create_dir_all(&objects_dir)?;
        std::fs::create_dir_all(&snapshots_dir)?;

        let device_id = Self::load_or_create_device_id(&cas_dir)?;

        Ok(Cas {
            blob_store: BlobStore::new(objects_dir),
            snapshots_dir,
            device_id,
        })
    }

    /// Read device ID from `{cas_dir}/device_id` or generate and persist a new one.
    fn load_or_create_device_id(cas_dir: &Path) -> Result<String> {
        let id_path = cas_dir.join("device_id");
        if id_path.exists() {
            let raw = std::fs::read_to_string(&id_path)?;
            return Ok(raw.trim().to_string());
        }
        // Generate a simple device ID from hostname + random bytes via hash
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
        let seed = format!(
            "{}{}",
            hostname,
            std::time::SystemTime::UNIX_EPOCH
                .elapsed()
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let id = crate::hash::hash_bytes(seed.as_bytes())[..16].to_string();
        std::fs::write(&id_path, &id)?;
        Ok(id)
    }
}
