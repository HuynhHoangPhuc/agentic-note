---
title: "v0.4.0 Stability & Production Hardening"
description: "Six production-hardening features: PostgreSQL backend, batch LLM, Prometheus, E2EE, multi-vault, WASM sandboxing"
status: complete
priority: P1
effort: 15h
branch: main
tags: [v0.4.0, production, stability, postgres, prometheus, encryption, wasm]
created: 2026-02-14
---

# v0.4.0 Stability & Production Hardening

## Summary

Six features to harden agentic-note for production deployments: PostgreSQL optional backend, batch LLM requests, Prometheus metrics, end-to-end encryption, multi-vault sync, and WASM plugin sandboxing.

## Phases

| # | Phase | Effort | Status | File |
|---|-------|--------|--------|------|
| 20 | PostgreSQL optional backend | 3h | complete | [phase-20](phase-20-postgresql-optional-backend.md) |
| 21 | Batch LLM requests | 2.5h | complete | [phase-21](phase-21-batch-llm-requests.md) |
| 22 | Prometheus integration | 2h | complete | [phase-22](phase-22-prometheus-integration.md) |
| 23 | End-to-end encryption | 3h | complete | [phase-23](phase-23-end-to-end-encryption.md) |
| 24 | Multi-vault sync | 2h | complete | [phase-24](phase-24-multi-vault-sync.md) |
| 25 | Plugin sandboxing (WASM) | 2.5h | complete | [phase-25](phase-25-plugin-sandboxing-wasm.md) |

## Dependencies

- Phase 20 (PostgreSQL) is independent; can be done first or in parallel
- Phase 21 (Batch LLM) is independent
- Phase 22 (Prometheus) depends on existing metrics stubs in `metrics_init.rs`
- Phase 23 (E2EE) depends on existing Ed25519 identity in sync crate
- Phase 24 (Multi-vault) depends on existing SyncEngine
- Phase 25 (WASM) depends on existing plugin runner

## Key Decisions

- **sqlx** for PostgreSQL (async, compile-time checks, built-in pool + migrations)
- **prometheus-client** crate for metrics (official Rust client, simple API)
- **chacha20poly1305** + **x25519-dalek** for E2EE (RustCrypto, audited, pure Rust)
- **wasmtime** for WASM sandboxing (Bytecode Alliance, WASI, fuel metering)
- Feature flags: `postgres`, `prometheus` (E2EE and WASM always compiled)

## New Workspace Dependencies

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "postgres", "any"] }
prometheus-client = "0.23"
chacha20poly1305 = "0.10"
x25519-dalek = { version = "2", features = ["static_secrets"] }
wasmtime = "28"
hyper = { version = "1", features = ["server", "http1"] }
http-body-util = "0.1"
hyper-util = { version = "0.1", features = ["tokio"] }
```

## New Error Variants

`Database(String)`, `Encryption(String)`, `Wasm(String)`, `MultiVault(String)`

## Version Bump

All 8 crates: 0.3.0 -> 0.4.0

## Validation Log

### Session 1 — 2026-02-14
**Trigger:** Initial plan creation validation
**Questions asked:** 6

#### Questions & Answers

1. **[Scope]** The plan includes 6 features at 15h total. Should all 6 ship in v0.4.0, or defer some to v0.5.0?
   - Options: All 6 in v0.4.0 | Core 4, defer 2 | P1 only (3 features)
   - **Answer:** All 6 in v0.4.0
   - **Rationale:** User wants full delivery. All phases independent, no blocking dependencies.

2. **[Architecture]** Phase 20 proposes a custom StorageBackend trait (SQLite + Postgres). How aggressive should the refactor be?
   - Options: Full trait abstraction | Postgres-only addition | Use sqlx for both
   - **Answer:** Full trait abstraction
   - **Rationale:** Clean architecture. Refactor Graph + ReviewQueue to Arc<dyn StorageBackend>. Bigger diff but proper abstraction.

3. **[Tradeoff]** Phase 25 adds wasmtime for WASM plugin sandboxing (+5-10MB binary). Worth the complexity?
   - Options: Yes, add WASM | Use Extism instead | Defer to v0.5
   - **Answer:** Yes, add WASM (raw wasmtime)
   - **Rationale:** Security-critical for untrusted plugins. wasmtime is production-ready. No Extism dependency.

4. **[Security]** Phase 23 E2EE uses static X25519 DH (no forward secrecy). Accept limitation?
   - Options: Accept, document | Add ephemeral keys now | Skip E2EE entirely
   - **Answer:** Accept, document limitation
   - **Rationale:** iroh QUIC TLS 1.3 provides transport encryption. E2EE is defense-in-depth. Forward secrecy planned for v0.5.

5. **[Metrics]** Phase 22 switches from `metrics` crate to `prometheus-client`. Confirm?
   - Options: Switch to prometheus-client | Keep metrics + add exporter | Keep stubs
   - **Answer:** Switch to prometheus-client
   - **Rationale:** Official Prometheus client, direct registry access, cleaner /metrics endpoint. Remove `metrics` crate.

6. **[LLM Batch]** Batch LLM approach: concurrent real-time or also async Batch API?
   - Options: Concurrent real-time only | Both real-time + async | Cache only
   - **Answer:** Concurrent real-time only
   - **Rationale:** Simpler. Batch parallel stages via concurrent HTTP. No polling complexity. Async Batch API deferred.

#### Confirmed Decisions
- All 6 features ship in v0.4.0
- Full StorageBackend trait abstraction with Arc<dyn StorageBackend>
- Raw wasmtime (no Extism) for WASM sandboxing
- Static X25519 DH accepted; forward secrecy deferred to v0.5
- Switch from `metrics` to `prometheus-client` crate
- Concurrent real-time LLM batching only; async Batch API deferred

#### Action Items
- [ ] Remove `metrics = "0.24"` from workspace Cargo.toml when Phase 22 implemented
- [ ] Document "no forward secrecy" in Phase 23 security section (already noted)

#### Impact on Phases
- Phase 22: Remove `metrics` crate dependency, replace with `prometheus-client` throughout
- Phase 23: Add explicit "Known Limitation: No forward secrecy" section
- No other phase changes needed — all decisions align with existing plan
