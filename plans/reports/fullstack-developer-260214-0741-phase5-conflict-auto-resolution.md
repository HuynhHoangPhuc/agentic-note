# Phase Implementation Report

### Executed Phase
- Phase: Phase 5 - Conflict Auto-Resolution Policies
- Plan: v0.2.0
- Status: completed

### Files Modified
- `crates/cas/src/conflict_policy.rs` — created, 241 lines (logic + 6 tests)
- `crates/cas/src/merge.rs` — rewritten, 197 lines (added AutoResolution, updated MergeResult, updated three_way_merge signature, extracted apply_policy helper)
- `crates/cas/src/lib.rs` — updated, 19 lines (added conflict_policy module + re-exports)
- `crates/cas/Cargo.toml` — added `serde_yaml = { workspace = true }`

### Tasks Completed
- [x] Created `conflict_policy.rs` with `ConflictResolution` enum
- [x] Implemented `resolve_conflict()` dispatcher
- [x] Implemented `newest_wins` (parses YAML frontmatter `modified` field via serde_yaml)
- [x] Implemented `longest_wins` (byte length comparison, tie → version_a)
- [x] Implemented `merge_both` (conflict markers, stores new blob)
- [x] `Manual` → returns `Unresolved(info.clone())`
- [x] Defined `AutoResolution` struct in `merge.rs`
- [x] Added `auto_resolved: Vec<AutoResolution>` to `MergeResult`
- [x] Updated `three_way_merge` signature to accept `policy: &ConflictPolicy`
- [x] Extracted `apply_policy` helper; delete-vs-modify (empty blob ids) go straight to conflicts
- [x] Updated `lib.rs` with new module and re-exports
- [x] Added serde_yaml dep to cas Cargo.toml
- [x] 6 new tests covering all 4 policies + edge cases

### Tests Status
- Type check: pass (`cargo check -p agentic-note-cas` — clean)
- Unit tests: pass — 14/14 (8 pre-existing + 6 new)
  - `conflict_policy::tests::newest_wins_picks_later_timestamp` ok
  - `conflict_policy::tests::newest_wins_falls_back_to_version_a_when_no_frontmatter` ok
  - `conflict_policy::tests::longest_wins_picks_longer_blob` ok
  - `conflict_policy::tests::longest_wins_tie_goes_to_version_a` ok
  - `conflict_policy::tests::merge_both_contains_conflict_markers` ok
  - `conflict_policy::tests::manual_policy_returns_unresolved` ok
- Workspace check: `crates/agent` has pre-existing `E0382` in `plugin/runner.rs` (unrelated, not modified)

### Issues Encountered
- `crates/agent/src/plugin/runner.rs` has a pre-existing `E0382` compile error (child moved by `wait_with_output` then borrowed by `kill`) — not introduced by Phase 5, file not in Phase 5 ownership.

### Next Steps
- Phase 5 unblocks any phase that calls `three_way_merge` — callers must pass a `ConflictPolicy` arg
- `crates/sync/src/merge_driver.rs` stub (Phase 7) is the primary consumer
- Pre-existing agent bug should be fixed in a separate task
