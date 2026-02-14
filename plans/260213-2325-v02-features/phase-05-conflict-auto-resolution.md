# Phase 5: Conflict Auto-Resolution Policies

## Context Links
- [Research: Conflict Resolution](/Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/research/researcher-iroh-p2p-crdt.md)
- [CAS merge.rs](/Users/phuc/Developer/agentic-note/crates/cas/src/merge.rs)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P2
- **Status:** completed
- **Effort:** 3h
- **Depends on:** Phase 1
- **Description:** Add auto-resolution policies (newest-wins, longest-wins, merge-both, manual) to CAS merge. Configurable per-vault and per-PARA. Auto-resolved conflicts create CAS snapshot.

## Key Insights
- Current `three_way_merge()` always marks both-changed files as `ConflictInfo` (manual only)
- `ConflictPolicy` enum from Phase 1: NewestWins, LongestWins, MergeBoth, Manual
- Policy resolution: stage-specific override > PARA category override > vault default > global default (Manual)
- `newest-wins`: compare `modified` timestamp in YAML frontmatter
- `longest-wins`: compare body byte length
- `merge-both`: concatenate bodies under `<<<< LOCAL` / `>>>> REMOTE` headers
- Auto-resolved conflicts must create a CAS snapshot for audit trail

## Requirements

### Functional
- F1: `three_way_merge()` accepts `ConflictPolicy` parameter
- F2: `resolve_conflict()` function dispatches to policy-specific handlers
- F3: NewestWins: parse frontmatter of both versions, keep newer `modified` timestamp
- F4: LongestWins: compare byte length of full file content, keep longer
- F5: MergeBoth: concatenate both under conflict markers, return merged content
- F6: Manual: existing behavior (return ConflictInfo unchanged)
- F7: `SyncConfig.conflict_overrides` maps PARA category string to policy
- F8: Auto-resolved conflicts produce a new blob in BlobStore + snapshot

### Non-Functional
- Policy resolution <1ms per conflict
- No data loss — merge-both preserves everything, others clearly pick a version

## Architecture

```
crates/cas/src/
├── merge.rs           # modify: add policy parameter, call resolve_conflict()
├── conflict_policy.rs # NEW: resolve_conflict(), newest_wins(), longest_wins(), merge_both()
├── cas.rs             # modify: three_way_merge() signature change
└── blob.rs            # unchanged (used by conflict_policy to store merged blobs)
```

### Policy Resolution Order
```
1. Check stage.conflict_policy (if called from sync context)
2. Check sync.conflict_overrides[note.para] (per-PARA)
3. Check sync.default_conflict_policy (vault-level)
4. Fallback: Manual
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/cas/src/conflict_policy.rs` | create | Policy resolver + handlers |
| `/Users/phuc/Developer/agentic-note/crates/cas/src/merge.rs` | modify | Accept ConflictPolicy, call resolve_conflict for conflicts |
| `/Users/phuc/Developer/agentic-note/crates/cas/src/cas.rs` | modify | Update three_way_merge signature |
| `/Users/phuc/Developer/agentic-note/crates/cas/src/lib.rs` | modify | +mod conflict_policy, re-exports |
| `/Users/phuc/Developer/agentic-note/crates/cas/Cargo.toml` | modify | +agentic-note-core dep (for ConflictPolicy type) if not already |

## Implementation Steps

1. Create `crates/cas/src/conflict_policy.rs`:
   - `resolve_conflict(store: &BlobStore, info: &ConflictInfo, policy: &ConflictPolicy) -> Result<ConflictResolution>`
   - `ConflictResolution` enum:
     ```rust
     pub enum ConflictResolution {
         Resolved { merged_blob_id: ObjectId, description: String },
         Unresolved(ConflictInfo),  // Manual policy
     }
     ```
   - `newest_wins(store, info) -> Result<ConflictResolution>`:
     - Load both blobs, parse YAML frontmatter for `modified` field
     - Keep version with later timestamp
     - Return Resolved with winning blob id
   - `longest_wins(store, info) -> Result<ConflictResolution>`:
     - Load both blobs, compare `content.len()`
     - Keep longer version
   - `merge_both(store, info) -> Result<ConflictResolution>`:
     - Load both blobs
     - Create merged content: `"<<<< LOCAL\n{local}\n====\n{remote}\n>>>> REMOTE\n"`
     - Store merged as new blob
     - Return Resolved with new blob id
2. Modify `crates/cas/src/merge.rs`:
   - Change `three_way_merge()` signature:
     ```rust
     pub fn three_way_merge(
         store: &BlobStore,
         ancestor: &ObjectId,
         local: &ObjectId,
         remote: &ObjectId,
         policy: &ConflictPolicy,
     ) -> Result<MergeResult>
     ```
   - In the conflict branch (both-changed), call `resolve_conflict(store, info, policy)`
   - If Resolved → add to `applied`, store resolution description
   - If Unresolved → add to `conflicts` (current behavior)
   - Update `MergeResult`:
     ```rust
     pub struct MergeResult {
         pub applied: Vec<String>,
         pub conflicts: Vec<ConflictInfo>,
         pub auto_resolved: Vec<AutoResolution>,  // NEW
     }
     pub struct AutoResolution {
         pub path: String,
         pub policy: ConflictPolicy,
         pub result_blob_id: ObjectId,
         pub description: String,
     }
     ```
3. Modify `crates/cas/src/cas.rs`:
   - Update `Cas::three_way_merge()` to accept and forward `ConflictPolicy`
   - After auto-resolution: create snapshot if any auto-resolved entries
4. Update `crates/cas/src/lib.rs` with new module and re-exports.
5. Write tests:
   - NewestWins: local newer → picks local
   - NewestWins: remote newer → picks remote
   - LongestWins: longer body wins
   - MergeBoth: both bodies present in merged output
   - Manual: unchanged behavior (ConflictInfo returned)
   - Auto-resolved creates snapshot

## Todo List

- [ ] Create conflict_policy.rs with resolve_conflict()
- [ ] Implement newest_wins()
- [ ] Implement longest_wins()
- [ ] Implement merge_both()
- [ ] Update three_way_merge() signature with policy param
- [ ] Add AutoResolution to MergeResult
- [ ] Update Cas facade
- [ ] Auto-snapshot after resolution
- [ ] Update lib.rs re-exports
- [ ] Tests: each policy
- [ ] Tests: fallback to Manual
- [ ] cargo check + cargo test pass

## Success Criteria
- `three_way_merge(... ConflictPolicy::NewestWins)` auto-resolves by timestamp
- Auto-resolved conflicts appear in `MergeResult.auto_resolved`
- Manual policy returns conflicts in `MergeResult.conflicts` (backward compat)
- CAS snapshot created after auto-resolution

## Risk Assessment
- **Low:** Frontmatter parsing for timestamps already exists in vault crate
- **Medium:** merge-both creates larger files — acceptable tradeoff for no data loss
- **Low:** Existing callers of three_way_merge must be updated (only cas.rs facade)

## Security Considerations
- No user input in conflict resolution — only comparing file content
- Merged content preserved in CAS for audit

## Next Steps
- Phase 7 (P2P Sync) uses conflict policies during sync merge
