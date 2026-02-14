//! Host import functions provided to WASM plugins.
//!
//! Minimal surface area: only logging and read-only note access.

use wasmtime::{Caller, Linker, StoreLimits, StoreLimitsBuilder};

/// State stored in wasmtime::Store, accessible from host imports.
pub struct HostState {
    /// Plugin name for log prefix.
    pub plugin_name: String,
    /// Fuel limit for execution.
    pub fuel_limit: u64,
    /// Memory/table limits enforced by wasmtime.
    pub limiter: StoreLimits,
}

impl HostState {
    pub fn new(plugin_name: String, fuel_limit: u64, memory_limit_mb: u32) -> Self {
        let limiter = StoreLimitsBuilder::new()
            .memory_size(memory_limit_mb as usize * 1024 * 1024)
            .build();
        Self {
            plugin_name,
            fuel_limit,
            limiter,
        }
    }
}

/// Register host import functions on the given linker.
pub fn register_host_imports(linker: &mut Linker<HostState>) -> wasmtime::Result<()> {
    // host_log(ptr: i32, len: i32) — log a message from the plugin
    linker.func_wrap(
        "env",
        "host_log",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            if ptr < 0 || len < 0 {
                return;
            }
            let mem = caller.get_export("memory").and_then(|e| e.into_memory());
            if let Some(mem) = mem {
                let data = mem.data(&caller);
                let start = ptr as usize;
                let end = start.saturating_add(len as usize);
                if end <= data.len() {
                    let msg = String::from_utf8_lossy(&data[start..end]);
                    let name = caller.data().plugin_name.clone();
                    tracing::info!(plugin = %name, "[wasm] {msg}");
                }
            }
        },
    )?;

    // host_read_note(id_ptr: i32, id_len: i32) -> i64
    // Returns 0 (not implemented yet — placeholder for future vault access)
    linker.func_wrap(
        "env",
        "host_read_note",
        |_caller: Caller<'_, HostState>, _id_ptr: i32, _id_len: i32| -> i64 {
            // Placeholder: returns 0 (null pointer, zero length)
            // Full implementation requires passing vault reference through Store data
            tracing::debug!("host_read_note called (not yet implemented)");
            0i64
        },
    )?;

    Ok(())
}
