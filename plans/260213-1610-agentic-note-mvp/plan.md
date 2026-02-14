---
title: "Agentic-Note MVP"
description: "Local-first agentic note-taking app in Rust — CLI + MCP, PARA/Zettelkasten, CAS versioning, AgentSpace pipelines"
status: pending
priority: P1
effort: 70h
branch: main
tags: [rust, cli, mcp, pkm, agent, local-first]
created: 2026-02-13
---

# Agentic-Note MVP Plan

## Architecture

Cargo workspace with 7 crates (sync deferred). CLI + MCP server binary. Plain `.md` vault with YAML frontmatter. Custom CAS versioning. AgentSpace pipeline engine with 4 built-in agents. Dual-mode HITL (Interactive + AgentSpace).

## Dependency Graph

```
Phase 01: Core Types
  ├──> Phase 02: Vault & Notes
  │      ├──> Phase 03: CLI Interface
  │      │      └──> Phase 08: MCP Server
  │      ├──> Phase 04: Search & Index
  │      │      └──> Phase 07: Agents + Review
  │      ├──> Phase 05: CAS & Versioning
  │      └──> Phase 06: AgentSpace Engine
  │             └──> Phase 07: Agents + Review
  └──────────────────> Phase 08: MCP Server
```

## Phases

| # | Phase | Effort | Status | File |
|---|-------|--------|--------|------|
| 01 | Project Setup & Core Types | 4h | pending | [phase-01](phase-01-project-setup-core-types.md) |
| 02 | Vault & Notes | 10h | pending | [phase-02](phase-02-vault-and-notes.md) |
| 03 | CLI Interface | 8h | pending | [phase-03](phase-03-cli-interface.md) |
| 04 | Search & Index | 10h | pending | [phase-04](phase-04-search-and-index.md) |
| 05 | CAS & Versioning | 12h | pending | [phase-05](phase-05-cas-and-versioning.md) |
| 06 | AgentSpace Engine | 8h | pending | [phase-06-agentspace-engine.md](phase-07-agentspace-engine.md) |
| 07 | Agent Core + Built-in Agents + Review | 10h | pending | [phase-07](phase-08-agents-and-review.md) |
| 08 | MCP Server | 8h | pending | [phase-08](phase-09-mcp-server.md) |
| ~~06~~ | ~~P2P Sync~~ | ~~10h~~ | **deferred** | [phase-06-p2p-sync.md](phase-06-p2p-sync.md) (v2) |

## Key Research

- [Architecture Brainstorm](../reports/brainstorm-260213-1552-agentic-note-app.md)
- [Rust Crates API](research/researcher-rust-crates-api.md)
- [Embeddings & LLM & Crypto](research/researcher-embeddings-llm-crypto.md)
- [AgentSpace Patterns](../reports/researcher-260213-1604-agentspace-pipeline-patterns.md)
- [P2P Sync Research](../reports/researcher-260213-1538-obsidian-anytype-p2p-sync.md)

## Critical Decisions

- P2P sync deferred to v2 — ship local-only MVP first, iroh API too unstable
- tantivy FTS + sqlite-vec embeddings — no external services required
- TOML pipelines — user-friendly, Cargo-familiar
- ort + all-MiniLM-L6-v2 for local embeddings — first-run download (~23MB), keeps binary small
- CAS kept for local versioning/undo — essential for AgentSpace safety
- All 4 built-in agents in MVP (classifier, linker, distiller, writer)
- LLM output: JSON mode + schema validation, retry on parse failure
- Conflict resolution (CAS): pick A or B (not git-style merge markers)
- Pipeline error policy: global skip + warn (per-stage config in v2)
- API keys: config.toml with 0600 perms (simplest for CLI tool)
- Sequential pipelines only (no DAG branching) for MVP

## Validation Log

### Session 1 — 2026-02-13
**Trigger:** Initial plan validation before implementation
**Questions asked:** 8

#### Questions & Answers

1. **[Security]** The plan stores LLM API keys in config.toml (plaintext). For MVP, which key storage approach?
   - Options: config.toml + 0600 perms | Environment variables only | OS keychain
   - **Answer:** config.toml + 0600 perms
   - **Rationale:** Standard CLI tool pattern. Warn user about permissions in docs.

2. **[Risk]** iroh 0.29 has unstable APIs (breaks every minor version). How should we handle this risk?
   - Options: Thin adapter layer | Pin and accept breakage | Defer P2P sync
   - **Answer:** Defer P2P sync
   - **Rationale:** Major scope reduction. Ship local-only MVP first. P2P sync moves to v2 when iroh stabilizes. Removes Phase 06 (10h) and sync crate from MVP.

3. **[Architecture]** For conflict resolution in sync, the plan uses git-style .conflict files. Sufficient for MVP?
   - Options: Pick A or B | Git-style .conflict files | Keep both as separate notes
   - **Answer:** Pick A or B
   - **Rationale:** Simpler UX. Show both versions, user picks one. Applies to CAS snapshot conflicts too.

4. **[Architecture]** For LLM structured output parsing (agent responses), what approach?
   - Options: JSON mode + schema validation | Markdown + regex extraction | Tool use / function calling
   - **Answer:** JSON mode + schema validation
   - **Rationale:** Most reliable. Use OpenAI/Anthropic JSON mode. Validate against schema. Retry on parse failure.

5. **[Scope]** With P2P sync deferred, should CAS (Phase 05) remain in MVP scope?
   - Options: Yes, keep CAS | Simplify to file backup only | Defer CAS too
   - **Answer:** Yes, keep CAS
   - **Rationale:** Local snapshots + undo essential for AgentSpace safety. Agent changes must be reversible via CAS snapshots.

6. **[Scope]** The plan has 4 built-in agents. For MVP shipping speed, should we start with fewer?
   - Options: All 4 agents | 2 agents: classifier + writer | 1 agent: classifier only
   - **Answer:** All 4 agents
   - **Rationale:** Full pipeline demonstrates the AgentSpace value prop. All 4 needed for the default auto-process-inbox pipeline.

7. **[Architecture]** Embedding model distribution — all-MiniLM-L6 (~23MB ONNX). How should it be distributed?
   - Options: First-run download | Bundle in binary | Optional feature flag
   - **Answer:** First-run download
   - **Rationale:** Keeps binary small (~5MB). Download model on first use. Needs internet once.

8. **[Architecture]** AgentSpace pipeline error policy — configurable per stage or global default?
   - Options: Global default: skip + warn | Per-stage configurable now | Always abort on error
   - **Answer:** Global default: skip + warn
   - **Rationale:** Failed stage logs warning, pipeline continues. Per-stage config deferred to v2.

#### Confirmed Decisions
- P2P sync deferred to v2 — removes Phase 06, sync crate. 10h saved.
- CAS kept — local versioning/undo for AgentSpace safety
- All 4 agents — full pipeline for MVP
- JSON mode + schema validation for LLM output
- First-run download for embedding model
- Pick A or B for conflicts (not git-style markers)
- Global skip+warn for pipeline errors
- config.toml + 0600 for API keys

#### Action Items
- [ ] Remove Phase 06 (P2P Sync) from active MVP scope, mark as deferred
- [ ] Remove sync crate from Cargo workspace members
- [ ] Update Phase 05 merge.rs to use pick-A-or-B instead of three-way merge markers
- [ ] Update Phase 08 to specify JSON mode + schema validation for LLM output
- [ ] Add first-run model download logic to Phase 04 embedding setup
- [ ] Add global error policy to Phase 07 AgentSpace engine config

#### Impact on Phases
- Phase 05: Simplify merge — pick A or B, no git-style conflict markers
- Phase 06: DEFERRED — P2P sync moved to v2
- Phase 07: Add global `on_error: skip` default to pipeline config
- Phase 08: LLM output parsing uses JSON mode + schema validation + retry
- Phase 04: Embedding model = first-run download (not bundled)
