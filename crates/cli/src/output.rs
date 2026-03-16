use zenon_vault::{Note, NoteSummary};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Human,
    Json,
}

pub fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

pub fn print_note(note: &Note, fmt: OutputFormat) {
    match fmt {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "id": note.id.to_string(),
                "title": note.frontmatter.title,
                "para": note.frontmatter.para,
                "tags": note.frontmatter.tags,
                "status": note.frontmatter.status,
                "created": note.frontmatter.created,
                "modified": note.frontmatter.modified,
                "body": note.body,
                "path": note.path,
            });
            print_json(&json);
        }
        OutputFormat::Human => {
            println!("ID:       {}", note.id);
            println!("Title:    {}", note.frontmatter.title);
            println!("PARA:     {}", note.frontmatter.para);
            println!("Tags:     {}", note.frontmatter.tags.join(", "));
            println!(
                "Created:  {}",
                note.frontmatter.created.format("%Y-%m-%d %H:%M")
            );
            println!(
                "Modified: {}",
                note.frontmatter.modified.format("%Y-%m-%d %H:%M")
            );
            println!("Path:     {}", note.path.display());
            println!("---");
            println!("{}", note.body);
        }
    }
}

pub fn print_notes(notes: &[NoteSummary], fmt: OutputFormat) {
    match fmt {
        OutputFormat::Json => {
            let items: Vec<_> = notes
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "id": n.id.to_string(),
                        "title": n.title,
                        "para": n.para,
                        "tags": n.tags,
                        "status": n.status,
                        "modified": n.modified,
                        "path": n.path,
                    })
                })
                .collect();
            print_json(&items);
        }
        OutputFormat::Human => {
            if notes.is_empty() {
                println!("No notes found.");
                return;
            }
            for n in notes {
                println!(
                    "{} | {:<30} | {:<12} | {}",
                    &n.id.to_string()[..8],
                    truncate(&n.title, 30),
                    n.para,
                    n.modified.format("%Y-%m-%d"),
                );
            }
            println!("\n{} note(s)", notes.len());
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}
