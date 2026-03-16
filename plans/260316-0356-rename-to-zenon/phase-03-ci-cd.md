# Phase 3: CI/CD Workflows

**Status:** DONE | **Priority:** Medium | **Effort:** Small

## Files

1. `.github/workflows/ci.yml` — no direct references (uses `cargo` generically)
2. `.github/workflows/release.yml` — likely has binary name refs
3. `.github/workflows/live-tests.yml` — may have refs

## Replacements

- `agentic-note` → `zenon` in any binary name or artifact references

## Todo

- [x] Update release.yml
- [x] Update live-tests.yml
- [x] Verify no broken references

## Completion Notes

- release.yml: updated binary references and artifact names (agentic-note → zenon)
- live-tests.yml: updated test references and configuration paths
- All CI/CD workflows aligned with new project naming
