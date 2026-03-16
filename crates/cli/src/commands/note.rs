use zenon_core::types::{NoteId, NoteStatus, ParaCategory};
use zenon_vault::{Note, NoteFilter, Vault};
use clap::Subcommand;
use std::path::PathBuf;
use std::str::FromStr;

use crate::output::{print_json, print_note, print_notes, OutputFormat};

#[derive(Subcommand)]
pub enum NoteCmd {
    /// Create a new note
    Create {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "inbox")]
        para: String,
        #[arg(long)]
        tags: Vec<String>,
        #[arg(long, default_value = "")]
        body: String,
    },
    /// Read a note by ID or path
    Read {
        /// Note ID (ULID) or file path
        target: String,
    },
    /// Update a note
    Update {
        /// Note ID (ULID)
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        tags: Option<Vec<String>>,
        #[arg(long)]
        para: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Delete a note
    Delete {
        /// Note ID (ULID) or path
        target: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// List notes
    List {
        #[arg(long)]
        para: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value = "50")]
        limit: usize,
    },
}

pub fn run(cmd: NoteCmd, vault_path: &PathBuf, fmt: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        NoteCmd::Create {
            title,
            para,
            tags,
            body,
        } => {
            let para = parse_para(&para)?;
            let note = Note::create(vault_path, &title, para, &body, tags)?;
            match fmt {
                OutputFormat::Json => print_json(&serde_json::json!({
                    "status": "created",
                    "id": note.id.to_string(),
                    "path": note.path,
                })),
                OutputFormat::Human => {
                    println!("Created: {} ({})", note.frontmatter.title, note.id);
                    println!("Path:    {}", note.path.display());
                }
            }
        }
        NoteCmd::Read { target } => {
            let path = resolve_note_path(vault_path, &target)?;
            let note = Note::read(&path)?;
            print_note(&note, fmt);
        }
        NoteCmd::Update {
            id,
            title,
            tags,
            para,
            body,
            status,
        } => {
            let path = resolve_note_path(vault_path, &id)?;
            let mut note = Note::read(&path)?;
            if let Some(t) = title {
                note.frontmatter.title = t;
            }
            if let Some(t) = tags {
                note.frontmatter.tags = t;
            }
            if let Some(p) = para {
                note.frontmatter.para = parse_para(&p)?;
            }
            if let Some(b) = body {
                note.body = b;
            }
            if let Some(s) = status {
                note.frontmatter.status = parse_status(&s)?;
            }
            note.update()?;
            match fmt {
                OutputFormat::Json => print_json(&serde_json::json!({
                    "status": "updated",
                    "id": note.id.to_string(),
                })),
                OutputFormat::Human => println!("Updated: {}", note.id),
            }
        }
        NoteCmd::Delete { target, force } => {
            let path = resolve_note_path(vault_path, &target)?;
            if !force {
                eprintln!("Delete {}? Use --force to confirm.", path.display());
                return Ok(());
            }
            Note::delete(&path)?;
            match fmt {
                OutputFormat::Json => print_json(&serde_json::json!({"status": "deleted"})),
                OutputFormat::Human => println!("Deleted: {}", path.display()),
            }
        }
        NoteCmd::List {
            para,
            tag,
            status,
            limit,
        } => {
            let vault = Vault::open(vault_path)?;
            let filter = NoteFilter {
                para: para.as_deref().and_then(|p| parse_para(p).ok()),
                tags: tag.map(|t| vec![t]),
                status: status.as_deref().and_then(|s| parse_status(s).ok()),
            };
            let mut notes = vault.list_notes(&filter)?;
            notes.truncate(limit);
            print_notes(&notes, fmt);
        }
    }
    Ok(())
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
fn resolve_note_path(vault: &PathBuf, target: &str) -> anyhow::Result<PathBuf> {
    // If target looks like a file path
    let as_path = PathBuf::from(target);
    if as_path.exists() {
        return Ok(as_path);
    }

    // Try as ULID — scan vault for matching filename prefix
    let _id = NoteId::from_str(target)
        .map_err(|_| anyhow::anyhow!("not a valid path or ULID: {target}"))?;

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
