# Phase 25: Plugin Sandboxing (WebAssembly)

## Context Links
- [plan.md](plan.md)
- [crates/agent/src/plugin/runner.rs](/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/runner.rs) — current subprocess runner
- [crates/agent/src/plugin/manifest.rs](/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/manifest.rs) — plugin manifest
- [crates/agent/src/plugin/mod.rs](/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/mod.rs) — plugin module

## Overview
- **Priority:** P1
- **Status:** Complete
- **Implementation Status:** complete
- **Review Status:** complete
- **Effort:** 2.5h
- **Description:** WebAssembly-based plugin isolation using wasmtime. Define plugin interface via WASM exports/imports. Memory limits via ResourceLimiter, execution timeout via fuel metering. Subprocess runner remains as fallback.

## Key Insights
- Current plugin system: subprocess-based, JSON-RPC over stdio (no isolation beyond OS process)
- wasmtime (Bytecode Alliance): production-ready, WASI support, fuel-based metering, ResourceLimiter
- Plugin interface: WASM module exports `execute(input_ptr, input_len) -> (output_ptr, output_len)`
- Host provides imports: `host_log(msg_ptr, msg_len)`, `host_read_note(id_ptr, id_len) -> (ptr, len)`
- No extism dependency (raw wasmtime for control)
- Plugin manifest extended: `runtime = "wasm" | "subprocess"` field

## Requirements

### Functional
- `WasmPluginRunner` loads `.wasm` files and executes them in sandboxed wasmtime engine
- Plugin manifest `runtime` field: `"wasm"` (default) or `"subprocess"` (legacy)
- Memory limit: configurable per-plugin (default 64MB via ResourceLimiter)
- Execution timeout: fuel metering (default 1M fuel units ~ 1s wall time)
- Host imports: `host_log`, `host_read_note` (minimal surface area)
- WASM plugin receives JSON input via linear memory, returns JSON output
- Fallback to subprocess runner when `runtime = "subprocess"`

### Non-Functional
- <50ms WASM module instantiation (cached compilation)
- <100MB memory per plugin instance
- Deterministic execution (fuel-based, not wall-clock timeout)

## Architecture

```
PluginManifest { runtime: "wasm", executable: "plugin.wasm", ... }
    |
    v
PluginAgent::execute()
    |
    +-- runtime == "wasm" --> WasmPluginRunner
    |       |
    |       +-- wasmtime::Engine (with fuel config)
    |       +-- wasmtime::Store (with ResourceLimiter)
    |       +-- Load & compile .wasm module (cached)
    |       +-- Create instance with host imports
    |       +-- Write JSON input to WASM linear memory
    |       +-- Call exported `execute` function
    |       +-- Read JSON output from linear memory
    |       +-- Return parsed Value
    |
    +-- runtime == "subprocess" --> existing PluginAgent (subprocess)
```

WASM Plugin Contract:
```
// Plugin exports:
fn allocate(size: u32) -> u32;          // allocate memory for input
fn deallocate(ptr: u32, size: u32);     // free memory
fn execute(ptr: u32, len: u32) -> u64;  // run plugin, return (ptr << 32) | len

// Host imports:
fn host_log(ptr: u32, len: u32);                    // log message
fn host_read_note(id_ptr: u32, id_len: u32) -> u64; // read note, return (ptr, len)
```

## Related Code Files

### Modify
- `Cargo.toml` — add wasmtime workspace dep
- `crates/agent/Cargo.toml` — add wasmtime dep
- `crates/agent/src/plugin/manifest.rs` — add `runtime` field, WASM config fields
- `crates/agent/src/plugin/runner.rs` — dispatch based on runtime field
- `crates/agent/src/plugin/mod.rs` — export WasmPluginRunner
- `crates/core/src/config.rs` — add WasmConfig to PluginsConfig
- `crates/core/src/error.rs` — add Wasm(String) variant

### Create
- `crates/agent/src/plugin/wasm_runner.rs` — WasmPluginRunner implementation
- `crates/agent/src/plugin/wasm_host.rs` — host import functions

## Implementation Steps

1. Add wasmtime to workspace Cargo.toml:
   ```toml
   wasmtime = "28"
   ```
2. Add `Wasm(String)` error variant to `error.rs`
3. Extend `PluginManifest` with WASM fields:
   ```rust
   pub struct PluginManifest {
       // ... existing fields
       pub runtime: PluginRuntime,  // default Wasm
       pub memory_limit_mb: u32,    // default 64
       pub fuel_limit: u64,         // default 1_000_000
   }
   pub enum PluginRuntime { Wasm, Subprocess }
   ```
4. Add `WasmConfig` to `PluginsConfig`:
   ```rust
   pub struct WasmConfig {
       pub default_memory_limit_mb: u32,  // 64
       pub default_fuel_limit: u64,       // 1_000_000
       pub cache_compiled: bool,          // true
   }
   ```
5. Create `wasm_host.rs` — host import definitions:
   - `host_log`: write message to tracing
   - `host_read_note`: read note content from vault (requires vault reference in Store data)
6. Create `wasm_runner.rs`:
   ```rust
   pub struct WasmPluginRunner {
       engine: Engine,
       module_cache: HashMap<PathBuf, Module>,
   }
   impl WasmPluginRunner {
       pub fn new(config: &WasmConfig) -> Result<Self>;
       pub async fn execute(&mut self, manifest: &PluginManifest, input: Value) -> Result<Value>;
   }
   ```
   - Engine config: `config.consume_fuel(true)`, `config.wasm_memory_limit(true)`
   - Store config: `store.set_fuel(fuel_limit)`, custom ResourceLimiter
   - Load module: check cache, else compile from .wasm file
   - Write input: serialize JSON, call `allocate`, write to memory
   - Call `execute`: get output ptr+len, read from memory
   - Parse JSON output
7. Modify `runner.rs` to dispatch:
   ```rust
   match manifest.runtime {
       PluginRuntime::Wasm => wasm_runner.execute(manifest, input).await,
       PluginRuntime::Subprocess => /* existing subprocess logic */,
   }
   ```
8. Add tests: WASM execution, memory limit, fuel exhaustion, host imports

## Todo List
- [x]Add wasmtime workspace dep
- [x]Add Wasm error variant
- [x]Extend PluginManifest with runtime + limits
- [x]Add WasmConfig to PluginsConfig
- [x]Create wasm_host.rs with host imports
- [x]Create wasm_runner.rs with WasmPluginRunner
- [x]Update runner.rs dispatch logic
- [x]Create test WASM module (Rust -> wasm32-wasi)
- [x]Add unit tests (execution, limits, errors)
- [x]Add integration test with sample plugin

## Success Criteria
- WASM plugin loads, receives JSON, returns JSON output
- Memory limit enforced: plugin exceeding limit gets trapped
- Fuel limit enforced: infinite loop gets trapped
- Subprocess plugins still work (`runtime = "subprocess"`)
- Existing plugin tests pass (subprocess fallback)
- Module compilation cached (second run faster)

## Risk Assessment
- **wasmtime binary size**: wasmtime adds ~5-10MB to binary. Acceptable for production tool.
- **WASM plugin ecosystem**: Authors must compile to wasm32-wasi. Mitigate: provide template Rust project.
- **Host import surface**: Minimal imports reduce attack surface. Only log + read_note initially.
- **Fuel calibration**: 1M fuel ~ 1s is approximate. May need tuning per workload.

## Security Considerations
- WASM sandbox: no filesystem access, no network access (WASI subset)
- Memory isolated: each plugin instance has own linear memory
- Host imports are explicit opt-in (only log + read_note)
- `host_read_note` is read-only; plugins cannot modify vault directly
- Plugin output goes through review queue (existing trust levels apply)
- ResourceLimiter prevents memory exhaustion on host

## Next Steps
- Independent of other phases
- Future: WASI filesystem subset for plugin temp files
- Future: plugin template generator CLI command
- Future: plugin marketplace/registry
