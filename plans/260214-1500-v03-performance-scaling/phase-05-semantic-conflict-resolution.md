---
phase: 5
title: "Semantic Conflict Resolution"
status: complete
effort: 2.5h
depends_on: [1]
---

## Context Links

- [CAS merge.rs](../../crates/cas/src/merge.rs)
- [CAS blob.rs](../../crates/cas/src/blob.rs)
- [Agent agents/mod.rs](../../crates/agent/src/agents/mod.rs)
- [Core types.rs ConflictPolicy](../../crates/core/src/types.rs)
- [Research: Semantic Merge](research/researcher-metrics-workers.md)

## Overview

Replace binary A/B conflict resolution with tiered semantic merge: (1) `diffy` paragraph-level 3-way diff for clean merges, (2) LLM-assisted merge via existing agent infrastructure for true conflicts, (3) manual fallback. Triggered when `ConflictPolicy::SemanticMerge` is active.

## Key Insights

- `diffy` 0.4 provides pure-Rust 3-way merge; zero deps, < 1ms for typical notes
- Most conflicts are non-overlapping edits (different paragraphs) -- `diffy` handles these automatically
- For overlapping edits: send both versions + ancestor to LLM via existing `crates/agent` infra (DRY)
- Existing `apply_policy()` in `merge.rs` is the hook point -- extend, don't replace
- Keep `Manual` as ultimate fallback -- never lose data

## Requirements

**Functional:**
- Tier 1: `diffy::merge()` at paragraph level. If clean merge (no conflicts): auto-apply
- Tier 2: If diffy reports conflicts, extract conflicting hunks and send to LLM merge agent
- Tier 3: If LLM unavailable or confidence low: fall back to `Manual` (user picks A or B)
- New agent: `merge-assistant` in `crates/agent/src/agents/`
- Works with existing `three_way_merge()` in `crates/cas/src/merge.rs`

**Non-functional:**
- Tier 1 (diffy): < 5ms per note
- Tier 2 (LLM): < 3s per conflicted note (network latency)
- No data loss -- manual fallback always available

## Architecture

```
three_way_merge() detects both-changed blob
  └── apply_policy(SemanticMerge)
        ├── Tier 1: diffy::merge(ancestor, local, remote)
        │     ├── Clean merge → AutoResolution
        │     └── Has conflicts → Tier 2
        ├── Tier 2: MergeAssistant agent
        │     ├── Prompt: "Merge these note versions preserving intent"
        │     ├── Input: ancestor + local + remote + conflict hunks
        │     ├── Output: merged text
        │     └── Fallback → Tier 3
        └── Tier 3: ConflictInfo (manual resolution)
```

## Related Code Files

**Create:**
- `crates/cas/src/semantic_merge.rs` -- diffy integration, tiered merge logic
- `crates/agent/src/agents/merge_assistant.rs` -- LLM merge agent

**Modify:**
- `crates/cas/src/merge.rs` -- extend `apply_policy()` to handle `SemanticMerge`
- `crates/cas/src/conflict_policy.rs` -- add semantic merge case
- `crates/cas/src/lib.rs` -- add `pub mod semantic_merge;`
- `crates/cas/Cargo.toml` -- add `diffy`
- `crates/agent/src/agents/mod.rs` -- register `merge_assistant`
- Root `Cargo.toml` -- add `diffy` workspace dep

## Implementation Steps

1. Add workspace dep to root `Cargo.toml`:
   ```toml
   diffy = "0.4"
   ```

2. Add to `crates/cas/Cargo.toml`:
   ```toml
   diffy = { workspace = true }
   ```

3. Create `crates/cas/src/semantic_merge.rs`:
   - `pub fn try_paragraph_merge(ancestor: &str, local: &str, remote: &str) -> MergeAttempt`
   - `MergeAttempt` enum: `Clean(String)` | `HasConflicts { merged_partial: String, conflicts: Vec<ConflictHunk> }`
   - Split text by `\n\n` (paragraph boundaries) before calling `diffy::merge()`
   - If `diffy::merge()` returns no conflict markers: return `Clean`
   - Otherwise: extract conflict hunks with line ranges

4. Create `crates/agent/src/agents/merge_assistant.rs`:
   - Implement `AgentHandler` trait
   - `agent_id()` returns `"merge-assistant"`
   - `execute()` builds prompt:
     ```
     You are merging two versions of a markdown note. Preserve the intent of both edits.

     ## Ancestor version:
     {ancestor}

     ## Version A (local):
     {local_hunks}

     ## Version B (remote):
     {remote_hunks}

     Output ONLY the merged text for the conflicting sections.
     ```
   - Parse LLM response as merged text
   - Return merged text via `StageOutput`

5. Extend `apply_policy()` in `crates/cas/src/merge.rs`:
   - Add match arm for `ConflictPolicy::SemanticMerge`:
     a. Load ancestor, local, remote blob content from `BlobStore`
     b. Call `semantic_merge::try_paragraph_merge()`
     c. If `Clean(merged)`: store merged blob, return `AutoResolution`
     d. If `HasConflicts`: return `ConflictInfo` (LLM merge will be invoked by caller)
   - Note: LLM merge is async, but `apply_policy` is sync. Solution: `apply_policy` returns a new variant `NeedsLlmMerge { hunks }` that the caller handles async

6. In `merge_driver.rs` (sync crate), handle `NeedsLlmMerge`:
   - If agent infrastructure available: invoke `MergeAssistant`
   - If unavailable (offline): fall back to `Manual`

7. Register `MergeAssistant` in `crates/agent/src/agents/mod.rs`.

8. Run `cargo check -p agentic-note-cas -p agentic-note-agent`.

9. Unit tests for `semantic_merge.rs`:
   - Test non-overlapping edits → clean merge
   - Test overlapping edits → HasConflicts
   - Test identical changes → clean (dedup)
   - Test one side empty (deletion) → conflict

10. Unit test for `MergeAssistant`: mock LLM provider, verify prompt structure.

## Todo List

- [ ] Add `diffy` workspace dep
- [ ] Create `semantic_merge.rs` in CAS crate
- [ ] Implement `try_paragraph_merge()` with diffy
- [ ] Create `merge_assistant.rs` agent
- [ ] Extend `apply_policy()` for `SemanticMerge`
- [ ] Handle async LLM merge in merge_driver
- [ ] Register merge-assistant agent
- [ ] Unit tests: paragraph merge scenarios
- [ ] Unit test: merge assistant prompt
- [ ] `cargo check` passes

## Success Criteria

- Non-overlapping paragraph edits auto-merged without user intervention
- Overlapping edits resolved by LLM with coherent output
- LLM unavailable gracefully falls back to manual
- No data loss in any merge path
- Existing conflict policies unchanged

## Risk Assessment

- **LLM hallucination**: merged text may lose content. Mitigation: always store both original versions in CAS for recovery; log merge diff
- **diffy edge cases**: unusual markdown (tables, code blocks) may split badly on `\n\n`. Mitigation: treat entire code blocks as single units
- **Async in sync path**: `apply_policy` is sync; LLM call is async. Mitigation: return `NeedsLlmMerge` variant, handle async at caller level

## Security Considerations

- Note content sent to LLM provider (same as existing agents) -- user controls provider choice
- No new API keys required -- reuses existing LLM config
- Merged content validated by storing in CAS with SHA-256

## Next Steps

Phase 8 (Integration) tests semantic merge with batch sync.
