---
title: "agentic-note v0.6.0: UX Polish, Integration Testing & API Stability"
description: "TUI dashboard, real E2E tests, and crates.io readiness across 8 phases."
status: in-progress
priority: P1
effort: 40h
branch: main
tags: [tui, testing, cratesio, api-stability, ratatui, fuzz, semver]
created: 2026-02-18
---

# v0.6.0 Plan

**Three focus areas:** UX/CLI polish · Real integration testing · API stability & crates.io prep

## Phases

| # | Phase | Focus | Est. | Status |
|---|-------|-------|------|--------|
| 32 | [TUI Dashboard (ratatui)](./phase-32-tui-dashboard-ratatui.md) | UX | 6h | pending |
| 33 | [Interactive REPL Mode](./phase-33-interactive-repl-mode.md) | UX | 4h | pending |
| 34 | [Shell Completions + Error Diagnostics](./phase-34-shell-completions-error-diagnostics.md) | UX | 4h | pending |
| 35 | [Vault Templates + Note Export](./phase-35-vault-templates-note-export.md) | UX | 4h | pending |
| 36 | [Integration Tests: LLM + P2P](./phase-36-integration-tests-llm-p2p.md) | Testing | 6h | in-progress |
| 37 | [Stress, Benchmarks & Fuzz Testing](./phase-37-stress-benchmarks-fuzz-testing.md) | Testing | 6h | pending |
| 38 | [API Audit & crates.io Prep](./phase-38-api-audit-cratesio-prep.md) | Stability | 6h | pending |
| 39 | [Rustdoc Examples + Version Bump](./phase-39-rustdoc-examples-version-bump.md) | Stability | 4h | pending |

**Total effort:** ~40h

## Key Dependencies

- Phase 32–35: UX — independent, can parallelize 33+34+35 after 32 is scaffolded
- Phase 36–37: Testing — 37 builds on 36 fixtures; run 36 first
- Phase 38–39: Stability — 39 depends on 38 (API audit before doc examples)
- All phases must pass existing CI before merge

## Research

- [TUI/CLI UX Research](./research/researcher-tui-cli-ux.md)
- [Integration Testing & crates.io Research](./research/researcher-testing-cratesio.md)

## Validation Log

### Session 1 — 2026-02-18
**Trigger:** Initial plan creation validation
**Questions asked:** 6

#### Questions & Answers

1. **[Scope]** The plan has 40h across 8 phases. Should all 8 ship in v0.6.0, or defer some to v0.7.0?
   - Options: All 8 in v0.6.0 | Core 6, defer TUI + REPL | Testing + API only, defer all UX
   - **Answer:** All 8 phases in v0.6.0
   - **Rationale:** User wants full delivery. All focus areas ship together.

2. **[Tradeoff]** Phase 35 proposes shelling out to `pandoc` for PDF export instead of embedding `typst` crate. Accept this tradeoff?
   - Options: Pandoc shell-out | Skip PDF entirely | Typst crate behind feature flag
   - **Answer:** Typst crate behind feature flag
   - **Rationale:** Pure Rust PDF; no external tool dependency. `export-pdf` feature flag keeps binary lean for users who don't need it.

3. **[Architecture]** Phase 38 proposes license `MIT OR Apache-2.0` for crates.io publishing. Confirm?
   - Options: MIT OR Apache-2.0 | MIT only | Apache-2.0 only
   - **Answer:** MIT OR Apache-2.0
   - **Rationale:** Standard Rust ecosystem dual-license; maximum compatibility.

4. **[Architecture]** The plan introduces 4 feature flags: `tui`, `repl`, `export-pdf`, `live-tests`. Acceptable complexity?
   - Options: 4 flags as planned | Merge tui + repl into `interactive` | Only `live-tests`, bundle rest
   - **Answer:** 4 flags as planned
   - **Rationale:** Each flag isolates heavy optional deps; base binary stays lean.

5. **[Architecture]** Phase 36 adds `base_url` override to LLM providers for wiremock testing. Expose as user-facing config?
   - Options: Yes, expose in config.toml | Internal only (test infra)
   - **Answer:** Yes, expose in config.toml
   - **Rationale:** Users can point to custom endpoints (Azure OpenAI, local proxies, LiteLLM).

6. **[Tradeoff]** Phase 37 uses cargo-fuzz (nightly-only, libFuzzer). Adds CI complexity. Accept?
   - Options: Yes, add fuzz CI job | Local-only, no CI | Skip fuzzing
   - **Answer:** Yes, add fuzz CI job
   - **Rationale:** Separate nightly workflow; catches parser panics before users.

#### Confirmed Decisions
- All 8 phases ship in v0.6.0
- PDF export via **typst** crate (not pandoc shell-out) behind `export-pdf` feature flag
- License: MIT OR Apache-2.0
- 4 feature flags: `tui`, `repl`, `export-pdf`, `live-tests`
- LLM `base_url` exposed in config.toml as user-facing option
- Fuzz CI job added (nightly toolchain, separate workflow)

#### Action Items
- [ ] Phase 35: Replace pandoc approach with typst crate; add `typst` to workspace deps behind `export-pdf` feature
- [ ] Phase 36: Add `base_url` to LLM provider config sections in `config.toml` schema (not just env var)
- [ ] Phase 38: Confirm license is `MIT OR Apache-2.0` in all Cargo.toml files

#### Impact on Phases
- Phase 35: Switch PDF export from pandoc shell-out to `typst` crate behind `export-pdf` feature. Add `typst` to workspace deps. Update export.rs to use typst rendering instead of `std::process::Command("pandoc")`.
- Phase 36: In addition to `base_url` env var override, add `base_url` field to `[llm.providers.openai]` and `[llm.providers.anthropic]` sections in config.toml schema + AppConfig struct. Update openai.rs/anthropic.rs to read from config first, env var fallback second.
