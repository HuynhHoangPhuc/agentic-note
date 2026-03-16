use zenon_core::types::NoteId;

/// Generate a random note title suitable for tests.
pub fn random_note_title() -> String {
    format!("test-note-{}", NoteId::new())
}
