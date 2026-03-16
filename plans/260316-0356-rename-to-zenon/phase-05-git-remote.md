# Phase 5: Git Remote & Repo Rename

**Status:** PENDING (MANUAL) | **Priority:** Low | **Effort:** Small

## Steps

1. **GitHub repo rename** (manual): Settings → General → Repository name → `zenon`
2. **Update local remote**: `git remote set-url origin https://github.com/HuynhHoangPhuc/zenon.git`
3. **Directory rename** (optional): `mv ~/Projects/agentic-note ~/Projects/zenon`

## Notes

- GitHub auto-redirects old URL, but update remote anyway
- After directory rename, update any IDE/editor workspace configs
- Update CLAUDE.md memory paths if applicable

## Todo

- [ ] Rename GitHub repo (manual) — requires user to do via GitHub Settings
- [ ] Update git remote URL (post-rename)
- [ ] Rename local directory (optional)

## Notes

This phase requires manual user action via GitHub web interface. Automated phases 1-4 (Cargo, Rust, CI/CD, docs) are complete. User must:

1. Go to https://github.com/HuynhHoangPhuc/agentic-note/settings
2. Change repository name from "agentic-note" to "zenon"
3. Run: `git remote set-url origin https://github.com/HuynhHoangPhuc/zenon.git`
4. Optionally rename local directory: `mv ~/Projects/agentic-note ~/Projects/zenon`
