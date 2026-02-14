use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::manifest::PluginManifest;
use crate::engine::context::StageContext;
use crate::engine::executor::AgentHandler;

/// Agent handler that executes a plugin as a subprocess.
/// Sends StageContext as JSON on stdin, reads JSON from stdout.
pub struct PluginAgent {
    manifest: PluginManifest,
    plugin_dir: PathBuf,
}

impl PluginAgent {
    pub fn new(manifest: PluginManifest, plugin_dir: PathBuf) -> Self {
        Self {
            manifest,
            plugin_dir,
        }
    }
}

#[async_trait]
impl AgentHandler for PluginAgent {
    fn agent_id(&self) -> &str {
        &self.manifest.name
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        let exe_path = self.plugin_dir.join(&self.manifest.executable);
        if !exe_path.exists() {
            return Err(AgenticError::Plugin(format!(
                "executable not found: {}",
                exe_path.display()
            )));
        }

        // Serialize context to JSON for plugin stdin
        let input = serde_json::json!({
            "note_id": ctx.note_id.to_string(),
            "note_content": ctx.note_content,
            "frontmatter": ctx.frontmatter,
            "outputs": ctx.outputs,
        });
        let input_bytes = serde_json::to_vec(&input)
            .map_err(|e| AgenticError::Plugin(format!("serialize input: {e}")))?;

        // Spawn subprocess
        let mut child = Command::new(&exe_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&self.plugin_dir)
            .spawn()
            .map_err(|e| AgenticError::Plugin(format!("spawn {}: {e}", exe_path.display())))?;

        // Write JSON to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&input_bytes)
                .await
                .map_err(|e| AgenticError::Plugin(format!("write stdin: {e}")))?;
            // stdin dropped here, closing it
        }

        // Wait with timeout (use wait_with_output which takes ownership)
        let timeout_dur = Duration::from_secs(self.manifest.timeout_secs);
        let name = self.manifest.name.clone();
        let secs = self.manifest.timeout_secs;
        let output = tokio::time::timeout(timeout_dur, child.wait_with_output())
            .await
            .map_err(|_| AgenticError::Plugin(format!("plugin '{name}' timed out after {secs}s")))?
            .map_err(|e| AgenticError::Plugin(format!("process error: {e}")))?;

        // Check exit code
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgenticError::Plugin(format!(
                "plugin '{}' exited with {}: {}",
                self.manifest.name,
                output.status,
                stderr.trim()
            )));
        }

        // Parse stdout as JSON
        serde_json::from_slice(&output.stdout)
            .map_err(|e| AgenticError::Plugin(format!("parse output: {e}")))
    }
}
