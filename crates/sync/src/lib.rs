/// Sync crate: P2P sync via iroh, device identity, and merge orchestration.
pub mod batch_sync;
pub mod compression;
pub mod device_registry;
pub mod identity;
pub mod iroh_transport;
pub mod merge_driver;
pub mod protocol;
pub mod transport;

pub use batch_sync::{BatchSyncResult, PeerSyncOutcome, PeerSyncStatus};
pub use device_registry::{DeviceRegistry, KnownDevice};
pub use identity::DeviceIdentity;
pub use iroh_transport::IrohTransport;
pub use merge_driver::MergeOutcome;
pub use protocol::SyncResult;
pub use transport::{SyncConnection, SyncMessage, SyncTransport};

use std::path::{Path, PathBuf};

use agentic_note_cas::Cas;
use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::ConflictPolicy;

/// Top-level facade combining identity, device registry, transport, and CAS.
///
/// Callers instantiate `SyncEngine` once per vault session, then call
/// `sync_with_peer()` to sync with a known peer.
pub struct SyncEngine {
    pub identity: DeviceIdentity,
    pub registry: DeviceRegistry,
    pub transport: Box<dyn SyncTransport>,
    pub cas: Cas,
    vault_path: PathBuf,
}

impl SyncEngine {
    /// Create a SyncEngine backed by the iroh transport.
    ///
    /// Loads (or generates) the device identity from `vault_path/.agentic/identity.key`,
    /// loads the device registry from `vault_path/.agentic/devices.json`,
    /// opens the CAS at `vault_path/.agentic/cas/`, and binds an iroh endpoint.
    pub async fn new_with_iroh(vault_path: &Path) -> Result<Self> {
        let agentic_dir = vault_path.join(".agentic");
        std::fs::create_dir_all(&agentic_dir)
            .map_err(|e| AgenticError::Sync(format!("create .agentic dir: {e}")))?;

        let identity = DeviceIdentity::init_or_load(&agentic_dir)?;
        let registry_path = agentic_dir.join("devices.json");
        let registry = DeviceRegistry::load(&registry_path)?;
        let cas = Cas::open(vault_path)?;

        let transport = IrohTransport::bind(identity.secret_key.clone())
            .await
            .map_err(|e| AgenticError::Sync(format!("bind iroh transport: {e}")))?;

        Ok(Self {
            identity,
            registry,
            transport: Box::new(transport),
            cas,
            vault_path: vault_path.to_path_buf(),
        })
    }

    /// Create a SyncEngine with a custom (e.g. mock) transport. Useful for tests.
    pub fn new_with_transport(
        identity: DeviceIdentity,
        registry: DeviceRegistry,
        transport: Box<dyn SyncTransport>,
        cas: Cas,
        vault_path: &Path,
    ) -> Self {
        Self {
            identity,
            registry,
            transport,
            cas,
            vault_path: vault_path.to_path_buf(),
        }
    }

    /// Sync with a known peer identified by `peer_id`.
    ///
    /// Connects to the peer, runs the full sync protocol, then updates
    /// the peer's `last_sync` timestamp in the registry.
    pub async fn sync_with_peer(
        &mut self,
        peer_id: &str,
        policy: &ConflictPolicy,
    ) -> Result<SyncResult> {
        let mut conn = self.transport.connect(peer_id).await?;
        let result =
            protocol::run_sync_initiator(conn.as_mut(), &self.cas, &self.vault_path, policy)
                .await?;

        // Update last_sync in registry
        self.registry.update_last_sync(peer_id);
        self.registry
            .save()
            .map_err(|e| AgenticError::Sync(format!("save registry after sync: {e}")))?;

        Ok(result)
    }

    /// Return the local device identity.
    pub fn device_info(&self) -> &DeviceIdentity {
        &self.identity
    }

    /// Return all known peer devices.
    pub fn known_devices(&self) -> &[KnownDevice] {
        self.registry.list()
    }

    /// Add a peer to the device registry and persist.
    pub fn pair_device(&mut self, peer_id: String, name: Option<String>) -> Result<()> {
        self.registry.add_device(peer_id, name);
        self.registry
            .save()
            .map_err(|e| AgenticError::Sync(format!("save registry after pairing: {e}")))
    }
}
