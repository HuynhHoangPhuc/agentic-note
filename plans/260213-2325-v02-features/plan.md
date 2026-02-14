---
title: "agentic-note v0.2.0 — Six Features"
description: "P2P sync, semantic search, DAG pipelines, error recovery, conflict policies, agent plugins"
status: completed
priority: P1
effort: 32h
branch: main
tags: [v0.2.0, p2p, embeddings, dag, plugins, sync]
created: 2026-02-13
---

# agentic-note v0.2.0 Implementation Plan

## Summary

6 features across 8 phases. Estimated ~32h total. All features build on Phase 1 (shared types/config). P2P sync is heaviest (~8h), placed last so other features ship independently.

## Dependency Graph

```
Phase 1 (Core Types) ─┬─> Phase 2 (Embeddings)
                       ├─> Phase 3 (DAG) ──> Phase 4 (Error Recovery)
                       ├─> Phase 5 (Conflict) ──> Phase 7 (P2P Sync)
                       ├─> Phase 6 (Plugins, depends on Phase 3)
                       └─────────────────────────> Phase 8 (Integration)
```

## Phases

| # | Phase | Status | Effort | File |
|---|-------|--------|--------|------|
| 1 | Core Types & Config Extensions | completed | 2h | [phase-01](phase-01-core-types-config.md) |
| 2 | Embeddings & Semantic Search | completed | 5h | [phase-02](phase-02-embeddings-semantic-search.md) |
| 3 | DAG Pipeline Engine | completed | 4h | [phase-03](phase-03-dag-pipeline-engine.md) |
| 4 | Pipeline Error Recovery | completed | 3h | [phase-04](phase-04-pipeline-error-recovery.md) |
| 5 | Conflict Auto-Resolution Policies | completed | 3h | [phase-05](phase-05-conflict-auto-resolution.md) |
| 6 | Custom Agent Plugin System | completed | 3h | [phase-06](phase-06-custom-agent-plugins.md) |
| 7 | P2P Sync via iroh | completed | 8h | [phase-07](phase-07-p2p-sync-iroh.md) |
| 8 | Integration & Testing | completed | 4h | [phase-08](phase-08-integration-testing.md) |

## Key Dependencies (workspace additions)

```toml
# Phase 1
petgraph = "0.6"
# Phase 2
ort = "2.0.0-rc.11"
sqlite-vec = "0.1"      # or vendored C source
indicatif = "0.17"
# Phase 7
iroh = "0.30"            # pin exact version
iroh-blobs = "0.30"
ed25519-dalek = "2"
```

## New Crate

- `crates/sync` — P2P sync via iroh (Phase 7)

## Risk Summary

1. **iroh API instability** — thin adapter trait, pinned version
2. **ort rc quality** — rc.11 close to stable, fallback: disable feature flag
3. **sqlite-vec Rust bindings** — may need raw `libsqlite3_sys` init call
4. **Binary size** — ort + iroh add ~20-30MB; feature-gate both

## Unresolved Questions

1. Self-hosted iroh relay in v0.2 or defer to v0.3?
2. sqlite-vec ANN timeline — brute-force OK for <10k notes?

## Validation Log

### Session 1 — 2026-02-13
**Trigger:** Initial plan validation before implementation
**Questions asked:** 6

#### Questions & Answers

1. **[Architecture]** The plan adds ort (~20MB) + iroh (~10MB) to the binary. Should embeddings and sync be cargo feature flags (opt-in at compile time)?
   - Options: Both feature-gated (Recommended) | Always included | Only sync gated
   - **Answer:** Both feature-gated
   - **Rationale:** Default binary stays ~45MB. Users opt-in with `cargo build --features embeddings,sync`. Conditional compilation needed in search, sync, cli crates.

2. **[Security]** Plugin system uses subprocess (KISS). But plugins run with same OS permissions as agentic-note. Is this acceptable security posture for v0.2?
   - Options: Subprocess OK for v0.2 (Recommended) | Add basic resource limits | Switch to WASM (wasmtime)
   - **Answer:** Subprocess OK for v0.2
   - **Rationale:** Document security warning in README. WASM sandboxing deferred to v0.3. Plugin resource limits (cgroups) also deferred.

3. **[Architecture]** The plan uses ort with `load-dynamic` feature (user provides libonnxruntime). Alternative: bundle ONNX Runtime in binary. Which approach?
   - Options: Bundle ONNX Runtime (Recommended) | load-dynamic (user provides) | Optional: bundle by default, load-dynamic flag
   - **Answer:** Bundle ONNX Runtime
   - **Rationale:** Zero user setup. `ort` default feature bundles libonnxruntime. Binary larger when embeddings feature enabled but works out of box.

4. **[Scope]** DAG condition evaluator supports only `==`/`!=` string comparison. Should it also support numeric comparisons and boolean logic (AND/OR)?
   - Options: String == / != only (Recommended) | Add numeric + boolean | Use mini expression language
   - **Answer:** String == / != only
   - **Rationale:** KISS. Covers 90% of use cases (checking PARA category, status, tags). Extend in v0.3 if needed.

5. **[Architecture]** Device registry stores peers in `.agentic/devices.json`. Alternative: use existing SQLite database (index.db). Which approach?
   - Options: JSON file (Recommended) | SQLite table in index.db | SQLite in separate devices.db
   - **Answer:** JSON file
   - **Rationale:** Human-readable, easy to inspect/edit/backup. Sufficient for <50 devices. Consistent with config.toml philosophy.

6. **[Risk]** iroh version: plan says 0.30 but actual latest may differ. Pin to exact version or use range?
   - Options: Pin exact version (Recommended) | Pin minor range | Latest at implementation time
   - **Answer:** Pin exact version
   - **Rationale:** `iroh = "=0.X.Y"` prevents surprise breakage. Check crates.io at Phase 7 implementation time, pin whatever is latest then.

#### Confirmed Decisions
- Feature gates: embeddings + sync behind cargo features — default binary small
- Plugins: subprocess, no sandboxing for v0.2 — document risk
- ONNX Runtime: bundled (no user setup) — only affects binary when feature enabled
- Conditions: `==`/`!=` string only — KISS, extend in v0.3
- Device registry: JSON file — human-readable, inspectable
- iroh: pin exact version at implementation time

#### Action Items
- [ ] Add `[features]` section to workspace and crate Cargo.tomls for `embeddings` and `sync`
- [ ] Remove `load-dynamic` from ort features in Phase 1, use default (bundled) instead
- [ ] Add `#[cfg(feature = "embeddings")]` gates in search crate and CLI
- [ ] Add `#[cfg(feature = "sync")]` gates in cli crate
- [ ] Document plugin security warning in Phase 6 + Phase 8
- [ ] Update iroh version to exact pin (=X.Y.Z) in Phase 7

#### Impact on Phases
- Phase 1: Add cargo feature definitions for `embeddings` and `sync`. Remove `load-dynamic` from ort.
- Phase 2: Wrap embedding code behind `#[cfg(feature = "embeddings")]`
- Phase 3: Condition evaluator confirmed `==`/`!=` only — no scope change
- Phase 6: Add security warning docs, confirmed subprocess — no scope change
- Phase 7: Device registry confirmed as JSON. iroh = exact pin. Wrap behind `#[cfg(feature = "sync")]`
- Phase 8: Add feature-flag testing matrix (default, +embeddings, +sync, +all)
