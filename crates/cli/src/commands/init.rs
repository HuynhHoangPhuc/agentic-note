use agentic_note_vault::init_vault;
use std::path::PathBuf;

use crate::output::{print_json, OutputFormat};

pub fn run(path: Option<PathBuf>, fmt: OutputFormat) -> anyhow::Result<()> {
    let vault_path = match path {
        Some(p) => p,
        None => std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("resolve current directory for init: {e}"))?,
    };
    init_vault(&vault_path)?;

    match fmt {
        OutputFormat::Json => {
            print_json(&serde_json::json!({
                "status": "ok",
                "path": vault_path,
            }));
        }
        OutputFormat::Human => {
            println!("Vault initialized at {}", vault_path.display());
        }
    }
    Ok(())
}
