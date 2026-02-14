use agentic_note_core::error::{AgenticError, Result};
use iroh::SecretKey;
use std::path::Path;

/// Device identity for P2P sync, backed by an iroh SecretKey.
pub struct DeviceIdentity {
    /// The iroh secret key used for transport-layer authentication.
    pub secret_key: SecretKey,
    /// Human-readable peer ID (base32-encoded public key).
    pub peer_id: String,
}

impl DeviceIdentity {
    /// Generate a new random identity using OS randomness.
    pub fn generate() -> Self {
        // Generate 32 random bytes using rand 0.8, then create SecretKey from bytes.
        // This avoids CryptoRng trait version mismatch between rand 0.8 and iroh's rand_core 0.9.
        use rand::RngCore;
        let mut key_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key_bytes);
        let secret_key = SecretKey::from(key_bytes);
        let peer_id = secret_key.public().to_string();
        Self {
            secret_key,
            peer_id,
        }
    }

    /// Load identity from file, or generate and save if not present.
    pub fn init_or_load(agentic_dir: &Path) -> Result<Self> {
        let key_path = agentic_dir.join("identity.key");
        if key_path.exists() {
            Self::load(&key_path)
        } else {
            let identity = Self::generate();
            identity.save(&key_path)?;
            Ok(identity)
        }
    }

    /// Load from a key file (32-byte secret key).
    pub fn load(path: &Path) -> Result<Self> {
        let bytes =
            std::fs::read(path).map_err(|e| AgenticError::Sync(format!("read identity: {e}")))?;
        if bytes.len() != 32 {
            return Err(AgenticError::Sync("invalid identity key length".into()));
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        let secret_key = SecretKey::from(key_bytes);
        let peer_id = secret_key.public().to_string();
        Ok(Self {
            secret_key,
            peer_id,
        })
    }

    /// Save secret key to file with restrictive permissions.
    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.secret_key.to_bytes())
            .map_err(|e| AgenticError::Sync(format!("save identity: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)
                .map_err(|e| AgenticError::Sync(format!("set permissions: {e}")))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generate_produces_valid_peer_id() {
        let identity = DeviceIdentity::generate();
        assert!(!identity.peer_id.is_empty());
        assert_eq!(identity.peer_id, identity.secret_key.public().to_string());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let key_path = dir.path().join("identity.key");

        let original = DeviceIdentity::generate();
        let original_peer_id = original.peer_id.clone();
        original.save(&key_path).unwrap();

        let loaded = DeviceIdentity::load(&key_path).unwrap();
        assert_eq!(loaded.peer_id, original_peer_id);
        assert_eq!(loaded.secret_key.to_bytes(), original.secret_key.to_bytes());
    }

    #[test]
    fn init_or_load_creates_on_first_call() {
        let dir = TempDir::new().unwrap();
        let agentic_dir = dir.path();
        let key_path = agentic_dir.join("identity.key");

        assert!(!key_path.exists());
        let identity = DeviceIdentity::init_or_load(agentic_dir).unwrap();
        assert!(key_path.exists());
        assert!(!identity.peer_id.is_empty());
    }

    #[test]
    fn init_or_load_loads_existing() {
        let dir = TempDir::new().unwrap();
        let agentic_dir = dir.path();

        let first = DeviceIdentity::init_or_load(agentic_dir).unwrap();
        let second = DeviceIdentity::init_or_load(agentic_dir).unwrap();
        assert_eq!(first.peer_id, second.peer_id);
    }

    #[test]
    fn key_file_has_32_bytes() {
        let dir = TempDir::new().unwrap();
        let key_path = dir.path().join("identity.key");
        let identity = DeviceIdentity::generate();
        identity.save(&key_path).unwrap();
        let bytes = std::fs::read(&key_path).unwrap();
        assert_eq!(bytes.len(), 32);
    }
}
