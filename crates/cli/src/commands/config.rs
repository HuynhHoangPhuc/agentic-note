use agentic_note_core::config::AppConfig;
use std::path::PathBuf;

use crate::output::{print_json, OutputFormat};

pub fn show(vault_path: &PathBuf, fmt: OutputFormat) -> anyhow::Result<()> {
    let config = AppConfig::load(Some(vault_path.clone()))?;

    match fmt {
        OutputFormat::Json => {
            // Mask API keys in JSON output
            let mut json = serde_json::to_value(&config)?;
            if let Some(providers) = json.pointer_mut("/llm/providers") {
                if let Some(obj) = providers.as_object_mut() {
                    for (_name, provider) in obj.iter_mut() {
                        if let Some(key) = provider.get_mut("api_key") {
                            let val = key.as_str().unwrap_or("");
                            if val.len() > 8 {
                                *key = serde_json::json!(format!(
                                    "{}...{}",
                                    &val[..4],
                                    &val[val.len() - 4..]
                                ));
                            } else {
                                *key = serde_json::json!("****");
                            }
                        }
                    }
                }
            }
            print_json(&json);
        }
        OutputFormat::Human => {
            let config_path = vault_path.join(".agentic").join("config.toml");
            let content = std::fs::read_to_string(&config_path)?;
            // Mask API keys in human-readable output
            let masked = regex::Regex::new(r#"(?m)(api_key\s*=\s*")([^"]{8,})(")"#)
                .map(|re| {
                    re.replace_all(&content, |caps: &regex::Captures| {
                        let val = &caps[2];
                        format!(
                            "{}{}...{}{}",
                            &caps[1],
                            &val[..4],
                            &val[val.len() - 4..],
                            &caps[3]
                        )
                    })
                    .to_string()
                })
                .unwrap_or(content);
            println!("{masked}");
        }
    }
    Ok(())
}
