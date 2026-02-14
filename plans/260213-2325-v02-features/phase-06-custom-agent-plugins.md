# Phase 6: Custom Agent Plugin System

## Context Links
- [Research: Plugins](/Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/research/researcher-embeddings-dag-plugins.md)
- [AgentHandler trait](/Users/phuc/Developer/agentic-note/crates/agent/src/engine/executor.rs)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P2
- **Status:** completed
- **Effort:** 3h
- **Depends on:** Phase 3 (agent engine changes)
- **Description:** Subprocess-based plugins. Plugin = executable reading JSON stdin, writing JSON stdout. Discovery from `.agentic/plugins/`. `plugin.toml` metadata. Configurable timeout (default 30s).

## Key Insights
- KISS: subprocess isolation, no WASM/FFI complexity
- Same JSON pattern as MCP tools — consistent architecture
- Plugin implements `AgentHandler` trait internally via `PluginAgent` adapter
- Plugin discovery at AgentSpace init — scan `.agentic/plugins/*/plugin.toml`
- Timeout via `tokio::time::timeout()` on subprocess

## Requirements

### Functional
- F1: `plugin.toml` schema: name, version, description, executable, timeout_secs
- F2: Plugin discovery: scan `.agentic/plugins/*/plugin.toml` at startup
- F3: `PluginAgent` struct implementing `AgentHandler` — spawns subprocess
- F4: Subprocess receives `StageContext` as JSON on stdin
- F5: Subprocess writes `StageOutput` as JSON on stdout
- F6: Configurable timeout per plugin (default 30s from PluginsConfig)
- F7: Stderr captured for error reporting
- F8: Non-zero exit code treated as agent failure
- F9: CLI command `plugin list` — show discovered plugins

### Non-Functional
- Plugin spawn overhead <50ms
- Timeout kills subprocess cleanly (SIGTERM, then SIGKILL)
- No concurrent plugin instances (one subprocess per invocation)

## Architecture

```
crates/agent/src/
├── plugin/
│   ├── mod.rs          # NEW: re-exports
│   ├── discovery.rs    # NEW: scan plugins dir, parse plugin.toml
│   ├── manifest.rs     # NEW: PluginManifest struct (from plugin.toml)
│   └── runner.rs       # NEW: PluginAgent (subprocess spawn + JSON I/O)
├── agents/mod.rs       # modify: register_plugins() alongside builtin agents
└── engine/mod.rs       # unchanged
```

### Plugin Directory Structure
```
.agentic/plugins/
├── custom-tagger/
│   ├── plugin.toml
│   └── run.sh          # or run.py, or compiled binary
└── summarizer-v2/
    ├── plugin.toml
    └── summarize
```

### Execution Flow
```
Pipeline stage references agent "custom-tagger"
  → PluginAgent found in handler registry
  → Serialize StageContext to JSON
  → Spawn subprocess: `.agentic/plugins/custom-tagger/run.sh`
  → Write JSON to stdin, close stdin
  → Read stdout (with timeout)
  → Parse JSON as Value (StageOutput)
  → Return to pipeline
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/mod.rs` | create | Module declarations + re-exports |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/manifest.rs` | create | PluginManifest struct |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/discovery.rs` | create | discover_plugins() |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/runner.rs` | create | PluginAgent + subprocess execution |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/lib.rs` | modify | +pub mod plugin |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/agents/mod.rs` | modify | +register_plugins() |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/mod.rs` | modify | +Plugin command |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/plugin.rs` | create | `plugin list` command |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/main.rs` | modify | +Plugin dispatch |

## Implementation Steps

1. Create `crates/agent/src/plugin/manifest.rs`:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct PluginManifest {
       pub name: String,
       pub version: String,
       pub description: String,
       pub executable: String,
       #[serde(default = "default_timeout")]
       pub timeout_secs: u64,  // default 30
   }
   fn default_timeout() -> u64 { 30 }
   ```
   - `PluginManifest::load(path: &Path) -> Result<Self>` — read + parse TOML

2. Create `crates/agent/src/plugin/discovery.rs`:
   - `discover_plugins(plugins_dir: &Path) -> Result<Vec<(PluginManifest, PathBuf)>>`
   - Scan `plugins_dir/*/plugin.toml`
   - Return (manifest, plugin_dir_path) pairs
   - Log warnings for invalid manifests, skip them

3. Create `crates/agent/src/plugin/runner.rs`:
   ```rust
   pub struct PluginAgent {
       manifest: PluginManifest,
       plugin_dir: PathBuf,
   }
   ```
   - Implement `AgentHandler`:
     - `agent_id()` → `manifest.name`
     - `execute()`:
       1. Serialize `StageContext` fields to JSON (note_id, note_content, frontmatter, outputs)
       2. Build executable path: `plugin_dir.join(&manifest.executable)`
       3. Check executable exists + is executable
       4. `tokio::process::Command::new(exe_path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`
       5. Write JSON to stdin, close stdin
       6. `tokio::time::timeout(Duration::from_secs(timeout), child.wait_with_output())`
       7. On timeout: kill child, return `AgenticError::Plugin("timeout")`
       8. Check exit code: non-zero → `AgenticError::Plugin(stderr)`
       9. Parse stdout as `serde_json::Value`
       10. Return parsed value

4. Create `crates/agent/src/plugin/mod.rs`:
   ```rust
   pub mod discovery;
   pub mod manifest;
   pub mod runner;
   pub use discovery::discover_plugins;
   pub use manifest::PluginManifest;
   pub use runner::PluginAgent;
   ```

5. Modify `crates/agent/src/agents/mod.rs`:
   - Add `register_plugins()`:
     ```rust
     pub fn register_plugins(space: &mut AgentSpace, plugins_dir: &Path) -> Result<usize> {
         let plugins = discover_plugins(plugins_dir)?;
         for (manifest, dir) in &plugins {
             space.register_agent(Arc::new(PluginAgent::new(manifest.clone(), dir.clone())));
         }
         Ok(plugins.len())
     }
     ```

6. Add `pub mod plugin;` to `crates/agent/src/lib.rs`.

7. Create `crates/cli/src/commands/plugin.rs`:
   - `plugin list` → discover and print plugins (name, version, description, path)

8. Update CLI `Commands` enum and main.rs dispatch.

9. Write tests:
   - Plugin discovery finds valid plugins, skips invalid
   - PluginAgent subprocess echoes JSON correctly
   - Timeout kills subprocess
   - Non-zero exit code returns error
   - Missing executable returns error

## Todo List

- [ ] Create plugin/manifest.rs
- [ ] Create plugin/discovery.rs
- [ ] Create plugin/runner.rs (subprocess + timeout)
- [ ] Create plugin/mod.rs
- [ ] Add register_plugins() to agents/mod.rs
- [ ] Add pub mod plugin to agent lib.rs
- [ ] Create CLI plugin list command
- [ ] Update Commands enum + main.rs
- [ ] Tests: discovery
- [ ] Tests: subprocess execution (echo plugin)
- [ ] Tests: timeout
- [ ] Tests: error handling
- [ ] cargo check + cargo test pass

## Success Criteria
- `.agentic/plugins/echo/plugin.toml` + `run.sh` discovered at startup
- Pipeline stage `agent = "echo"` dispatches to subprocess
- Subprocess receives JSON stdin, returns JSON stdout
- Timeout kills slow plugins cleanly
- `plugin list` shows discovered plugins

## Risk Assessment
- **Low:** Subprocess spawning is well-tested in tokio
- **Medium:** Windows compatibility — `run.sh` won't work. Mitigation: `executable` field supports any name, docs recommend cross-platform approach or `.exe`
- **Low:** JSON serialization of StageContext is straightforward (already Serialize)

## Security Considerations
- Plugins run with same user permissions as agentic-note
- No sandboxing beyond process isolation — document risk in README
- Plugin executables must be explicitly placed by user (no auto-download)
- Executable permissions checked before spawn

## Next Steps
- Phase 8 (Integration) tests plugins within DAG pipelines
