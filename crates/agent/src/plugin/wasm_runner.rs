//! WASM plugin runner using wasmtime.
//!
//! Loads .wasm modules, executes them with fuel metering and memory limits,
//! passes JSON input/output through linear memory.

use agentic_note_core::error::{AgenticError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use wasmtime::*;

use super::wasm_host::HostState;
use super::wasm_host::register_host_imports;

/// WASM plugin runner with module caching and resource limits.
pub struct WasmPluginRunner {
    engine: Engine,
    module_cache: HashMap<PathBuf, Module>,
    default_memory_limit_mb: u32,
    default_fuel_limit: u64,
}

impl WasmPluginRunner {
    /// Create a new runner with default configuration.
    pub fn new(memory_limit_mb: u32, fuel_limit: u64) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)
            .map_err(|e| AgenticError::Wasm(format!("create engine: {e}")))?;

        Ok(Self {
            engine,
            module_cache: HashMap::new(),
            default_memory_limit_mb: memory_limit_mb,
            default_fuel_limit: fuel_limit,
        })
    }

    /// Load and cache a WASM module from a file path.
    fn load_module(&mut self, wasm_path: &Path) -> Result<Module> {
        if let Some(module) = self.module_cache.get(wasm_path) {
            return Ok(module.clone());
        }
        let wasm_bytes = std::fs::read(wasm_path)
            .map_err(|e| AgenticError::Wasm(format!("read {}: {e}", wasm_path.display())))?;
        let module = Module::new(&self.engine, &wasm_bytes)
            .map_err(|e| AgenticError::Wasm(format!("compile {}: {e}", wasm_path.display())))?;
        self.module_cache.insert(wasm_path.to_path_buf(), module.clone());
        Ok(module)
    }

    /// Execute a WASM plugin with JSON input, returning JSON output.
    pub fn execute(
        &mut self,
        wasm_path: &Path,
        plugin_name: &str,
        input: &Value,
        fuel_limit: Option<u64>,
        memory_limit_mb: Option<u32>,
    ) -> Result<Value> {
        let module = self.load_module(wasm_path)?;
        let fuel = fuel_limit.unwrap_or(self.default_fuel_limit);
        let mem_mb = memory_limit_mb.unwrap_or(self.default_memory_limit_mb);

        let mut linker = Linker::new(&self.engine);
        register_host_imports(&mut linker)
            .map_err(|e| AgenticError::Wasm(format!("register imports: {e}")))?;

        let state = HostState::new(plugin_name.to_string(), fuel, mem_mb);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limiter);
        store
            .set_fuel(fuel)
            .map_err(|e| AgenticError::Wasm(format!("set fuel: {e}")))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| AgenticError::Wasm(format!("instantiate: {e}")))?;

        // Serialize input to JSON bytes
        let input_bytes = serde_json::to_vec(input)
            .map_err(|e| AgenticError::Wasm(format!("serialize input: {e}")))?;

        // Call allocate to get a pointer for input data
        let allocate = instance
            .get_typed_func::<u32, u32>(&mut store, "allocate")
            .map_err(|e| AgenticError::Wasm(format!("get allocate: {e}")))?;
        let input_ptr = allocate
            .call(&mut store, input_bytes.len() as u32)
            .map_err(|e| AgenticError::Wasm(format!("allocate call: {e}")))?;

        // Write input bytes to WASM memory
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| AgenticError::Wasm("no memory export".into()))?;
        memory
            .write(&mut store, input_ptr as usize, &input_bytes)
            .map_err(|e| AgenticError::Wasm(format!("write input: {e}")))?;

        // Call execute(ptr, len) -> u64 where result = (out_ptr << 32) | out_len
        let execute_fn = instance
            .get_typed_func::<(u32, u32), u64>(&mut store, "execute")
            .map_err(|e| AgenticError::Wasm(format!("get execute: {e}")))?;
        let result = execute_fn
            .call(&mut store, (input_ptr, input_bytes.len() as u32))
            .map_err(|e| AgenticError::Wasm(format!("execute call: {e}")))?;

        let out_ptr = (result >> 32) as u32;
        let out_len = (result & 0xFFFF_FFFF) as u32;

        // Read output bytes from WASM memory
        let mut output_bytes = vec![0u8; out_len as usize];
        memory
            .read(&store, out_ptr as usize, &mut output_bytes)
            .map_err(|e| AgenticError::Wasm(format!("read output: {e}")))?;

        // Parse JSON output
        serde_json::from_slice(&output_bytes)
            .map_err(|e| AgenticError::Wasm(format!("parse output: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_creation() {
        let runner = WasmPluginRunner::new(64, 1_000_000);
        assert!(runner.is_ok());
    }

    #[test]
    fn missing_wasm_file_errors() {
        let mut runner = WasmPluginRunner::new(64, 1_000_000)
            .expect("create wasm runner");
        let result = runner.execute(
            Path::new("/nonexistent.wasm"),
            "test",
            &serde_json::json!({}),
            None,
            None,
        );
        assert!(result.is_err());
        let err = result.expect_err("missing wasm file error");
        assert!(err.to_string().contains("read"));
    }
}
