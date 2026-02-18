---
title: "v0.5.0 Quality & Polish"
description: "Six quality phases: forward secrecy, async batch LLM, test coverage, CI/CD, rustdoc, bug fixes"
status: completed
priority: P1
effort: 14.5h
branch: main
tags: [v0.5.0, quality, polish, testing, cicd, docs, security]
created: 2026-02-18
---

# v0.5.0 Quality & Polish

## Summary

Six phases to prepare agentic-note for 1.0: upgrade E2EE to Double Ratchet forward secrecy, add async OpenAI Batch API, achieve 90%+ test coverage, establish CI/CD pipeline, complete rustdoc, and fix edge cases.

## Phases

| # | Phase | Effort | Status | File |
|---|-------|--------|--------|------|
| 26 | Forward Secrecy (Double Ratchet) | 3h | completed | [phase-26](phase-26-forward-secrecy-double-ratchet.md) |
| 27 | Async Batch LLM API | 2.5h | completed | [phase-27](phase-27-async-batch-llm-api.md) |
| 28 | Comprehensive Test Coverage | 4h | completed | [phase-28](phase-28-comprehensive-test-coverage.md) |
| 29 | CI/CD GitHub Actions | 2h | completed | [phase-29](phase-29-cicd-github-actions.md) |
| 30 | Rustdoc & API Documentation | 1.5h | completed | [phase-30](phase-30-rustdoc-api-documentation.md) |
| 31 | Bug Fixes & Version Bump | 1.5h | completed | [phase-31](phase-31-bug-fixes-edge-cases-version-bump.md) |

## Dependencies

- Phase 26 (Forward Secrecy) independent; builds on `crates/sync/src/encryption.rs`
- Phase 27 (Batch LLM) independent; extends `LlmProvider` trait in `crates/agent/src/llm/`
- Phase 28 (Tests) should run after 26+27 to cover new code
- Phase 29 (CI/CD) independent; can run in parallel with 26/27
- Phase 30 (Docs) should run after 26+27+28 (document final APIs)
- Phase 31 (Bug Fixes) runs last; version bump all crates to 0.5.0

## New Workspace Dependencies

```toml
ksi-double-ratchet = "0.1"     # Phase 26: Signal-spec Double Ratchet
async-openai = "0.27"          # Phase 27: OpenAI Batch API client
tokio-retry = "0.3"            # Phase 27: Exponential backoff polling
proptest = "1"                 # Phase 28: Property-based testing (dev-dep)
cargo-llvm-cov = "install"     # Phase 28: Coverage tool (cargo install)
```

## New Error Variants

`Batch(String)` for async batch API errors.

## Key Decisions

- **ksi-double-ratchet** over vodozemac: lighter, Signal-spec, no Matrix baggage
- **async-openai** over raw reqwest: built-in Batch+Files API, maintained
- **cargo-llvm-cov** over tarpaulin: cross-platform, LLVM-based, more accurate
- **proptest** over quickcheck: better shrinking, macro integration
- Version byte envelope (0x01=legacy, 0x02=DR) for backward compat
- Optional batch methods with `NotSupported` default fallback
- CI matrix reduced to 3x1 (validated)
- ADRs deferred to v0.6 (validated)
- Manual version bump, no cargo-release (validated)

## Validation Log

### Session 1 — 2026-02-18
**Trigger:** Initial plan creation validation
**Questions asked:** 6

#### Questions & Answers

1. **[Architecture]** Phase 26 adds full Double Ratchet (X3DH + ratcheting + session state in SQLite). This is significant crypto complexity. Is full DR needed, or would simpler ephemeral-per-session DH suffice for P2P sync?
   - Options: Full Double Ratchet (Recommended) | Ephemeral DH per-session | Defer crypto upgrade to v0.6
   - **Answer:** Full Double Ratchet
   - **Rationale:** Per-message forward secrecy and break-in recovery are essential for production-grade P2P sync. Ephemeral-per-session would leave messages vulnerable within a session.

2. **[Dependencies]** Plan adds 4 new dependencies (ksi-double-ratchet, async-openai, tokio-retry, proptest). This contradicts the 'no new deps unless necessary' constraint. Which are acceptable?
   - Options: All 4 acceptable | Skip async-openai, use raw reqwest | Only proptest (dev-dep)
   - **Answer:** All 4 acceptable
   - **Rationale:** Each dep serves a distinct purpose. proptest is dev-only. async-openai and tokio-retry are behind feature flag. ksi-double-ratchet is security-critical.

3. **[Architecture]** Phase 28 creates 2 new crates (test-utils + integration-tests), expanding workspace from 8 to 10 crates. Alternative: put integration tests in existing crate test dirs. Prefer which approach?
   - Options: 2 new crates (Recommended) | Tests in existing crates only | Just test-utils crate
   - **Answer:** 2 new crates
   - **Rationale:** Clean separation avoids circular deps. test-utils reusable across all crates. integration-tests can depend on everything without polluting production builds.

4. **[Scope]** CI test matrix is 5 feature combos x 2 OS = 10 parallel jobs per push. For a pre-1.0 single-dev project, is this overkill?
   - Options: Keep full matrix | Reduce to 3x1 (Recommended) | Minimal: single job
   - **Answer:** Reduce to 3x1
   - **Rationale:** 3 feature combos (default, all-features, no-default) on ubuntu-only is sufficient. macOS testing only for release builds. Saves CI minutes.

5. **[Scope]** Phase 30 proposes 5 ADRs (Architecture Decision Records) in docs/adr/. These take effort to write well. Worth including in v0.5.0 or defer?
   - Options: Include 5 ADRs | Skip ADRs, focus on rustdoc only (Recommended) | Write 2 key ADRs only
   - **Answer:** Skip ADRs, focus on rustdoc only
   - **Rationale:** Code-level rustdoc is higher impact than ADR documents. ADRs deferred to v0.6 or when community grows.

6. **[Tradeoff]** The plan bumps to 10 crates (8 existing + test-utils + integration-tests). Should workspace version be managed with cargo-release or manual sed?
   - Options: Manual version bump (Recommended) | Add cargo-release
   - **Answer:** Manual version bump
   - **Rationale:** Simple and matches current workflow. cargo-release adds tooling overhead not justified for 10 crates pre-1.0.

#### Confirmed Decisions
- Full Double Ratchet protocol for forward secrecy (Phase 26 unchanged)
- All 4 new dependencies approved
- 2 new test crates (test-utils + integration-tests)
- CI matrix reduced from 5x2 to 3x1 (ubuntu only)
- ADRs deferred — Phase 30 focuses on rustdoc only
- Manual version bump via sed/edit

#### Action Items
- [ ] Update Phase 29 CI matrix from 5x2 to 3x1
- [ ] Update Phase 30 to remove ADR section, reduce effort estimate

#### Impact on Phases
- Phase 29: CI test matrix reduced from 5 feature combos x 2 OS → 3 feature combos x 1 OS. Effort unchanged (2h) but simpler config.
- Phase 30: Remove ADR creation (5 files). Reduce effort from 2h → 1.5h. Focus purely on rustdoc completion.
