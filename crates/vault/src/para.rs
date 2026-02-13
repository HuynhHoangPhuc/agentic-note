use agentic_note_core::error::Result;
use agentic_note_core::types::ParaCategory;
use std::path::{Path, PathBuf};

/// All PARA category folders in creation order.
pub const PARA_FOLDERS: &[ParaCategory] = &[
    ParaCategory::Inbox,
    ParaCategory::Projects,
    ParaCategory::Areas,
    ParaCategory::Resources,
    ParaCategory::Archives,
    ParaCategory::Zettelkasten,
];

/// Get the folder path for a PARA category within a vault.
pub fn para_path(vault: &Path, category: &ParaCategory) -> PathBuf {
    vault.join(category.to_string())
}

/// Detect PARA category from a file path by checking parent directory name.
pub fn detect_category(path: &Path) -> Option<ParaCategory> {
    let parent = path.parent()?.file_name()?.to_str()?;
    match parent {
        "inbox" => Some(ParaCategory::Inbox),
        "projects" => Some(ParaCategory::Projects),
        "areas" => Some(ParaCategory::Areas),
        "resources" => Some(ParaCategory::Resources),
        "archives" => Some(ParaCategory::Archives),
        "zettelkasten" => Some(ParaCategory::Zettelkasten),
        _ => None,
    }
}

/// Validate vault structure, returning list of issues found.
pub fn validate_structure(vault: &Path) -> Result<Vec<String>> {
    let mut issues = Vec::new();
    for cat in PARA_FOLDERS {
        let dir = para_path(vault, cat);
        if !dir.exists() {
            issues.push(format!("missing folder: {}", cat));
        }
    }
    let agentic_dir = vault.join(".agentic");
    if !agentic_dir.exists() {
        issues.push("missing .agentic/ directory".into());
    }
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_category() {
        let path = PathBuf::from("/vault/inbox/note.md");
        assert_eq!(detect_category(&path), Some(ParaCategory::Inbox));

        let path = PathBuf::from("/vault/zettelkasten/idea.md");
        assert_eq!(detect_category(&path), Some(ParaCategory::Zettelkasten));

        let path = PathBuf::from("/vault/random/note.md");
        assert_eq!(detect_category(&path), None);
    }
}
