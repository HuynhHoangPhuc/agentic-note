//! Background indexer that watches vault filesystem changes and auto-indexes notes.
//!
//! Uses `notify-debouncer-full` for debounced FS events, `tokio::sync::mpsc` for task queue,
//! configurable debounce window and batch size, and `CancellationToken` for graceful shutdown.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use zenon_core::config::IndexerConfig;
use zenon_core::Result;
use zenon_vault::Note;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::SearchEngine;

/// Tasks the background indexer can process.
pub enum IndexTask {
    FileChanged(PathBuf),
    FileDeleted(PathBuf),
    ReindexAll,
}

/// Background indexer that monitors vault directory for `.md` file changes.
pub struct BackgroundIndexer {
    vault_path: PathBuf,
    config: IndexerConfig,
    cancel: CancellationToken,
    task_tx: Option<mpsc::Sender<IndexTask>>,
}

impl BackgroundIndexer {
    pub fn new(vault_path: PathBuf, config: IndexerConfig) -> Self {
        Self {
            vault_path,
            config,
            cancel: CancellationToken::new(),
            task_tx: None,
        }
    }

    /// Returns a sender for manual index task submission.
    pub fn task_sender(&self) -> Option<mpsc::Sender<IndexTask>> {
        self.task_tx.clone()
    }

    /// Returns the cancellation token for graceful shutdown.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Spawn the background indexer as a tokio task.
    /// Returns a JoinHandle and a Sender for manual index tasks.
    pub fn spawn(
        mut self,
        search_engine: Arc<Mutex<SearchEngine>>,
    ) -> Result<(JoinHandle<()>, mpsc::Sender<IndexTask>)> {
        let (task_tx, task_rx) = mpsc::channel::<IndexTask>(256);
        self.task_tx = Some(task_tx.clone());
        let vault_path = self.vault_path.clone();
        let config = self.config.clone();
        let cancel = self.cancel.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) =
                run_indexer_loop(vault_path, config, search_engine, task_rx, cancel).await
            {
                error!("Background indexer failed: {e}");
            }
        });

        Ok((handle, task_tx))
    }
}

/// Main indexer event loop.
async fn run_indexer_loop(
    vault_path: PathBuf,
    config: IndexerConfig,
    search_engine: Arc<Mutex<SearchEngine>>,
    mut task_rx: mpsc::Receiver<IndexTask>,
    cancel: CancellationToken,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let debounce_duration = Duration::from_millis(config.debounce_ms);

    // Channel for FS events from notify
    let (fs_tx, mut fs_rx) = mpsc::channel::<Vec<PathBuf>>(64);

    // Create the FS watcher
    let fs_tx_clone = fs_tx.clone();
    let vault_path_clone = vault_path.clone();
    let mut debouncer = new_debouncer(
        debounce_duration,
        None,
        move |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    let paths: Vec<PathBuf> = events
                        .into_iter()
                        .flat_map(|e| e.event.paths)
                        .filter(|p| {
                            p.extension().is_some_and(|ext| ext == "md")
                                && p.starts_with(&vault_path_clone)
                        })
                        .collect();
                    if !paths.is_empty() {
                        // Use try_send to avoid blocking the notify thread
                        // (blocking_send could deadlock if the channel is full)
                        if fs_tx_clone.try_send(paths).is_err() {
                            warn!("Indexer FS event channel full, dropping batch");
                        }
                    }
                }
                Err(errors) => {
                    for e in errors {
                        warn!("FS watcher error: {e}");
                    }
                }
            }
        },
    )?;

    // Watch the vault directory recursively
    use notify_debouncer_full::notify::RecursiveMode;
    debouncer.watch(&vault_path, RecursiveMode::Recursive)?;
    info!(
        "Background indexer started, watching {}",
        vault_path.display()
    );

    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut pending_deletes: HashSet<PathBuf> = HashSet::new();
    let batch_size = config.batch_size;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Background indexer shutting down");
                // Flush remaining
                if !pending.is_empty() || !pending_deletes.is_empty() {
                    flush_batch(&search_engine, &mut pending, &mut pending_deletes).await;
                }
                break;
            }
            Some(paths) = fs_rx.recv() => {
                for path in paths {
                    if path.exists() {
                        pending.insert(path);
                    } else {
                        pending_deletes.insert(path);
                    }
                }
                if pending.len() + pending_deletes.len() >= batch_size {
                    flush_batch(&search_engine, &mut pending, &mut pending_deletes).await;
                }
            }
            Some(task) = task_rx.recv() => {
                match task {
                    IndexTask::FileChanged(path) => {
                        pending.insert(path);
                    }
                    IndexTask::FileDeleted(path) => {
                        pending_deletes.insert(path);
                    }
                    IndexTask::ReindexAll => {
                        info!("Manual reindex requested");
                        let engine = search_engine.lock().await;
                        if let Err(e) = engine.reindex(&vault_path) {
                            error!("Reindex failed: {e}");
                        }
                    }
                }
                if pending.len() + pending_deletes.len() >= batch_size {
                    flush_batch(&search_engine, &mut pending, &mut pending_deletes).await;
                }
            }
            // Periodic flush for pending items that haven't hit batch size
            _ = tokio::time::sleep(Duration::from_millis(500)), if !pending.is_empty() || !pending_deletes.is_empty() => {
                flush_batch(&search_engine, &mut pending, &mut pending_deletes).await;
            }
        }
    }

    drop(debouncer);
    info!("Background indexer stopped");
    Ok(())
}

/// Process pending file changes and deletions.
async fn flush_batch(
    search_engine: &Arc<Mutex<SearchEngine>>,
    pending: &mut HashSet<PathBuf>,
    pending_deletes: &mut HashSet<PathBuf>,
) {
    let changed: Vec<PathBuf> = pending.drain().collect();
    let deleted: Vec<PathBuf> = pending_deletes.drain().collect();

    // Lock mutably so index_note (&mut self) can be called
    let mut engine = search_engine.lock().await;

    for path in &changed {
        match Note::read(path) {
            Ok(note) => {
                if let Err(e) = engine.index_note(&note) {
                    warn!("Failed to index {}: {e}", path.display());
                }
            }
            Err(e) => {
                debug!("Skipping {}: {e}", path.display());
            }
        }
    }

    for path in &deleted {
        // Extract note ID from filename (format: "{ulid}-{slug}.md")
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(ulid_part) = stem.split('-').next() {
                if let Ok(note_id) = ulid_part.parse::<zenon_core::NoteId>() {
                    if let Err(e) = engine.remove_note(&note_id) {
                        warn!("Failed to remove index for {}: {e}", path.display());
                    }
                }
            }
        }
    }

    if !changed.is_empty() || !deleted.is_empty() {
        debug!(
            "Indexed {} changed, {} deleted files",
            changed.len(),
            deleted.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_task_variants() {
        let path = PathBuf::from("/tmp/test.md");
        let _ = IndexTask::FileChanged(path.clone());
        let _ = IndexTask::FileDeleted(path);
        let _ = IndexTask::ReindexAll;
    }
}
