//! Semantic merge using paragraph-level 3-way diff via `diffy`.
//!
//! Tier 1: Attempt automatic paragraph-level merge using diffy.
//! Returns either a clean merge result or conflict hunks for LLM/manual resolution.

/// Result of attempting a paragraph-level semantic merge.
#[derive(Debug, Clone)]
pub enum MergeAttempt {
    /// All changes merged cleanly without conflicts.
    Clean(String),
    /// Some sections conflict and need LLM or manual resolution.
    HasConflicts {
        /// Partially merged text (non-conflicting sections resolved).
        merged_partial: String,
        /// Conflicting hunks requiring resolution.
        conflicts: Vec<ConflictHunk>,
    },
}

/// A single conflicting hunk from the 3-way merge.
#[derive(Debug, Clone)]
pub struct ConflictHunk {
    pub ancestor: String,
    pub local: String,
    pub remote: String,
}

/// Attempt a paragraph-level 3-way merge of markdown text.
///
/// Splits text on `\n\n` paragraph boundaries and applies diffy's merge.
/// Returns `Clean` if all paragraphs merge without conflicts, or
/// `HasConflicts` with the unresolved hunks.
pub fn try_paragraph_merge(ancestor: &str, local: &str, remote: &str) -> MergeAttempt {
    let merged = diffy::merge(ancestor, local, remote);

    match merged {
        Ok(clean) => MergeAttempt::Clean(clean),
        Err(conflicted) => {
            // Parse the conflicted output to extract conflict hunks
            let (partial, conflicts) = parse_conflict_markers(&conflicted);
            if conflicts.is_empty() {
                // Diffy returned Err but no conflict markers found -- treat as clean
                MergeAttempt::Clean(conflicted)
            } else {
                MergeAttempt::HasConflicts {
                    merged_partial: partial,
                    conflicts,
                }
            }
        }
    }
}

/// Parse diffy conflict markers from merged text.
/// Diffy uses: `<<<<<<<\nlocal\n|||||||\nancestor\n=======\nremote\n>>>>>>>\n`
fn parse_conflict_markers(text: &str) -> (String, Vec<ConflictHunk>) {
    let mut partial = String::new();
    let mut conflicts = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            let mut local_lines = Vec::new();
            let mut ancestor_lines = Vec::new();
            let mut remote_lines = Vec::new();
            let mut section = "local";

            for inner_line in lines.by_ref() {
                if inner_line.starts_with("|||||||") {
                    section = "ancestor";
                } else if inner_line.starts_with("=======") {
                    section = "remote";
                } else if inner_line.starts_with(">>>>>>>") {
                    break;
                } else {
                    match section {
                        "local" => local_lines.push(inner_line),
                        "ancestor" => ancestor_lines.push(inner_line),
                        "remote" => remote_lines.push(inner_line),
                        _ => {}
                    }
                }
            }

            conflicts.push(ConflictHunk {
                ancestor: ancestor_lines.join("\n"),
                local: local_lines.join("\n"),
                remote: remote_lines.join("\n"),
            });
        } else {
            if !partial.is_empty() {
                partial.push('\n');
            }
            partial.push_str(line);
        }
    }

    (partial, conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_overlapping_edits_merge_cleanly() {
        let ancestor = "Paragraph A\n\nParagraph B\n\nParagraph C";
        let local = "Paragraph A modified\n\nParagraph B\n\nParagraph C";
        let remote = "Paragraph A\n\nParagraph B\n\nParagraph C modified";

        match try_paragraph_merge(ancestor, local, remote) {
            MergeAttempt::Clean(merged) => {
                assert!(merged.contains("Paragraph A modified"));
                assert!(merged.contains("Paragraph C modified"));
            }
            MergeAttempt::HasConflicts { .. } => {
                panic!("Non-overlapping edits should merge cleanly");
            }
        }
    }

    #[test]
    fn test_overlapping_edits_produce_conflicts() {
        let ancestor = "Same line";
        let local = "Local change";
        let remote = "Remote change";

        match try_paragraph_merge(ancestor, local, remote) {
            MergeAttempt::HasConflicts { conflicts, .. } => {
                assert!(!conflicts.is_empty(), "Should have at least one conflict");
            }
            MergeAttempt::Clean(_) => {
                panic!("Overlapping edits to same line should conflict");
            }
        }
    }

    #[test]
    fn test_identical_changes_merge_cleanly() {
        let ancestor = "Original text";
        let both = "Same edit by both";

        match try_paragraph_merge(ancestor, both, both) {
            MergeAttempt::Clean(merged) => {
                assert_eq!(merged, "Same edit by both");
            }
            MergeAttempt::HasConflicts { .. } => {
                panic!("Identical changes should merge cleanly");
            }
        }
    }

    #[test]
    fn test_no_changes_returns_clean() {
        let text = "No changes here";
        match try_paragraph_merge(text, text, text) {
            MergeAttempt::Clean(merged) => {
                assert_eq!(merged, text);
            }
            MergeAttempt::HasConflicts { .. } => {
                panic!("No changes should be clean");
            }
        }
    }
}
