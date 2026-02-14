# Code Review: agentic-note Full Workspace

**Reviewer:** code-reviewer
**Date:** 2026-02-13
**Scope:** All 51 Rust source files across 7 crates

---

## Code Review Summary

### Scope
- **Files:** 51 `.rs` files across `core`, `vault`, `cas`, `search`, `agent`, `review`, `cli`
- **Estimated LOC:** ~3,400
- **Focus:** Full workspace -- security, error handling, API design, file sizes, dead code, bugs

### Overall Assessment

Solid, well-structured Rust workspace with clean separation of concerns. Most files are well under 200 lines. Code is readable with good documentation comments. The architecture (7 crates, PARA model, CAS versioning, agent pipelines, review queue, MCP server) is coherent and follows KISS/YAGNI principles. 29 passing tests cover core functionality.

However, there are several security issues (2 critical, 3 high) and a handful of correctness/design issues that should be addressed.

---

### Critical Issues

#### C1. Path Traversal in MCP `vault/init` Tool
**File:** `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs` lines 104-115
**Impact:** An MCP client can pass an arbitrary `path` argument (e.g., `"/"`, `"/etc"`, `"../../sensitive-dir"`) to `vault/init`, which will create directories and write a config file at that location. Since the MCP server runs with the user's privileges, this is a path traversal vulnerability.

```rust
fn tool_vault_init(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    let path = args["path"].as_str()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| vault_path.to_path_buf());
    init_vault(&path).map_err(|e| anyhow::anyhow!("init vault: {e}"))?;
    // ...
}
```

**Fix:** Validate that the resolved path is either the configured vault_path or a subdirectory of it, or remove the `path` argument entirely from the MCP tool (the CLI already has its own `init` subcommand):

```rust
fn tool_vault_init(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    let path = match args["path"].as_str() {
        Some(p) => {
            let candidate = std::path::PathBuf::from(p).canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from(p));
            let vault_canon = vault_path.canonicalize()
                .unwrap_or_else(|_| vault_path.to_path_buf());
            if !candidate.starts_with(&vault_canon) {
                anyhow::bail!("path must be within the vault directory");
            }
            candidate
        }
        None => vault_path.to_path_buf(),
    };
    // ...
}
```

#### C2. Path Traversal in MCP `note/read` Tool
**File:** `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs` lines 43-58
**Also:** `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs` lines 148-167
**Impact:** The `resolve_note_path` function accepts arbitrary file paths. If `target` is `/etc/passwd` and that file exists, it returns it directly. An MCP client could read any file on disk the process can access.

```rust
fn resolve_note_path(vault: &Path, target: &str) -> anyhow::Result<std::path::PathBuf> {
    let as_path = std::path::PathBuf::from(target);
    if as_path.exists() {
        return Ok(as_path);  // <-- NO validation that path is within vault!
    }
    // ...
}
```

**Fix:** After resolving, verify the canonical path is within the vault:

```rust
fn resolve_note_path(vault: &Path, target: &str) -> anyhow::Result<std::path::PathBuf> {
    let as_path = std::path::PathBuf::from(target);
    if as_path.exists() {
        let canonical = as_path.canonicalize()?;
        let vault_canonical = vault.canonicalize()?;
        if !canonical.starts_with(&vault_canonical) {
            anyhow::bail!("path is outside the vault");
        }
        return Ok(canonical);
    }
    // ...
}
```

**Note:** The same `resolve_note_path` pattern exists in the CLI (`/Users/phuc/Developer/agentic-note/crates/cli/src/commands/note.rs` lines 151-176). The CLI version is lower risk since the user controls the input, but the MCP version is exposed to programmatic callers.

---

### High Priority

#### H1. API Key Stored in Plaintext in Config
**File:** `/Users/phuc/Developer/agentic-note/crates/core/src/config.rs` line 49
**Impact:** `ProviderConfig.api_key` is a plain `String`. The `config show` command masks it in JSON mode but prints the raw TOML file in human mode, leaking the key.

**File:** `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/config.rs` lines 29-33

```rust
OutputFormat::Human => {
    let config_path = vault_path.join(".agentic").join("config.toml");
    let content = std::fs::read_to_string(&config_path)?;
    println!("{content}");  // <-- prints raw API keys
}
```

**Fix:** Apply the same masking logic in human-readable mode. Consider supporting env var references (e.g., `api_key = "$OPENAI_API_KEY"`) instead of literal keys.

#### H2. `truncate()` Panics on Multi-byte UTF-8
**File:** `/Users/phuc/Developer/agentic-note/crates/cli/src/output.rs` lines 82-88

```rust
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])  // PANIC if max-3 is mid-codepoint
    }
}
```

**Impact:** Note titles with CJK, emoji, or accented characters will panic at `&s[..max-3]` if the byte offset lands inside a multi-byte character.

**Fix:**
```rust
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}
```

#### H3. `BlobStore::object_path` Panics on Short Object IDs
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/blob.rs` line 19

```rust
pub fn object_path(&self, id: &ObjectId) -> PathBuf {
    self.objects_dir.join(&id[..2]).join(&id[2..])  // PANIC if id.len() < 2
}
```

`ObjectId` is just `type ObjectId = String`. If a corrupted or truncated ID is passed, this panics. While SHA-256 hex strings are always 64 chars, `load()` accepts any `&ObjectId`, and external callers (like `Snapshot::load` deserializing JSON) could produce short strings.

**Fix:** Add a validation guard:
```rust
pub fn object_path(&self, id: &ObjectId) -> Result<PathBuf> {
    if id.len() < 3 {
        return Err(AgenticError::Parse(format!("invalid object ID: too short: {id}")));
    }
    Ok(self.objects_dir.join(&id[..2]).join(&id[2..]))
}
```

#### H4. No Request Timeout on LLM HTTP Calls
**Files:**
- `/Users/phuc/Developer/agentic-note/crates/agent/src/llm/openai.rs` line 23
- `/Users/phuc/Developer/agentic-note/crates/agent/src/llm/anthropic.rs` line 19

Both providers create `reqwest::Client::new()` with no timeout configured. A slow or unresponsive API will hang the agent pipeline indefinitely.

**Fix:**
```rust
client: reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(60))
    .connect_timeout(std::time::Duration::from_secs(10))
    .build()
    .expect("failed to build HTTP client"),
```

---

### Medium Priority

#### M1. Duplicated Code: `parse_para`, `parse_status`, `resolve_note_path`
**Files:**
- `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/note.rs` lines 129-176
- `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs` lines 127-167

These three functions are copy-pasted between the CLI commands and MCP handlers. This violates DRY and means security fixes (like the path traversal fix above) must be applied in two places.

**Fix:** Extract into a shared module (e.g., `cli/src/resolve.rs`) and reuse from both `commands/note.rs` and `mcp/handlers.rs`.

#### M2. `ParaCategory` Parsing Should Live on the Type
**Files:** `core/src/types.rs`, CLI note commands, MCP handlers

`ParaCategory` has `Display` but no `FromStr`. Three separate `parse_para` functions exist. The canonical parsing should be an `impl FromStr for ParaCategory` in `core/src/types.rs`.

#### M3. `Graph::notes_by_tag` Uses a Custom `PipeOk` Trait
**File:** `/Users/phuc/Developer/agentic-note/crates/search/src/graph.rs` lines 95-106, 152-160

```rust
.collect::<Vec<_>>()
.pipe_ok()
```

This custom trait wrapping `Vec` in `Ok()` adds unnecessary indirection. The other graph methods (`incoming_links`, `outgoing_links`, `orphans`) just use `Ok(rows...collect())` directly.

**Fix:** Replace `.collect::<Vec<_>>().pipe_ok()` with `Ok(rows...collect())` for consistency, and remove the `PipeOk` trait entirely.

#### M4. `ProviderConfig` Debug Trait Leaks API Keys
**File:** `/Users/phuc/Developer/agentic-note/crates/core/src/config.rs` line 47

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: String,
```

If `ProviderConfig` is ever printed via `{:?}` (including in error messages or tracing), the API key is exposed in logs.

**Fix:** Implement a custom `Debug` that masks the key:
```rust
impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("api_key", &"****")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}
```

#### M5. `Vault::list_notes` Reads Entire File for Each Note
**File:** `/Users/phuc/Developer/agentic-note/crates/vault/src/lib.rs` line 77

The comment says "Read only frontmatter (first ~20 lines) for performance" but the code does `std::fs::read_to_string(path)` which reads the entire file. For large notes, this wastes memory during listing.

**Fix:** Read a limited buffer (e.g., first 4KB) which is sufficient for frontmatter, or use `BufReader` and read line-by-line until the closing `---` delimiter.

#### M6. `Snapshot::create` Uses Deprecated `timestamp_nanos_opt`
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/snapshot.rs` line 26

```rust
let raw = format!("{}{}{}", root_tree, timestamp.timestamp_nanos_opt().unwrap_or(0), cas.device_id);
```

`timestamp_nanos_opt()` returns `None` for dates far in the future. While `unwrap_or(0)` handles this, using `timestamp_millis()` or `timestamp_micros()` would be more robust and less surprising.

#### M7. `Cas::load_or_create_device_id` Low Entropy
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/cas.rs` lines 34-48

Device ID is generated from `HOSTNAME + current_nanos`, then SHA-256 truncated to 16 hex chars. The `HOSTNAME` env var is often unset (falls back to "unknown"), and nanosecond timestamps are predictable. While this is an internal device identifier (not a security token), consider using `ulid::Ulid::new()` for simplicity and guaranteed uniqueness.

#### M8. `reindex_vault` Skips `.agentic` with String Contains Check
**File:** `/Users/phuc/Developer/agentic-note/crates/search/src/reindex.rs` line 30

```rust
if path.to_str().map(|s| s.contains(".agentic")).unwrap_or(false) {
    continue;
}
```

This is fragile -- a note titled "my-.agentic-note.md" in any folder would be skipped. Should check the path component specifically:

```rust
if path.components().any(|c| c.as_os_str() == ".agentic") {
    continue;
}
```

---

### Low Priority

#### L1. `init_vault` Uses `std::fs::create_dir_all` Without Race Condition Handling
**File:** `/Users/phuc/Developer/agentic-note/crates/vault/src/init.rs`
The `if !dir.exists()` -> `create_dir_all` pattern has a TOCTOU race. In practice, `create_dir_all` is already idempotent, so the existence check is unnecessary overhead. Remove the `if !dir.exists()` guards and just call `create_dir_all` directly.

#### L2. Missing `#[derive(Serialize)]` on `DiffEntry` and `DiffStatus`
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/diff.rs`
These types would benefit from `Serialize` for JSON output in future CLI commands.

#### L3. `NoteId` Should Be `Ord` to Enable Sorting
**File:** `/Users/phuc/Developer/agentic-note/crates/core/src/types.rs` line 10-12
`NoteId` derives `Eq` and `Hash` but not `Ord`/`PartialOrd`. Since ULID is inherently orderable, adding `Ord` would enable sorted collections.

#### L4. `OpenAiProvider::name()` Always Returns "openai" Even for Custom Endpoints
**File:** `/Users/phuc/Developer/agentic-note/crates/agent/src/llm/openai.rs` line 43
When using `new_custom()` for Ollama, the provider still identifies as "openai". Consider making the name configurable or returning a more accurate identifier.

---

### Edge Cases Found by Scouting

1. **Frontmatter parsing edge case:** If the body itself contains `\n---` (a horizontal rule in markdown), `frontmatter::parse` will incorrectly split at the first occurrence. The parser finds the *first* `\n---` after the opening delimiter, which may be a body HR rather than the closing delimiter.

2. **`Note::filename` with empty title:** `slug::slugify("")` returns `""`, producing a filename like `01JMABCD-.md` (trailing hyphen). Not harmful but aesthetically poor.

3. **CAS tree builder follows symlinks:** `Tree::from_dir` uses `std::fs::read_dir` which follows symlinks. A symlink pointing outside the vault would cause content from arbitrary paths to be stored in the CAS.

4. **MCP server has no request size limit:** The `read_line` loop in `McpServer::serve_stdio` will read arbitrarily large lines into memory. A malicious client could send a multi-gigabyte line to cause OOM.

5. **`list_notes` uses `max_depth(2)` which misses deeply nested notes:** If users create subdirectories within PARA folders (e.g., `projects/rust/my-note.md`), those notes won't be listed.

---

### Positive Observations

1. **Clean crate separation.** Each crate has a single, clear responsibility. Dependencies flow one direction.
2. **Config permissions.** Setting `0o600` on `config.toml` (line 49, `init.rs`) is a good security practice for files that may contain API keys.
3. **Idempotent `init_vault`.** Skips existing dirs/files gracefully.
4. **CAS design.** Content-addressed storage with SHA-256, two-char prefix sharding, and backup-before-restore is well implemented.
5. **Review gate pattern.** Trust levels with the gate function cleanly separating policy from mechanism.
6. **Tracing to stderr.** MCP server correctly sends logs to stderr, keeping stdout clean for JSON-RPC.
7. **Graceful stage failures.** Pipeline executor skips failing stages instead of aborting the entire pipeline.
8. **Good test coverage** for the review queue, CAS blob store, tree builder, and pipeline executor.
9. **Config masking in JSON.** The `config show` command masks API keys in JSON output.

---

### Recommended Actions (Prioritized)

1. **[CRITICAL] Fix path traversal** in MCP `resolve_note_path` and `tool_vault_init` -- validate all paths are within vault
2. **[CRITICAL] Fix path traversal** in MCP `note/read` -- same pattern, both MCP handlers
3. **[HIGH] Add HTTP timeouts** to OpenAI and Anthropic providers
4. **[HIGH] Fix `truncate()` UTF-8 panic** -- use char-based truncation
5. **[HIGH] Fix `object_path` panic** on short IDs -- add length guard
6. **[HIGH] Mask API keys** in human-mode `config show`
7. **[MEDIUM] Extract shared `parse_para`/`parse_status`/`resolve_note_path`** to eliminate duplication
8. **[MEDIUM] Add `FromStr` impl** for `ParaCategory` and `NoteStatus` on the core types
9. **[MEDIUM] Fix `.agentic` path check** in reindex to use component-based matching
10. **[MEDIUM] Implement custom `Debug`** for `ProviderConfig` to mask API keys in logs
11. **[LOW] Remove `PipeOk` trait**, add `Ord` to `NoteId`, add `Serialize` to diff types

---

### Metrics

| Metric | Value |
|--------|-------|
| Total files | 51 |
| Files over 200 lines | 2 (`executor.rs` ~246, `cli/commands/note.rs` ~177 -- under with tests excluded) |
| Test count | 29 |
| Crate count | 7 |
| Critical issues | 2 (path traversal) |
| High issues | 4 |
| Medium issues | 8 |
| Low issues | 4 |

### File Size Check

All files are well under 200 lines of production code. The two largest files (`executor.rs` at ~246 lines and `mcp/handlers.rs` at ~167 lines) include substantial test code. No action needed.

---

### Unresolved Questions

1. Should the MCP server support authentication? Currently any process that can connect via stdio has full vault access.
2. Should `Note::delete` also remove the note from the search index and CAS? Currently those are independent operations.
3. Is `max_depth(2)` in `list_notes` and `reindex_vault` intentional? It prevents subfolder organization within PARA folders.
4. Should the `VaultWriter` agent actually apply changes, or is the "propose only" pattern intentional? (Appears intentional based on the review gate design.)
