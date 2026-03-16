use zenon_search::SearchEngine;
use zenon_test_utils::TempVault;
use zenon_vault::{Note, NoteFilter, Vault};

#[test]
fn note_create_index_search_delete() -> zenon_core::Result<()> {
    let vault = TempVault::new()?;
    let note = Note::create(
        vault.path(),
        "Lifecycle",
        zenon_core::types::ParaCategory::Inbox,
        "Hello world",
        vec!["tag".to_string()],
    )?;

    let mut engine = SearchEngine::open(vault.path())?;
    engine.index_note(&note)?;

    let results = engine.search_fts("Hello", 10)?;
    assert!(!results.is_empty());

    Note::delete(&note.path)?;
    let vault_state = Vault::open(vault.path())?;
    let notes = vault_state.list_notes(&NoteFilter::default())?;
    assert!(notes.is_empty());
    Ok(())
}
