use crate::hash::{hash_bytes, ObjectId};
use zenon_core::{AgenticError, Result};
use std::path::PathBuf;
use tracing::debug;

/// Content-addressed object store.
/// Objects are stored at `objects/{id[0..2]}/{id[2..]}`.
pub struct BlobStore {
    pub objects_dir: PathBuf,
}

impl BlobStore {
    pub fn new(objects_dir: PathBuf) -> Self {
        Self { objects_dir }
    }

    /// Compute object path from its ID (two-char prefix split).
    pub fn object_path(&self, id: &ObjectId) -> PathBuf {
        self.objects_dir.join(&id[..2]).join(&id[2..])
    }

    /// Store bytes, returning the ObjectId. Skips write if already present.
    pub fn store(&self, data: &[u8]) -> Result<ObjectId> {
        let id = hash_bytes(data);
        let path = self.object_path(&id);
        if path.exists() {
            debug!("blob already exists: {}", id);
            return Ok(id);
        }
        let parent = path.parent().ok_or_else(|| {
            AgenticError::NotFound("object path missing parent directory".to_string())
        })?;
        std::fs::create_dir_all(parent)?;
        std::fs::write(&path, data)?;
        debug!("stored blob: {}", id);
        Ok(id)
    }

    /// Load object bytes by ID.
    pub fn load(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self.object_path(id);
        if !path.exists() {
            return Err(AgenticError::NotFound(format!("object not found: {}", id)));
        }
        Ok(std::fs::read(&path)?)
    }

    /// Return true if object with given ID exists in the store.
    pub fn exists(&self, id: &ObjectId) -> bool {
        self.object_path(id).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_store() -> BlobStore {
        let dir = env::temp_dir().join(format!("cas-test-{}", std::process::id()));
        BlobStore::new(dir.join("objects"))
    }

    #[test]
    fn store_and_load_roundtrip() -> Result<()> {
        let store = temp_store();
        let data = b"hello cas";
        let id = store.store(data)?;
        assert_eq!(id.len(), 64);
        let loaded = store.load(&id)?;
        assert_eq!(loaded, data);
        Ok(())
    }

    #[test]
    fn store_idempotent() -> Result<()> {
        let store = temp_store();
        let data = b"idempotent";
        let id1 = store.store(data)?;
        let id2 = store.store(data)?;
        assert_eq!(id1, id2);
        Ok(())
    }

    #[test]
    fn missing_object_returns_not_found() {
        let store = temp_store();
        let fake_id = "a".repeat(64);
        let result = store.load(&fake_id);
        assert!(matches!(result, Err(AgenticError::NotFound(_))));
    }
}
