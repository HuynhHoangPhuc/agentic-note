use agentic_note_core::error::Result;
use std::path::Path;
use tracing::info;

use crate::para::{para_path, PARA_FOLDERS};

const DEFAULT_CONFIG: &str = r#"[vault]
path = "."

[llm]
default_provider = "openai"

# [llm.providers.openai]
# api_key = "sk-..."
# model = "gpt-4o"

[agent]
default_trust = "review"
max_concurrent_pipelines = 1
"#;

/// Initialize a vault at the given path. Creates PARA folders, .agentic/ dir,
/// and default config.toml. Idempotent — skips existing dirs/files.
pub fn init_vault(path: &Path) -> Result<()> {
    // Create PARA folders
    for cat in PARA_FOLDERS {
        let dir = para_path(path, cat);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
            info!("created {}", dir.display());
        }
    }

    // Create .agentic/ system directory
    let agentic_dir = path.join(".agentic");
    if !agentic_dir.exists() {
        std::fs::create_dir_all(&agentic_dir)?;
        info!("created {}", agentic_dir.display());
    }

    // Create default config
    let config_path = agentic_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, DEFAULT_CONFIG)?;
        // Set 0600 permissions on Unix (API keys may be stored here)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600))?;
        }
        info!("created {}", config_path.display());
    }

    // Create .agentic/sessions dir for agent session logs
    let sessions_dir = agentic_dir.join("sessions");
    if !sessions_dir.exists() {
        std::fs::create_dir_all(&sessions_dir)?;
    }

    Ok(())
}
