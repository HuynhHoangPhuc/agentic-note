//! Plugin execution: dispatches to WASM sandbox or subprocess based on manifest.

use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::manifest::{PluginManifest, PluginRuntime};
use super::wasm_runner::WasmPluginRunner;
use crate::engine::context::StageContext;
use crate::engine::executor::AgentHandler;

/// Agent handler that executes a plugin via WASM or subprocess.
pub struct PluginAgent {
    manifest: PluginManifest,
    plugin_dir: PathBuf,
    wasm_runner: Option<std::sync::Arc<std::sync::Mutex<WasmPluginRunner>>>,
}

impl PluginAgent {
    pub fn new(manifest: PluginManifest, plugin_dir: PathBuf) -> Self {
        let wasm_runner = if manifest.runtime == PluginRuntime::Wasm {
            WasmPluginRunner::new(manifest.memory_limit_mb, manifest.fuel_limit)
                .ok()
                .map(|r| std::sync::Arc::new(std::sync::Mutex::new(r)))
        } else {
            None
        };
        Self {
            manifest,
            plugin_dir,
            wasm_runner,
        }
    }
}

#[async_trait]
impl AgentHandler for PluginAgent {
    fn agent_id(&self) -> &str {
        &self.manifest.name
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        match self.manifest.runtime {
            PluginRuntime::Wasm => self.execute_wasm(ctx).await,
            PluginRuntime::Subprocess => self.execute_subprocess(ctx).await,
        }
    }
}

impl PluginAgent {
    async fn execute_wasm(&self, ctx: &mut StageContext) -> Result<Value> {
        // Clone all needed data from &self before entering spawn_blocking.
        // Cloning the Arc produces an independent owned handle ('static).
        let runner_ref = self
            .wasm_runner
            .clone()
            .ok_or_else(|| AgenticError::Wasm("WASM runner not initialized".into()))?;

        let wasm_path = self.plugin_dir.join(&self.manifest.executable);
        let input = build_plugin_input(ctx);
        let name = self.manifest.name.clone();
        let fuel = self.manifest.fuel_limit;
        let mem = self.manifest.memory_limit_mb;

        // Run WASM execution in blocking thread (wasmtime is sync).
        tokio::task::spawn_blocking(move || {
            let mut r = runner_ref
                .lock()
                .map_err(|e| AgenticError::Wasm(format!("lock runner: {e}")))?;
            r.execute(&wasm_path, &name, &input, Some(fuel), Some(mem))
        })
        .await
        .map_err(|e| AgenticError::Wasm(format!("spawn_blocking: {e}")))?
    }

    async fn execute_subprocess(&self, ctx: &mut StageContext) -> Result<Value> {
        let exe_path = self.plugin_dir.join(&self.manifest.executable);
        if !exe_path.exists() {
            return Err(AgenticError::Plugin(format!(
                "executable not found: {}",
                exe_path.display()
            )));
        }

        let input = build_plugin_input(ctx);
        let input_bytes = serde_json::to_vec(&input)
            .map_err(|e| AgenticError::Plugin(format!("serialize input: {e}")))?;

        let mut child = Command::new(&exe_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&self.plugin_dir)
            .spawn()
            .map_err(|e| AgenticError::Plugin(format!("spawn {}: {e}", exe_path.display())))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&input_bytes)
                .await
                .map_err(|e| AgenticError::Plugin(format!("write stdin: {e}")))?;
        }

        let timeout_dur = Duration::from_secs(self.manifest.timeout_secs);
        let name = self.manifest.name.clone();
        let secs = self.manifest.timeout_secs;
        let output = tokio::time::timeout(timeout_dur, child.wait_with_output())
            .await
            .map_err(|_| AgenticError::Plugin(format!("plugin '{name}' timed out after {secs}s")))?
            .map_err(|e| AgenticError::Plugin(format!("process error: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgenticError::Plugin(format!(
                "plugin '{}' exited with {}: {}",
                self.manifest.name,
                output.status,
                stderr.trim()
            )));
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|e| AgenticError::Plugin(format!("parse output: {e}")))
    }
}

fn build_plugin_input(ctx: &StageContext) -> Value {
    serde_json::json!({
        "note_id": ctx.note_id.to_string(),
        "note_content": ctx.note_content,
        "frontmatter": ctx.frontmatter,
        "outputs": ctx.outputs,
    })
}
