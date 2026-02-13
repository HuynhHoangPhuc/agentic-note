pub mod distiller;
pub mod para_classifier;
pub mod vault_writer;
pub mod zettelkasten_linker;

pub use distiller::Distiller;
pub use para_classifier::ParaClassifier;
pub use vault_writer::VaultWriter;
pub use zettelkasten_linker::ZettelkastenLinker;

use crate::engine::AgentSpace;
use crate::llm::LlmProvider;
use agentic_note_search::SearchEngine;
use std::sync::{Arc, Mutex};

/// Register all built-in agent handlers into the given `AgentSpace`.
///
/// `search` is wrapped in a `Mutex` because `SearchEngine` contains a
/// non-Sync `rusqlite::Connection` but `AgentHandler` requires `Send + Sync`.
pub fn register_builtin_agents(
    space: &mut AgentSpace,
    llm: Arc<dyn LlmProvider>,
    search: Option<Arc<Mutex<SearchEngine>>>,
) {
    space.register_agent(Arc::new(ParaClassifier::new(Arc::clone(&llm))));
    space.register_agent(Arc::new(Distiller::new(Arc::clone(&llm))));
    space.register_agent(Arc::new(ZettelkastenLinker::new(Arc::clone(&llm), search)));
    space.register_agent(Arc::new(VaultWriter::new()));
}
