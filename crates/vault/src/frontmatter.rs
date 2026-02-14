use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::FrontMatter;

const DELIMITER: &str = "---";

/// Parse a raw markdown string into (FrontMatter, body).
/// Expects `---\nyaml\n---\nbody` format.
pub fn parse(raw: &str) -> Result<(FrontMatter, String)> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with(DELIMITER) {
        return Err(AgenticError::Parse("missing frontmatter delimiter".into()));
    }

    let after_first = &trimmed[DELIMITER.len()..];
    let after_first = after_first.strip_prefix('\n').unwrap_or(after_first);

    let end_pos = after_first
        .find(&format!("\n{DELIMITER}"))
        .ok_or_else(|| AgenticError::Parse("missing closing frontmatter delimiter".into()))?;

    let yaml_str = &after_first[..end_pos];
    let body_start = end_pos + 1 + DELIMITER.len(); // skip \n---
    let body = if body_start < after_first.len() {
        after_first[body_start..]
            .strip_prefix('\n')
            .unwrap_or(&after_first[body_start..])
    } else {
        ""
    };

    let fm: FrontMatter = serde_yaml::from_str(yaml_str)
        .map_err(|e| AgenticError::Parse(format!("invalid frontmatter YAML: {e}")))?;

    Ok((fm, body.to_string()))
}

/// Serialize frontmatter + body back into a markdown string.
pub fn serialize(fm: &FrontMatter, body: &str) -> Result<String> {
    let yaml = serde_yaml::to_string(fm)
        .map_err(|e| AgenticError::Parse(format!("failed to serialize frontmatter: {e}")))?;
    Ok(format!("{DELIMITER}\n{yaml}{DELIMITER}\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentic_note_core::types::{NoteId, NoteStatus, ParaCategory};
    use chrono::Utc;

    #[test]
    fn test_roundtrip() {
        let fm = FrontMatter {
            id: NoteId::new(),
            title: "Test Note".into(),
            created: Utc::now(),
            modified: Utc::now(),
            tags: vec!["rust".into(), "test".into()],
            para: ParaCategory::Inbox,
            links: vec![],
            status: NoteStatus::Seed,
        };
        let body = "Hello world\n\nSome content here.";
        let raw = serialize(&fm, body).unwrap();
        let (fm2, body2) = parse(&raw).unwrap();
        assert_eq!(fm.id, fm2.id);
        assert_eq!(fm.title, fm2.title);
        assert_eq!(fm.tags, fm2.tags);
        assert_eq!(body, body2);
    }
}
