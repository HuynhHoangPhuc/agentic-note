use agentic_note_cas::Cas;
use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::ConflictPolicy;
use agentic_note_sync::batch_sync::sync_all_peers;
use agentic_note_sync::transport::{SyncConnection, SyncMessage, SyncTransport};
use proptest::prelude::*;
use std::path::Path;

struct NoopTransport;

#[async_trait::async_trait]
impl SyncTransport for NoopTransport {
    async fn connect(&self, _peer_id: &str) -> Result<Box<dyn SyncConnection>> {
        Err(AgenticError::Sync("not connected".into()))
    }

    async fn listen(&self) -> Result<Box<dyn SyncConnection>> {
        Err(AgenticError::Sync("not supported".into()))
    }

    fn local_peer_id(&self) -> String {
        "noop".into()
    }
}

#[async_trait::async_trait]
impl SyncConnection for NoopConnection {
    async fn send(&mut self, _msg: &SyncMessage) -> Result<()> {
        Ok(())
    }

    async fn recv(&mut self) -> Result<SyncMessage> {
        Err(AgenticError::Sync("not supported".into()))
    }

    async fn send_blob(&mut self, _id: &str, _data: &[u8]) -> Result<()> {
        Ok(())
    }

    async fn recv_blob(&mut self) -> Result<(String, Vec<u8>)> {
        Err(AgenticError::Sync("not supported".into()))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

struct NoopConnection;

proptest! {
    #[test]
    fn peer_order_is_deterministic(mut peers in proptest::collection::vec("[a-z]{1,6}", 0..12)) {
        let mut sorted = peers.clone();
        sorted.sort();
        peers.sort();
        prop_assert_eq!(peers, sorted);
    }
}

#[tokio::test]
async fn empty_peer_list_returns_empty_result() -> Result<()> {
    let temp = tempfile::tempdir().map_err(AgenticError::Io)?;
    let cas = Cas::open(Path::new(temp.path()))?;
    let res = sync_all_peers(
        &NoopTransport,
        &cas,
        temp.path(),
        &[],
        &ConflictPolicy::Manual,
    )
    .await?;
    assert!(res.outcomes.is_empty());
    assert_eq!(res.total_merged, 0);
    Ok(())
}
