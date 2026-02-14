/// Tool call dispatch for MCP `tools/call` method.
use agentic_note_agent::plugin;
use agentic_note_core::config::AppConfig;
use agentic_note_core::types::{NoteStatus, ParaCategory};
use agentic_note_search::SearchEngine;
use agentic_note_vault::{init_vault, Note, NoteFilter, Vault};
use serde_json::Value;
use std::path::Path;

/// Dispatch a tool call by name, returning a JSON result value.
pub async fn handle_tool(name: &str, args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    match name {
        "note/create" => tool_note_create(args, vault_path),
        "note/read" => tool_note_read(args, vault_path),
        "note/list" => tool_note_list(args, vault_path),
        "note/search" => tool_note_search(args, vault_path),
        "vault/init" => tool_vault_init(args, vault_path),
        "vault/status" => tool_vault_status(vault_path),
        "plugin/list" => tool_plugin_list(vault_path),
        _ => anyhow::bail!("unknown tool: {name}"),
    }
}

fn tool_note_create(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    let title = args["title"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required param: title"))?
        .to_string();
    let para_str = args["para"].as_str().unwrap_or("inbox");
    let body = args["body"].as_str().unwrap_or("").to_string();
    let tags: Vec<String> = args["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let para = parse_para(para_str)?;
    let note = Note::create(vault_path, &title, para, &body, tags)?;

    Ok(serde_json::json!({
        "status": "created",
        "id": note.id.to_string(),
        "title": note.frontmatter.title,
        "path": note.path.display().to_string(),
    }))
}

fn tool_note_read(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    let target = args["target"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required param: target"))?;
    let path = resolve_note_path(vault_path, target)?;
    let note = Note::read(&path)?;

    Ok(serde_json::json!({
        "id": note.id.to_string(),
        "title": note.frontmatter.title,
        "para": format!("{:?}", note.frontmatter.para).to_lowercase(),
        "tags": note.frontmatter.tags,
        "status": format!("{:?}", note.frontmatter.status).to_lowercase(),
        "body": note.body,
        "path": note.path.display().to_string(),
    }))
}

fn tool_note_list(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    let vault = Vault::open(vault_path)?;
    let filter = NoteFilter {
        para: args["para"].as_str().and_then(|p| parse_para(p).ok()),
        tags: args["tag"].as_str().map(|t| vec![t.to_string()]),
        status: args["status"].as_str().and_then(|s| parse_status(s).ok()),
    };
    let limit = args["limit"].as_u64().unwrap_or(50) as usize;
    let mut notes = vault.list_notes(&filter)?;
    notes.truncate(limit);

    let items: Vec<Value> = notes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id.to_string(),
                "title": n.title,
                "para": format!("{:?}", n.para).to_lowercase(),
                "tags": n.tags,
                "status": format!("{:?}", n.status).to_lowercase(),
                "modified": n.modified.to_rfc3339(),
                "path": n.path.display().to_string(),
            })
        })
        .collect();

    Ok(serde_json::json!({ "notes": items, "count": items.len() }))
}

fn tool_note_search(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required param: query"))?;
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;
    let mode = args["mode"].as_str().unwrap_or("fts");

    #[allow(unused_mut)]
    let mut engine =
        SearchEngine::open(vault_path).map_err(|e| anyhow::anyhow!("search engine: {e}"))?;

    let results = match mode {
        #[cfg(feature = "embeddings")]
        "semantic" => engine
            .search_semantic(query, limit)
            .map_err(|e| anyhow::anyhow!("semantic search: {e}"))?,
        #[cfg(feature = "embeddings")]
        "hybrid" => engine
            .search_hybrid(query, limit)
            .map_err(|e| anyhow::anyhow!("hybrid search: {e}"))?,
        _ => engine
            .search_fts(query, limit)
            .map_err(|e| anyhow::anyhow!("search: {e}"))?,
    };

    let items: Vec<Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id.to_string(),
                "title": r.title,
                "score": r.score,
                "snippet": r.snippet,
            })
        })
        .collect();

    Ok(serde_json::json!({ "results": items, "count": items.len(), "mode": mode }))
}

fn tool_plugin_list(vault_path: &Path) -> anyhow::Result<Value> {
    let plugins_dir = match AppConfig::load(Some(vault_path.to_path_buf())) {
        Ok(config) => vault_path.join(&config.plugins.plugins_dir),
        Err(_) => vault_path.join("plugins"),
    };
    let discovered = plugin::discover_plugins(&plugins_dir).unwrap_or_default();

    let items: Vec<Value> = discovered
        .iter()
        .map(|(manifest, path)| {
            serde_json::json!({
                "name": manifest.name,
                "version": manifest.version,
                "description": manifest.description,
                "path": path.display().to_string(),
            })
        })
        .collect();

    Ok(serde_json::json!({ "plugins": items, "count": items.len() }))
}

fn tool_vault_init(args: Value, vault_path: &Path) -> anyhow::Result<Value> {
    // Only allow initializing the configured vault path (no arbitrary paths via MCP).
    let path = vault_path.to_path_buf();
    let _ignored = args["path"].as_str(); // arg accepted but not used for safety

    init_vault(&path).map_err(|e| anyhow::anyhow!("init vault: {e}"))?;

    Ok(serde_json::json!({
        "status": "initialized",
        "path": path.display().to_string(),
    }))
}

fn tool_vault_status(vault_path: &Path) -> anyhow::Result<Value> {
    let vault = Vault::open(vault_path)?;
    let notes = vault.list_notes(&NoteFilter::default())?;

    Ok(serde_json::json!({
        "path": vault_path.display().to_string(),
        "note_count": notes.len(),
    }))
}

fn parse_para(s: &str) -> anyhow::Result<ParaCategory> {
    match s.to_lowercase().as_str() {
        "inbox" => Ok(ParaCategory::Inbox),
        "projects" => Ok(ParaCategory::Projects),
        "areas" => Ok(ParaCategory::Areas),
        "resources" => Ok(ParaCategory::Resources),
        "archives" => Ok(ParaCategory::Archives),
        "zettelkasten" | "zk" => Ok(ParaCategory::Zettelkasten),
        _ => anyhow::bail!("invalid PARA category: {s}"),
    }
}

fn parse_status(s: &str) -> anyhow::Result<NoteStatus> {
    match s.to_lowercase().as_str() {
        "seed" => Ok(NoteStatus::Seed),
        "budding" => Ok(NoteStatus::Budding),
        "evergreen" => Ok(NoteStatus::Evergreen),
        _ => anyhow::bail!("invalid status: {s}"),
    }
}

/// Resolve a note target (ULID string or file path) to an actual file path.
/// All resolved paths are validated to be within the vault directory.
fn resolve_note_path(vault: &Path, target: &str) -> anyhow::Result<std::path::PathBuf> {
    let vault_canon = vault.canonicalize().unwrap_or_else(|_| vault.to_path_buf());

    let as_path = std::path::PathBuf::from(target);
    if as_path.exists() {
        let canonical = as_path.canonicalize()?;
        if !canonical.starts_with(&vault_canon) {
            anyhow::bail!("path is outside the vault");
        }
        return Ok(canonical);
    }
    for entry in walkdir::WalkDir::new(vault)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with(target) && name.ends_with(".md") {
                return Ok(entry.path().to_path_buf());
            }
        }
    }
    anyhow::bail!("note not found: {target}")
}
