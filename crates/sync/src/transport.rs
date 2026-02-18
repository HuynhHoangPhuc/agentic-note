use agentic_note_core::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Messages exchanged during sync protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    SyncRequest { snapshot_id: String },
    SyncResponse { snapshot_id: String },
    BlobRequest { ids: Vec<String> },
    BlobBatch { blobs: Vec<(String, Vec<u8>)> },
    SyncComplete { snapshot_id: String },
    Error { message: String },
    /// An encrypted note blob sent from one peer to another.
    /// `sender_pubkey` is the sender's X25519 public key (32 bytes).
    EncryptedNote {
        sender_pubkey: [u8; 32],
        payload: crate::encryption::EncryptedEnvelope,
    },
    /// Advertise X25519 encryption capability during handshake.
    EncryptionSupported {
        x25519_pubkey: [u8; 32],
    },
    /// Initiate X3DH handshake for Double Ratchet sessions.
    X3DHInit {
        sender_pubkey: [u8; 32],
        prekey: [u8; 32],
    },
    /// Respond to X3DH handshake with derived root key.
    X3DHResponse {
        sender_pubkey: [u8; 32],
        root_key: [u8; 32],
    },
}

/// Transport abstraction — thin adapter to isolate iroh API.
#[async_trait]
pub trait SyncTransport: Send + Sync {
    async fn connect(&self, peer_id: &str) -> Result<Box<dyn SyncConnection>>;
    async fn listen(&self) -> Result<Box<dyn SyncConnection>>;
    fn local_peer_id(&self) -> String;
}

/// A single sync connection for bidirectional message exchange.
#[async_trait]
pub trait SyncConnection: Send + Sync {
    async fn send(&mut self, msg: &SyncMessage) -> Result<()>;
    async fn recv(&mut self) -> Result<SyncMessage>;
    async fn send_blob(&mut self, id: &str, data: &[u8]) -> Result<()>;
    async fn recv_blob(&mut self) -> Result<(String, Vec<u8>)>;
    async fn close(&mut self) -> Result<()>;
}
