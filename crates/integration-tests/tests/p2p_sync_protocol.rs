use agentic_note_cas::{Cas, Snapshot};
use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::ConflictPolicy;
use agentic_note_sync::protocol::{run_sync_initiator, run_sync_responder};
use agentic_note_sync::transport::{SyncConnection, SyncMessage};
use agentic_note_test_utils::TempVault;
use async_trait::async_trait;
use std::path::Path;
use tokio::sync::mpsc;

struct MemoryConnection {
    tx: mpsc::Sender<SyncMessage>,
    rx: mpsc::Receiver<SyncMessage>,
}

impl MemoryConnection {
    fn pair() -> (Self, Self) {
        let (a_tx, a_rx) = mpsc::channel(16);
        let (b_tx, b_rx) = mpsc::channel(16);
        (Self { tx: a_tx, rx: b_rx }, Self { tx: b_tx, rx: a_rx })
    }
}

#[async_trait]
impl SyncConnection for MemoryConnection {
    async fn send(&mut self, msg: &SyncMessage) -> Result<()> {
        self.tx
            .send(msg.clone())
            .await
            .map_err(|_| AgenticError::Sync("memory connection closed".into()))
    }

    async fn recv(&mut self) -> Result<SyncMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| AgenticError::Sync("memory connection closed".into()))
    }

    async fn send_blob(&mut self, id: &str, data: &[u8]) -> Result<()> {
        self.send(&SyncMessage::BlobBatch {
            blobs: vec![(id.to_string(), data.to_vec())],
        })
        .await
    }

    async fn recv_blob(&mut self) -> Result<(String, Vec<u8>)> {
        match self.recv().await? {
            SyncMessage::BlobBatch { mut blobs } => blobs
                .pop()
                .ok_or_else(|| AgenticError::Sync("missing blob payload".into())),
            other => Err(AgenticError::Sync(format!(
                "expected BlobBatch, got {other:?}"
            ))),
        }
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn sync_protocol_completes_for_identical_vaults() -> Result<()> {
    let vault_a = TempVault::with_note("inbox/shared.md", "same content")?;
    let vault_b = TempVault::with_note("inbox/shared.md", "same content")?;
    let cas_a = Cas::open(vault_a.path())?;
    let cas_b = Cas::open(vault_b.path())?;

    Snapshot::create(vault_a.path(), &cas_a, Some("seed".into()))?;
    Snapshot::create(vault_b.path(), &cas_b, Some("seed".into()))?;

    let (mut conn_a, mut conn_b) = MemoryConnection::pair();
    let policy = ConflictPolicy::NewestWins;

    let (initiator, responder) = tokio::join!(
        run_sync_initiator(&mut conn_a, &cas_a, vault_a.path(), &policy),
        run_sync_responder(&mut conn_b, &cas_b, vault_b.path(), &policy)
    );

    let initiator = initiator?;
    let responder = responder?;

    assert_eq!(initiator.conflicts, 0);
    assert_eq!(responder.conflicts, 0);
    assert!(!initiator.snapshot_id.is_empty());
    assert!(!responder.snapshot_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn sync_protocol_fast_forwards_one_sided_change() -> Result<()> {
    let vault_a = TempVault::new()?;
    let cas_a = Cas::open(vault_a.path())?;
    Snapshot::create(vault_a.path(), &cas_a, Some("seed".into()))?;

    let vault_b = TempVault::new()?;
    copy_dir_all(vault_a.path(), vault_b.path())?;

    vault_a.write_note("inbox/new-note.md", "from peer a")?;

    let cas_a = Cas::open(vault_a.path())?;
    let cas_b = Cas::open(vault_b.path())?;
    let (mut conn_a, mut conn_b) = MemoryConnection::pair();
    let policy = ConflictPolicy::NewestWins;

    let (initiator, responder) = tokio::join!(
        run_sync_initiator(&mut conn_a, &cas_a, vault_a.path(), &policy),
        run_sync_responder(&mut conn_b, &cas_b, vault_b.path(), &policy)
    );

    initiator?;
    responder?;

    let synced_note = std::fs::read_to_string(vault_b.path().join("inbox/new-note.md"))?;
    assert_eq!(synced_note, "from peer a");
    Ok(())
}

#[tokio::test]
async fn sync_protocol_merges_non_conflicting_divergent_changes() -> Result<()> {
    let vault_a = TempVault::new()?;
    let cas_a = Cas::open(vault_a.path())?;
    Snapshot::create(vault_a.path(), &cas_a, Some("seed".into()))?;

    let vault_b = TempVault::new()?;
    copy_dir_all(vault_a.path(), vault_b.path())?;

    vault_a.write_note("inbox/from-a.md", "alpha")?;
    vault_b.write_note("inbox/from-b.md", "beta")?;

    let cas_a = Cas::open(vault_a.path())?;
    let cas_b = Cas::open(vault_b.path())?;
    let (mut conn_a, mut conn_b) = MemoryConnection::pair();
    let policy = ConflictPolicy::NewestWins;

    let (initiator, responder) = tokio::join!(
        run_sync_initiator(&mut conn_a, &cas_a, vault_a.path(), &policy),
        run_sync_responder(&mut conn_b, &cas_b, vault_b.path(), &policy)
    );

    let initiator = initiator?;
    let responder = responder?;

    assert_eq!(initiator.conflicts, 0);
    assert_eq!(responder.conflicts, 0);
    assert_note(vault_a.path(), "inbox/from-a.md", "alpha")?;
    assert_note(vault_a.path(), "inbox/from-b.md", "beta")?;
    assert_note(vault_b.path(), "inbox/from-a.md", "alpha")?;
    assert_note(vault_b.path(), "inbox/from-b.md", "beta")?;
    Ok(())
}

#[tokio::test]
async fn sync_protocol_materializes_manual_conflicts_into_vault() -> Result<()> {
    let vault_a = TempVault::with_note("inbox/shared.md", "base")?;
    let cas_a = Cas::open(vault_a.path())?;
    Snapshot::create(vault_a.path(), &cas_a, Some("seed".into()))?;

    let vault_b = TempVault::new()?;
    copy_dir_all(vault_a.path(), vault_b.path())?;

    vault_a.write_note("inbox/shared.md", "local edit")?;
    vault_b.write_note("inbox/shared.md", "remote edit")?;

    let cas_a = Cas::open(vault_a.path())?;
    let cas_b = Cas::open(vault_b.path())?;
    let (mut conn_a, mut conn_b) = MemoryConnection::pair();
    let policy = ConflictPolicy::Manual;

    let (initiator, responder) = tokio::join!(
        run_sync_initiator(&mut conn_a, &cas_a, vault_a.path(), &policy),
        run_sync_responder(&mut conn_b, &cas_b, vault_b.path(), &policy)
    );

    let initiator = initiator?;
    let responder = responder?;

    assert_eq!(initiator.conflicts, 1);
    assert_eq!(responder.conflicts, 1);
    assert_note_contains(vault_a.path(), "inbox/shared.md", "local edit")?;
    assert_note_contains(vault_a.path(), "inbox/shared.md", "remote edit")?;
    assert_note_contains(vault_b.path(), "inbox/shared.md", "local edit")?;
    assert_note_contains(vault_b.path(), "inbox/shared.md", "remote edit")?;
    assert_note_contains(vault_a.path(), "inbox/shared.md", "<<<< LOCAL")?;
    assert_note_contains(vault_b.path(), "inbox/shared.md", ">>>> REMOTE")?;
    assert_conflict_file(vault_a.path(), "inbox_shared.md.conflict")?;
    assert_conflict_file(vault_b.path(), "inbox_shared.md.conflict")?;
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_dir_all(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn assert_note(vault_root: &Path, rel_path: &str, expected: &str) -> Result<()> {
    let note_path = vault_root.join(rel_path);
    if !note_path.exists() {
        return Err(AgenticError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("missing note: {}", note_path.display()),
        )));
    }
    let body = std::fs::read_to_string(&note_path)?;
    assert_eq!(body, expected);
    Ok(())
}

fn assert_note_contains(vault_root: &Path, rel_path: &str, expected_fragment: &str) -> Result<()> {
    let note_path = vault_root.join(rel_path);
    let body = std::fs::read_to_string(&note_path)?;
    assert!(
        body.contains(expected_fragment),
        "expected {} to contain {:?}, got {:?}",
        note_path.display(),
        expected_fragment,
        body
    );
    Ok(())
}

fn assert_conflict_file(vault_root: &Path, name: &str) -> Result<()> {
    let path = vault_root.join(".agentic").join("conflicts").join(name);
    if !path.exists() {
        return Err(AgenticError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("missing conflict file: {}", path.display()),
        )));
    }
    Ok(())
}
