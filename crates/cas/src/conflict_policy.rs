use crate::blob::BlobStore;
use crate::hash::ObjectId;
use crate::merge::ConflictInfo;
use agentic_note_core::types::ConflictPolicy;
use agentic_note_core::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Outcome of applying a conflict resolution policy to a single conflict.
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    /// The conflict was resolved automatically; `merged_blob_id` is the winning or merged blob.
    Resolved {
        merged_blob_id: ObjectId,
        description: String,
    },
    /// The conflict could not be resolved automatically and requires manual intervention.
    Unresolved(ConflictInfo),
}

/// Information recorded for each automatically resolved conflict during a merge.
#[derive(Debug, Clone)]
pub struct AutoResolution {
    pub path: String,
    pub policy: ConflictPolicy,
    pub result_blob_id: ObjectId,
    pub description: String,
}

/// Minimal frontmatter structure for parsing the `modified` timestamp.
#[derive(Debug, Deserialize)]
struct PartialFrontMatter {
    modified: Option<DateTime<Utc>>,
}

/// Apply `policy` to `info`, returning either a resolved or unresolved result.
pub fn resolve_conflict(
    store: &BlobStore,
    info: &ConflictInfo,
    policy: &ConflictPolicy,
) -> Result<ConflictResolution> {
    match policy {
        ConflictPolicy::NewestWins => newest_wins(store, info),
        ConflictPolicy::LongestWins => longest_wins(store, info),
        ConflictPolicy::MergeBoth => merge_both(store, info),
        ConflictPolicy::Manual => Ok(ConflictResolution::Unresolved(info.clone())),
    }
}

/// Parse the `modified` field from YAML frontmatter (`---\n...\n---\n`).
fn parse_modified(content: &[u8]) -> Option<DateTime<Utc>> {
    let text = std::str::from_utf8(content).ok()?;
    let stripped = text.strip_prefix("---\n")?;
    let end = stripped.find("\n---\n")?;
    let yaml_block = &stripped[..end];
    let fm: PartialFrontMatter = serde_yaml::from_str(yaml_block).ok()?;
    fm.modified
}

/// Keep the blob with the later `modified` frontmatter timestamp.
/// Falls back to version_a when timestamps are unavailable or equal.
fn newest_wins(store: &BlobStore, info: &ConflictInfo) -> Result<ConflictResolution> {
    let a_bytes = store.load(&info.version_a)?;
    let b_bytes = store.load(&info.version_b)?;

    let a_ts = parse_modified(&a_bytes);
    let b_ts = parse_modified(&b_bytes);

    let (winner_id, description) = match (a_ts, b_ts) {
        (Some(ta), Some(tb)) if tb > ta => (
            info.version_b.clone(),
            format!("newest-wins: chose version_b ({})", tb),
        ),
        _ => (
            info.version_a.clone(),
            "newest-wins: chose version_a (fallback or a is newer)".to_string(),
        ),
    };

    Ok(ConflictResolution::Resolved {
        merged_blob_id: winner_id,
        description,
    })
}

/// Keep the longer blob by byte length; ties go to version_a.
fn longest_wins(store: &BlobStore, info: &ConflictInfo) -> Result<ConflictResolution> {
    let a_bytes = store.load(&info.version_a)?;
    let b_bytes = store.load(&info.version_b)?;

    let (winner_id, description) = if b_bytes.len() > a_bytes.len() {
        (
            info.version_b.clone(),
            format!(
                "longest-wins: chose version_b ({} bytes > {} bytes)",
                b_bytes.len(),
                a_bytes.len()
            ),
        )
    } else {
        (
            info.version_a.clone(),
            format!(
                "longest-wins: chose version_a ({} bytes >= {} bytes)",
                a_bytes.len(),
                b_bytes.len()
            ),
        )
    };

    Ok(ConflictResolution::Resolved {
        merged_blob_id: winner_id,
        description,
    })
}

/// Combine both versions with conflict markers and store as a new blob.
fn merge_both(store: &BlobStore, info: &ConflictInfo) -> Result<ConflictResolution> {
    let a_bytes = store.load(&info.version_a)?;
    let b_bytes = store.load(&info.version_b)?;

    let local_content = String::from_utf8_lossy(&a_bytes);
    let remote_content = String::from_utf8_lossy(&b_bytes);

    let merged = format!(
        "<<<< LOCAL\n{}\n====\n{}\n>>>> REMOTE\n",
        local_content, remote_content
    );

    let merged_id = store.store(merged.as_bytes())?;

    Ok(ConflictResolution::Resolved {
        merged_blob_id: merged_id,
        description: format!(
            "merge-both: combined {} bytes + {} bytes into conflict markers",
            a_bytes.len(),
            b_bytes.len()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_store() -> BlobStore {
        let dir = env::temp_dir().join(format!(
            "cas-policy-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        BlobStore::new(dir.join("objects"))
    }

    fn make_note(modified: &str, body: &str) -> Vec<u8> {
        format!("---\nmodified: {}\n---\n{}", modified, body).into_bytes()
    }

    #[test]
    fn newest_wins_picks_later_timestamp() {
        let store = temp_store();
        let older = make_note("2024-01-01T00:00:00Z", "old content");
        let newer = make_note("2024-06-01T00:00:00Z", "new content");
        let id_a = store.store(&older).unwrap();
        let id_b = store.store(&newer).unwrap();

        let info = ConflictInfo {
            path: "note.md".into(),
            version_a: id_a,
            version_b: id_b.clone(),
        };
        let res = resolve_conflict(&store, &info, &ConflictPolicy::NewestWins).unwrap();
        match res {
            ConflictResolution::Resolved { merged_blob_id, .. } => {
                assert_eq!(merged_blob_id, id_b, "newer version_b should win");
            }
            ConflictResolution::Unresolved(_) => panic!("expected resolved"),
        }
    }

    #[test]
    fn newest_wins_falls_back_to_version_a_when_no_frontmatter() {
        let store = temp_store();
        let id_a = store.store(b"no frontmatter a").unwrap();
        let id_b = store.store(b"no frontmatter b").unwrap();

        let info = ConflictInfo {
            path: "note.md".into(),
            version_a: id_a.clone(),
            version_b: id_b,
        };
        let res = resolve_conflict(&store, &info, &ConflictPolicy::NewestWins).unwrap();
        match res {
            ConflictResolution::Resolved { merged_blob_id, .. } => {
                assert_eq!(merged_blob_id, id_a, "should fall back to version_a");
            }
            ConflictResolution::Unresolved(_) => panic!("expected resolved"),
        }
    }

    #[test]
    fn longest_wins_picks_longer_blob() {
        let store = temp_store();
        let short = b"short";
        let long = b"this is a much longer content blob";
        let id_a = store.store(short).unwrap();
        let id_b = store.store(long).unwrap();

        let info = ConflictInfo {
            path: "note.md".into(),
            version_a: id_a,
            version_b: id_b.clone(),
        };
        let res = resolve_conflict(&store, &info, &ConflictPolicy::LongestWins).unwrap();
        match res {
            ConflictResolution::Resolved { merged_blob_id, .. } => {
                assert_eq!(merged_blob_id, id_b, "longer version_b should win");
            }
            ConflictResolution::Unresolved(_) => panic!("expected resolved"),
        }
    }

    #[test]
    fn longest_wins_tie_goes_to_version_a() {
        let store = temp_store();
        let id_a = store.store(b"same").unwrap();
        let id_b = store.store(b"also").unwrap(); // same length as "same"
        let info = ConflictInfo {
            path: "note.md".into(),
            version_a: id_a.clone(),
            version_b: id_b,
        };
        let res = resolve_conflict(&store, &info, &ConflictPolicy::LongestWins).unwrap();
        match res {
            ConflictResolution::Resolved { merged_blob_id, .. } => {
                assert_eq!(merged_blob_id, id_a, "tie should go to version_a");
            }
            ConflictResolution::Unresolved(_) => panic!("expected resolved"),
        }
    }

    #[test]
    fn merge_both_contains_conflict_markers() {
        let store = temp_store();
        let id_a = store.store(b"local content").unwrap();
        let id_b = store.store(b"remote content").unwrap();

        let info = ConflictInfo {
            path: "note.md".into(),
            version_a: id_a,
            version_b: id_b,
        };
        let res = resolve_conflict(&store, &info, &ConflictPolicy::MergeBoth).unwrap();
        match res {
            ConflictResolution::Resolved { merged_blob_id, .. } => {
                let merged_bytes = store.load(&merged_blob_id).unwrap();
                let merged_text = String::from_utf8(merged_bytes).unwrap();
                assert!(merged_text.contains("<<<< LOCAL"));
                assert!(merged_text.contains("===="));
                assert!(merged_text.contains(">>>> REMOTE"));
                assert!(merged_text.contains("local content"));
                assert!(merged_text.contains("remote content"));
            }
            ConflictResolution::Unresolved(_) => panic!("expected resolved"),
        }
    }

    #[test]
    fn manual_policy_returns_unresolved() {
        let store = temp_store();
        let id_a = store.store(b"version a").unwrap();
        let id_b = store.store(b"version b").unwrap();

        let info = ConflictInfo {
            path: "note.md".into(),
            version_a: id_a,
            version_b: id_b,
        };
        let res = resolve_conflict(&store, &info, &ConflictPolicy::Manual).unwrap();
        assert!(
            matches!(res, ConflictResolution::Unresolved(_)),
            "manual policy must return Unresolved"
        );
    }
}
