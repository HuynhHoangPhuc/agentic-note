use agentic_note_core::error::Result;
use agentic_note_vault::{markdown, Note};
use std::path::Path;
use tracing::info;
use walkdir::WalkDir;

use crate::fts::FtsIndex;
use crate::graph::Graph;

/// Reindex entire vault: rebuild FTS and graph from all .md files.
pub fn reindex_vault(
    vault_path: &Path,
    fts: &FtsIndex,
    graph: &Graph<'_>,
) -> Result<usize> {
    let mut writer = fts.writer()?;
    let mut count = 0;

    for entry in WalkDir::new(vault_path)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        // Skip .agentic directory
        if path.to_str().map(|s| s.contains(".agentic")).unwrap_or(false) {
            continue;
        }

        let note = match Note::read(path) {
            Ok(n) => n,
            Err(e) => {
                tracing::warn!("skip {}: {e}", path.display());
                continue;
            }
        };

        fts.index_note(
            &writer,
            &note.id,
            &note.frontmatter.title,
            &note.body,
            &note.frontmatter.tags,
        )?;

        let links = markdown::extract_wikilinks(&note.body);
        graph.update_note(&note.id, &note.frontmatter.tags, &links)?;

        count += 1;
        if count % 100 == 0 {
            info!("indexed {count} notes...");
        }
    }

    writer.commit()
        .map_err(|e| agentic_note_core::error::AgenticError::Search(
            format!("commit: {e}"),
        ))?;

    info!("reindex complete: {count} notes");
    Ok(count)
}
