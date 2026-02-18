# Phase 29: CI/CD Pipeline — GitHub Actions

## Context Links

- [Research: GitHub Actions CI/CD](research/researcher-forward-secrecy-cicd.md)
- Code standards: `docs/code-standards.md` (pre-commit checks section)
- Current state: no CI/CD, manual `cargo test && cargo clippy && cargo fmt`

## Overview

- **Priority:** P1
- **Status:** completed
- **Effort:** 2h
- **Description:** Set up GitHub Actions CI (fmt, clippy, test, audit) and release (tag-triggered cross-compile for 5 targets). Use Swatinem/rust-cache for fast builds, feature flag matrix for thorough testing.

## Key Insights

- Swatinem/rust-cache@v2: caches ~/.cargo + target/, keyed by feature-set matrix
- Feature flag matrix: default, no-default, embeddings, postgres, prometheus, all-features
- OS matrix: ubuntu-latest + macos-latest (covers Linux + macOS; Windows via cross-compile)
- actions-rust-cross@v0 for cross-compile; 5 targets cover all major platforms
- cargo-audit via rustsec/audit-check action (no manual install needed)
- Separate ci.yml (push/PR) and release.yml (tag v*)

## Requirements

**Functional:**
- `ci.yml`: runs on push to main + all PRs
  - Job 1: fmt check (`cargo fmt --all --check`)
  - Job 2: clippy (`cargo clippy --workspace --all-features -- -D warnings`)
  - Job 3: test matrix (6 feature combinations x 2 OS)
  - Job 4: audit (`cargo audit`)
- `release.yml`: runs on tag `v*`
  - Cross-compile for 5 targets
  - Package binaries: .tar.gz (Linux/macOS), .zip (Windows)
  - Create GitHub Release with artifacts

**Non-functional:**
- CI completes in < 10 min (with cache hits)
- Cache hit rate > 80% on subsequent runs
- Release artifacts named: `agentic-note-{version}-{target}.{ext}`

## Architecture

```
.github/workflows/
  ci.yml:
    Jobs (parallel):
    ├── check-fmt     → cargo fmt --all --check
    ├── clippy        → cargo clippy --workspace --all-features -D warnings
    ├── test-matrix   → strategy.matrix: feature-set x os
    │   ├── "" (default)
    │   ├── "--features embeddings"
    │   ├── "--features postgres"
    │   ├── "--features prometheus"
    │   └── "--all-features"
    └── audit         → cargo audit

  release.yml:
    Trigger: push tags v*
    Jobs:
    ├── build-matrix  → 5 targets via actions-rust-cross
    │   ├── x86_64-unknown-linux-gnu
    │   ├── aarch64-unknown-linux-gnu
    │   ├── x86_64-apple-darwin
    │   ├── aarch64-apple-darwin
    │   └── x86_64-pc-windows-msvc
    └── release       → softprops/action-gh-release with artifacts
```

## Related Code Files

**Create:**
- `.github/workflows/ci.yml` (~80 LOC)
- `.github/workflows/release.yml` (~70 LOC)
- `scripts/package-release.sh` (~30 LOC) — tar.gz/zip packaging helper

**No Modify. No Delete.**

## Implementation Steps

1. Create `.github/workflows/` directory
2. Write `ci.yml`:
   ```yaml
   name: CI
   on: [push, pull_request]
   env:
     CARGO_TERM_COLOR: always
   jobs:
     fmt:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - run: cargo fmt --all --check
     clippy:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - run: cargo clippy --workspace --all-features -- -D warnings
     test:
       # Validated: reduced from 5x2 to 3x1 (Session 1, Q4)
       strategy:
         matrix:
           feature-set: ["", "--no-default-features", "--all-features"]
           os: [ubuntu-latest]
       runs-on: ${{ matrix.os }}
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
           with: { shared-key: "test-${{ matrix.feature-set }}" }
         - run: cargo test --workspace ${{ matrix.feature-set }}
     audit:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - uses: rustsec/audit-check@v2
           with: { token: "${{ secrets.GITHUB_TOKEN }}" }
   ```
3. Write `release.yml`:
   ```yaml
   name: Release
   on:
     push:
       tags: ['v*']
   jobs:
     build:
       strategy:
         matrix:
           include:
             - target: x86_64-unknown-linux-gnu
               os: ubuntu-latest
             - target: aarch64-unknown-linux-gnu
               os: ubuntu-latest
             - target: x86_64-apple-darwin
               os: macos-latest
             - target: aarch64-apple-darwin
               os: macos-latest
             - target: x86_64-pc-windows-msvc
               os: windows-latest
       runs-on: ${{ matrix.os }}
       steps:
         - uses: actions/checkout@v4
         - uses: houseabsolute/actions-rust-cross@v0
           with:
             command: build
             target: ${{ matrix.target }}
             args: "--release --bin agentic-note"
         - name: Package
           run: # tar.gz or zip depending on OS
         - uses: actions/upload-artifact@v4
     release:
       needs: build
       runs-on: ubuntu-latest
       steps:
         - uses: actions/download-artifact@v4
         - uses: softprops/action-gh-release@v2
           with: { files: "**/*" }
   ```
4. Write `scripts/package-release.sh`: detect OS, create tar.gz or zip, name as `agentic-note-{tag}-{target}`
5. Test locally: `act -j fmt` (if act installed) or push to branch and verify
6. Verify cache keys work correctly across matrix dimensions

## Todo List

- [x] Create .github/workflows/ directory
- [x] Write ci.yml with fmt, clippy, test matrix, audit jobs
- [x] Write release.yml with cross-compile matrix + GitHub Release
- [x] Write scripts/package-release.sh
- [ ] Test CI on a branch push (pending remote run)
- [ ] Verify Swatinem/rust-cache produces cache hits (pending remote run)
- [ ] Verify release workflow packages correct artifacts (pending tag run)

## Success Criteria

- CI runs on every push/PR to main
- All 4 CI jobs pass (fmt, clippy, test matrix, audit)
- Test matrix covers 3 feature combinations x 1 OS = 3 jobs <!-- Updated: Validation Session 1 - reduced matrix -->
- Cache reduces build time by 50%+ on second run
- Release workflow triggers on `v*` tag, produces 5 platform binaries
- Artifacts downloadable from GitHub Release page

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Matrix explosion (too many jobs) | Medium | Low | 5 features x 2 OS = 10 jobs is acceptable |
| macOS runner availability | Low | Medium | macos-latest is well-supported |
| Cross-compile failures for aarch64 | Medium | Medium | actions-rust-cross handles Docker-based cross |
| cargo-audit false positives | Low | Low | Review advisories, ignore known |

## Security Considerations

- No secrets needed for CI (no API keys in tests)
- Release uses `GITHUB_TOKEN` (auto-provided) for artifact upload
- Audit job checks for known vulnerabilities in dependencies
- No cache poisoning risk with Swatinem/rust-cache (scoped to repo)

## Next Steps

- Phase 28 coverage results can be uploaded as CI artifact
- Phase 31 version bump triggers release workflow
- Future: add coverage badge to README, notify on audit failures
