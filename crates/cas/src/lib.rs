//! Content-addressable storage with SHA-256 hashing, snapshots, and 3-way merge.
//!
//! Exposes the `Cas` facade plus tree, snapshot, diff, and merge utilities
//! for versioning vault contents.

pub mod blob;
pub mod cas;
pub mod conflict_policy;
pub mod diff;
pub mod hash;
pub mod merge;
pub mod restore;
pub mod semantic_merge;
pub mod snapshot;
pub mod tree;

pub use blob::BlobStore;
pub use cas::Cas;
pub use conflict_policy::{resolve_conflict, AutoResolution, ConflictResolution};
pub use diff::{diff_trees, DiffEntry, DiffStatus};
pub use hash::{hash_bytes, hash_file, ObjectId};
pub use merge::{build_merged_tree, three_way_merge, ConflictInfo, MergeResult};
pub use restore::restore;
pub use semantic_merge::{try_paragraph_merge, ConflictHunk, MergeAttempt};
pub use snapshot::Snapshot;
pub use tree::{EntryType, Tree, TreeEntry};
