# Phase 05: CAS & Versioning

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 02 (vault & notes)
- Research: [Architecture Brainstorm](../reports/brainstorm-260213-1552-agentic-note-app.md)

## Overview
- **Priority:** P1 (required for sync + undo)
- **Status:** pending
- **Effort:** 12h
- **Description:** SHA-256 content-addressed blob store, tree objects, snapshots with merkle trees, diff computation, three-way merge, conflict detection, CLI snapshot/restore commands.

## Key Insights
- Git-like but simpler: blob, tree, snapshot (no commits/branches/refs)
- Blob = SHA-256 hash of file content → stored at `.agentic/cas/objects/{hash[0..2]}/{hash[2..]}`
- Tree = serialized directory listing: `[{name, type(blob|tree), hash}]`
- Snapshot = root tree hash + timestamp + device_id + optional signature
- Three-way merge: common ancestor + local + remote → auto-merge or .conflict file
- sha2 crate for hashing — pure Rust, fast

## Requirements

**Functional:**
- Store file content as content-addressed blobs (deduplication)
- Represent directory structure as tree objects
- Create snapshot of entire vault state
- Compute diff between two snapshots (list of added/modified/deleted files)
- Three-way merge between two snapshots with common ancestor
- Generate `.conflict` files for unresolvable conflicts
- Restore vault to any previous snapshot
- `agentic-note snapshot create [--message <m>]`
- `agentic-note snapshot list`
- `agentic-note snapshot diff <hash1> <hash2>`
- `agentic-note snapshot restore <hash>`

**Non-functional:**
- Snapshot creation < 2s for 1k files
- CAS deduplication: unchanged files cost zero additional storage
- Object format is portable (same on all platforms)

## Architecture

```
crates/cas/src/
├── lib.rs          # pub mod re-exports, Cas struct
├── hash.rs         # SHA-256 hashing utilities
├── blob.rs         # Blob store (read/write content-addressed)
├── tree.rs         # Tree objects (directory listing)
├── snapshot.rs     # Snapshot creation/listing/metadata
├── diff.rs         # Merkle tree diff between snapshots
├── merge.rs        # Three-way merge + conflict detection
└── restore.rs      # Restore vault from snapshot

.agentic/cas/
├── objects/        # {hash[0..2]}/{hash[2..]} blob/tree storage
└── snapshots/      # snapshot metadata files (JSON)
```

## Related Code Files

**Create:**
- `crates/cas/Cargo.toml` (update stub)
- `crates/cas/src/lib.rs`
- `crates/cas/src/hash.rs`
- `crates/cas/src/blob.rs`
- `crates/cas/src/tree.rs`
- `crates/cas/src/snapshot.rs`
- `crates/cas/src/diff.rs`
- `crates/cas/src/merge.rs`
- `crates/cas/src/restore.rs`

**Modify:**
- `crates/cli/src/commands/mod.rs` — add Snapshot subcommand
- `crates/cli/Cargo.toml` — add cas dep

## Cargo.toml Dependencies
```toml
[dependencies]
agentic-note-core = { path = "../core" }
agentic-note-vault = { path = "../vault" }
sha2 = "0.10"
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
walkdir = "2"
anyhow = { workspace = true }
tracing = { workspace = true }
```

## Implementation Steps

1. **`hash.rs`:**
   - `fn hash_bytes(data: &[u8]) -> String` — SHA-256, return hex string
   - `fn hash_file(path: &Path) -> Result<String>` — stream file, return hash
   - `ObjectId` type alias for String (hex SHA-256)

2. **`blob.rs`:**
   - `BlobStore { objects_dir: PathBuf }`
   - `store(data: &[u8]) -> Result<ObjectId>` — hash, write to `{prefix}/{rest}`, skip if exists
   - `load(id: &ObjectId) -> Result<Vec<u8>>` — read from object path
   - `exists(id: &ObjectId) -> bool`
   - Object path: `objects/{id[0..2]}/{id[2..]}`

3. **`tree.rs`:**
   - `TreeEntry { name: String, entry_type: EntryType, hash: ObjectId }`
   - `EntryType` enum: Blob, Tree
   - `Tree { entries: Vec<TreeEntry> }` — sorted by name for deterministic hashing
   - `Tree::from_dir(vault: &Path, store: &BlobStore, exclude: &[&str]) -> Result<(Tree, ObjectId)>`
     - Recursively walk dir, store blobs, build tree, store tree blob, return hash
     - Exclude `.agentic/` from tree
   - `Tree::load(store: &BlobStore, id: &ObjectId) -> Result<Tree>` — deserialize

4. **`snapshot.rs`:**
   - `Snapshot { id: ObjectId, root_tree: ObjectId, timestamp: DateTime<Utc>, device_id: String, message: Option<String> }`
   - `Snapshot::create(vault: &Path, cas: &Cas, message: Option<String>) -> Result<Snapshot>`
     - Build tree from vault, store snapshot metadata as JSON in `snapshots/`
   - `Snapshot::list(cas: &Cas) -> Result<Vec<Snapshot>>` — sorted by timestamp desc
   - `Snapshot::load(cas: &Cas, id: &ObjectId) -> Result<Snapshot>`

5. **`diff.rs`:**
   - `DiffEntry { path: String, status: DiffStatus }` where `DiffStatus` = Added, Modified, Deleted
   - `fn diff_trees(store: &BlobStore, tree_a: &ObjectId, tree_b: &ObjectId) -> Result<Vec<DiffEntry>>`
     - Recursive tree comparison by hash; hash match = skip entire subtree
   - `fn diff_snapshots(cas: &Cas, snap_a: &ObjectId, snap_b: &ObjectId) -> Result<Vec<DiffEntry>>`

6. **`merge.rs`:**
   <!-- Updated: Validation Session 1 - Pick A or B instead of git-style merge markers -->
   - `MergeResult { applied: Vec<String>, conflicts: Vec<ConflictInfo> }`
   - `ConflictInfo { path: String, version_a: ObjectId, version_b: ObjectId }`
   - `fn three_way_merge(store: &BlobStore, ancestor: &ObjectId, local: &ObjectId, remote: &ObjectId) -> Result<MergeResult>`
     - Compare each file: if only one side changed, take that change
     - If both changed differently, record as `ConflictInfo` — user picks A or B
   - CLI conflict resolution: show both versions side-by-side, user selects which to keep
   - No git-style `<<<<` merge markers — simpler UX

7. **`restore.rs`:**
   - `fn restore(vault: &Path, cas: &Cas, snapshot_id: &ObjectId) -> Result<()>`
     - Create backup snapshot of current state first
     - Rebuild vault files from tree objects
     - Delete files not in snapshot tree

8. **`lib.rs`:** `Cas` facade struct
   - Holds `BlobStore`, snapshot dir path, device_id
   - `Cas::open(vault_path: &Path) -> Result<Self>` — ensure .agentic/cas/ dirs exist

9. **CLI commands:** snapshot create/list/diff/restore via clap subcommands

## Todo List
- [ ] Implement SHA-256 hashing
- [ ] Implement blob store (content-addressed)
- [ ] Implement tree objects (directory serialization)
- [ ] Implement snapshot create/list
- [ ] Implement merkle tree diff
- [ ] Implement three-way merge + conflict detection
- [ ] Implement restore from snapshot
- [ ] Add snapshot CLI commands
- [ ] Write tests for diff and merge

## Success Criteria
- Create snapshot -> modify file -> create snapshot -> diff shows modification
- Identical files produce identical hashes (deduplication works)
- Three-way merge: only-one-side-changed -> auto-resolve
- Three-way merge: both-sides-changed-differently -> .conflict file
- Restore reverts vault to exact snapshot state

## Risk Assessment
- **Large files:** binary attachments in vault could bloat CAS — consider excluding by extension
- **Tree hash determinism:** sort entries by name, use consistent serialization (JSON with sorted keys)
- **Restore data loss:** always create backup snapshot before restore

## Security Considerations
- SHA-256 for integrity — not for security-sensitive signing (use Ed25519 separately for sync)
- CAS objects are local-only — no network exposure until sync phase

## Next Steps
- ~~Phase 06 (P2P Sync) uses CAS for merkle diff sync protocol~~ — DEFERRED to v2
- Snapshot undo supports AgentSpace revert (Phase 08)
- CAS diff/merge ready for when P2P sync is added in v2
