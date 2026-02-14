/// iroh 0.96 implementation of SyncTransport and SyncConnection.
///
/// All iroh-specific code is isolated here. If the iroh API changes,
/// only this file needs updating.
use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use iroh::{
    endpoint::RecvStream, endpoint::SendStream, Endpoint, EndpointAddr, PublicKey, SecretKey,
};

use crate::transport::{SyncConnection, SyncMessage, SyncTransport};

/// ALPN identifier for our sync protocol.
pub const SYNC_ALPN: &[u8] = b"/agentic-note/sync/1";

/// iroh-based transport. Wraps an `Endpoint` which manages QUIC connections.
pub struct IrohTransport {
    endpoint: Endpoint,
}

impl IrohTransport {
    /// Create a new transport using the provided iroh Endpoint.
    pub fn new(endpoint: Endpoint) -> Self {
        Self { endpoint }
    }

    /// Build and bind a new endpoint with our ALPN registered and a given secret key.
    pub async fn bind(secret_key: SecretKey) -> Result<Self> {
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .alpns(vec![SYNC_ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| AgenticError::Sync(format!("iroh bind: {e}")))?;
        Ok(Self { endpoint })
    }

    /// Return the underlying iroh Endpoint.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Return the current EndpointAddr (includes relay URL + direct addresses).
    pub fn endpoint_addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }
}

#[async_trait]
impl SyncTransport for IrohTransport {
    async fn connect(&self, peer_id: &str) -> Result<Box<dyn SyncConnection>> {
        // Parse peer_id as iroh PublicKey (base32-encoded)
        let public_key: PublicKey = peer_id
            .parse()
            .map_err(|e| AgenticError::Sync(format!("parse peer_id '{peer_id}': {e}")))?;

        let addr = EndpointAddr::new(public_key);
        let conn = self
            .endpoint
            .connect(addr, SYNC_ALPN)
            .await
            .map_err(|e| AgenticError::Sync(format!("connect to {peer_id}: {e}")))?;

        let (send, recv) = conn
            .open_bi()
            .await
            .map_err(|e| AgenticError::Sync(format!("open_bi: {e}")))?;

        Ok(Box::new(IrohSyncConnection { send, recv }))
    }

    async fn listen(&self) -> Result<Box<dyn SyncConnection>> {
        // `accept()` returns a future (Accept<'_>) that resolves to Option<Incoming>
        let incoming =
            self.endpoint.accept().await.ok_or_else(|| {
                AgenticError::Sync("endpoint closed — no incoming connection".into())
            })?;

        // Await the Incoming to complete the handshake and get a Connection
        let conn = incoming
            .await
            .map_err(|e| AgenticError::Sync(format!("accept handshake: {e}")))?;

        let (send, recv) = conn
            .accept_bi()
            .await
            .map_err(|e| AgenticError::Sync(format!("accept_bi: {e}")))?;

        Ok(Box::new(IrohSyncConnection { send, recv }))
    }

    fn local_peer_id(&self) -> String {
        // id() returns EndpointId (= PublicKey) whose Display is base32
        self.endpoint.id().to_string()
    }
}

/// A single QUIC bi-directional stream wrapped as SyncConnection.
pub struct IrohSyncConnection {
    send: SendStream,
    recv: RecvStream,
}

#[async_trait]
impl SyncConnection for IrohSyncConnection {
    async fn send(&mut self, msg: &SyncMessage) -> Result<()> {
        let bytes = serde_json::to_vec(msg)
            .map_err(|e| AgenticError::Sync(format!("serialize msg: {e}")))?;
        // Length-prefix framing: 4-byte big-endian u32 length header
        let len = bytes.len() as u32;
        self.send
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| AgenticError::Sync(format!("write len: {e}")))?;
        self.send
            .write_all(&bytes)
            .await
            .map_err(|e| AgenticError::Sync(format!("write msg: {e}")))?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<SyncMessage> {
        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        self.recv
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| AgenticError::Sync(format!("read len: {e}")))?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Sanity check: 64 MiB max message size
        const MAX_MSG: usize = 64 * 1024 * 1024;
        if len > MAX_MSG {
            return Err(AgenticError::Sync(format!(
                "message too large: {len} bytes"
            )));
        }

        let mut buf = vec![0u8; len];
        self.recv
            .read_exact(&mut buf)
            .await
            .map_err(|e| AgenticError::Sync(format!("read msg body: {e}")))?;

        serde_json::from_slice(&buf)
            .map_err(|e| AgenticError::Sync(format!("deserialize msg: {e}")))
    }

    async fn send_blob(&mut self, id: &str, data: &[u8]) -> Result<()> {
        // Wrap blob in SyncMessage::BlobBatch and use the same framing
        let msg = SyncMessage::BlobBatch {
            blobs: vec![(id.to_string(), data.to_vec())],
        };
        self.send(&msg).await
    }

    async fn recv_blob(&mut self) -> Result<(String, Vec<u8>)> {
        let msg = self.recv().await?;
        match msg {
            SyncMessage::BlobBatch { mut blobs } if blobs.len() == 1 => Ok(blobs.remove(0)),
            other => Err(AgenticError::Sync(format!(
                "expected BlobBatch(1), got {other:?}"
            ))),
        }
    }

    async fn close(&mut self) -> Result<()> {
        self.send
            .finish()
            .map_err(|e| AgenticError::Sync(format!("finish stream: {e}")))?;
        Ok(())
    }
}
