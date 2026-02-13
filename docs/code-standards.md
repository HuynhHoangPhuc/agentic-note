# Agentic-Note: Code Standards & Development Guidelines

## Overview

This document establishes coding standards, architectural patterns, and development practices for the agentic-note Rust codebase. All contributors must follow these guidelines to maintain consistency, readability, and maintainability.

---

## Core Principles

### YAGNI (You Aren't Gonna Need It)
- Implement only what the current phase requires
- Avoid speculative abstractions
- Defer features to later phases when constraints are clear

### KISS (Keep It Simple, Stupid)
- Prefer straightforward logic over clever optimizations
- Readable code beats compact code
- Explicit types and named values over implicit conversions

### DRY (Don't Repeat Yourself)
- Extract common patterns into shared modules
- Use traits and generics to avoid duplication
- Link to relevant docs rather than duplicating explanations

---

## Project Structure

### Workspace Layout
```
agentic-note/                          # Cargo workspace root
├── Cargo.toml                         # Workspace manifest with shared dependencies
├── Cargo.lock                         # Lock file (committed)
├── crates/
│   ├── core/                          # Shared types, errors, config
│   │   └── src/
│   │       ├── lib.rs                 # Re-exports for public API
│   │       ├── error.rs               # Custom error types
│   │       ├── types.rs               # Core domain types
│   │       ├── config.rs              # Configuration loading
│   │       └── id.rs                  # ULID ID generation
│   ├── vault/                         # Note CRUD and organization
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── note.rs                # Note struct and operations
│   │       ├── frontmatter.rs         # YAML frontmatter parsing
│   │       ├── para.rs                # PARA folder structure
│   │       ├── markdown.rs            # Markdown utilities
│   │       └── init.rs                # Vault initialization
│   ├── cas/                           # Content-addressable storage
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── hash.rs                # SHA-256 hashing
│   │       ├── blob.rs                # Blob store operations
│   │       ├── tree.rs                # Tree structure (vault snapshot)
│   │       ├── snapshot.rs            # Snapshot creation/restore
│   │       ├── diff.rs                # Snapshot diffing
│   │       ├── merge.rs               # Conflict resolution
│   │       ├── cas.rs                 # Main CAS interface
│   │       └── restore.rs             # Restore from snapshots
│   ├── search/                        # FTS and graph indexing
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fts.rs                 # tantivy full-text search
│   │       ├── graph.rs               # SQLite tag/link graph
│   │       └── reindex.rs             # Incremental reindexing
│   ├── agent/                         # AgentSpace engine and agents
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs              # Pipeline execution engine
│   │       ├── llm/                   # LLM provider integrations
│   │       │   ├── mod.rs
│   │       │   ├── openai.rs
│   │       │   ├── anthropic.rs
│   │       │   └── ollama.rs
│   │       └── agents/                # Built-in agent implementations
│   │           ├── mod.rs
│   │           ├── para_classifier.rs
│   │           ├── zettelkasten_linker.rs
│   │           ├── distiller.rs
│   │           └── vault_writer.rs
│   ├── review/                        # Review queue and approval gate
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── queue.rs               # Review item storage
│   │       └── gate.rs                # Approval gate logic
│   └── cli/                           # Binary and commands
│       └── src/
│           ├── lib.rs
│           ├── main.rs                # Entry point, CLI parsing
│           ├── commands/              # Command implementations
│           │   ├── mod.rs
│           │   ├── init.rs
│           │   ├── note.rs
│           │   ├── config.rs
│           │   └── agent.rs
│           ├── mcp/                   # MCP server implementation
│           │   ├── mod.rs
│           │   ├── server.rs
│           │   ├── handlers.rs
│           │   └── messages.rs
│           └── output.rs              # JSON/human output formatting
├── pipelines/                         # Sample TOML pipeline configurations
│   └── auto-process-inbox.toml
├── docs/                              # User and developer documentation
└── target/                            # Build artifacts (gitignored)
```

### File Naming Conventions
- **Rust modules**: snake_case (e.g., `para_classifier.rs`, `openai.rs`)
- **Documentation**: kebab-case for section files (e.g., `system-architecture.md`)
- **Pipelines**: kebab-case (e.g., `auto-process-inbox.toml`)
- **Avoid abbreviations** except widely recognized (ID, UUID, ULID, FTS, CAS, LLM, MCP)

### File Size Targets
- **Source files**: <200 LOC (split if exceeding)
- **Test files**: <300 LOC
- **Documentation files**: <800 LOC (split into indexed directories)

---

## Rust Code Standards

### Module Organization

**lib.rs Template:**
```rust
pub mod config;
pub mod error;
pub mod types;

// Re-export public API
pub use config::AppConfig;
pub use error::{AgenticError, Result};
pub use types::{MyType, AnotherType};
```

**Private Implementation:**
- Keep implementation modules private by not re-exporting
- Use `pub use` only for types/functions that are part of public API
- Internal helper functions: no `pub`, use module-level documentation

### Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| Crates | snake_case | `agentic-note-core` |
| Modules | snake_case | `para_classifier` |
| Types (struct/enum) | PascalCase | `NoteId`, `ParaCategory` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_PIPELINE_STAGES` |
| Functions | snake_case | `list_notes`, `create_note` |
| Type aliases | PascalCase | `Result<T>` |
| Lifetimes | lowercase | `'a`, `'static` |
| Generic params | PascalCase | `<T>`, `<R>` |

### Error Handling

**Custom Error Type (from core):**
```rust
use thiserror::Error;
use agentic_note_core::{AgenticError, Result};

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("note not found: {0}")]
    NotFound(String),

    #[error("invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
}

// Convert to AgenticError if needed
impl From<VaultError> for AgenticError {
    fn from(e: VaultError) -> Self {
        AgenticError::Vault(e.to_string())
    }
}
```

**Error Propagation:**
- Use `?` operator extensively for brevity
- Provide context with `.map_err()` when appropriate
- Never ignore errors with `.unwrap()` or `.expect()` in production code
- Panic only for true programming errors, not user errors

### Type Safety

**Prefer Strong Types:**
```rust
// Good: semantic meaning
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct NoteId(pub Ulid);

// Avoid: primitive type
type NoteId = String;
```

**Implement Traits Explicitly:**
```rust
impl Display for ParaCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projects => write!(f, "projects"),
            // ...
        }
    }
}
```

### Async/Concurrency

**Use tokio for async:**
- Feature flag: `tokio = { version = "1", features = ["full"] }`
- Prefer `#[tokio::main]` for CLI entry points
- Use `async-trait` for async trait methods

**Patterns:**
```rust
// Good: clear async signature
pub async fn process_pipeline(config: &PipelineConfig) -> Result<PipelineResult> {
    // ...
}

// Avoid: hiding async in wrapper functions
pub fn process(config: &PipelineConfig) -> impl Future {
    // ...
}
```

### Testing

**Test Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_creates_note_with_correct_metadata() {
        // Arrange
        let vault = setup_test_vault();

        // Act
        let note = vault.create_note("Test", "Body", vec![]);

        // Assert
        assert_eq!(note.title, "Test");
    }
}
```

**Test Naming:** `test_<function>_<scenario>_<expected_result>`

**Integration Tests:**
- Place in `tests/` directory at crate root
- Use `tempfile` crate for isolated test fixtures
- Clean up resources in `Drop` impl or test cleanup functions

### Serialization

**Derive with Features:**
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontMatter {
    pub id: NoteId,
    pub title: String,
    #[serde(rename_all = "lowercase")]
    pub para: ParaCategory,
}
```

**YAML for Frontmatter:**
- Human-readable format for config and metadata
- Use `serde_yaml` for parsing/serializing
- Comments are preserved in round-trip

**JSON for LLM Output:**
- Use JSON mode when available (OpenAI, Anthropic)
- Validate against schema before processing
- Include field descriptions in schema for better LLM output

### Documentation Comments

**Public Items Must Have Docs:**
```rust
/// Unique note identifier wrapping a ULID for monotonic ordering.
///
/// # Examples
///
/// ```
/// use agentic_note_core::NoteId;
/// let id = NoteId::new();
/// println!("{}", id);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NoteId(pub Ulid);

/// Creates a new note in the vault.
///
/// # Arguments
///
/// * `vault` - Path to the vault root
/// * `title` - Note title (required)
/// * `para` - PARA category
/// * `tags` - Optional tag list
///
/// # Returns
///
/// Returns the created note or an error if file operations fail.
///
/// # Errors
///
/// - `AgenticError::Io` if file creation fails
/// - `AgenticError::Parse` if ULID generation fails
pub fn create_note(vault: &Path, title: &str, para: ParaCategory, tags: Vec<String>) -> Result<Note> {
    // ...
}
```

**Comment Style:**
- Use `///` for public items
- Use `//` for inline comments
- Use `//!` for module-level documentation
- Reference related items with backticks: `` `Note` ``
- Include examples for complex functions

---

## Architecture Patterns

### Trait-Based Abstraction

**LLM Provider Trait:**
```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;
    async fn complete_with_schema(&self, prompt: &str, schema: &str) -> Result<String>;
}
```

**Agent Trait:**
```rust
#[async_trait]
pub trait AgentHandler: Send + Sync {
    async fn execute(&self, context: &StageContext) -> Result<StageOutput>;
}
```

### Repository Pattern

**Vault as Repository:**
```rust
pub struct Vault {
    pub root: PathBuf,
    pub config: AppConfig,
}

impl Vault {
    pub fn open(path: &Path) -> Result<Self> { /* ... */ }
    pub fn list_notes(&self, filter: &NoteFilter) -> Result<Vec<NoteSummary>> { /* ... */ }
    pub fn create_note(&self, note: &Note) -> Result<()> { /* ... */ }
}
```

### Builder Pattern

**For Complex Configuration:**
```rust
pub struct PipelineBuilder {
    config: PipelineConfig,
}

impl PipelineBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            config: PipelineConfig {
                name: name.to_string(),
                ..Default::default()
            },
        }
    }

    pub fn with_stage(mut self, stage: StageConfig) -> Self {
        self.config.stages.push(stage);
        self
    }

    pub fn build(self) -> PipelineConfig {
        self.config
    }
}
```

### Facade Pattern

**SearchEngine Facade:**
```rust
pub struct SearchEngine {
    pub fts: FtsIndex,
    db: Connection,
}

impl SearchEngine {
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> { /* ... */ }
    pub fn index_note(&self, note: &Note) -> Result<()> { /* ... */ }
    pub fn get_graph(&self) -> Result<Graph> { /* ... */ }
}
```

---

## CLI Design Patterns

### Command Structure

**Using clap:**
```rust
#[derive(Parser)]
#[command(name = "agentic-note", about = "Local-first note-taking")]
struct Cli {
    #[arg(long, global = true)]
    vault: Option<PathBuf>,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init { path: PathBuf },
    Note {
        #[command(subcommand)]
        cmd: NoteCmd,
    },
}

#[derive(Subcommand)]
enum NoteCmd {
    Create { title: String, #[arg(long)] body: Option<String> },
    List { #[arg(long)] para: Option<String> },
}
```

### Output Formatting

**Dual Mode (JSON + Human):**
```rust
pub enum OutputFormat {
    Json,
    Human,
}

pub fn print_note(note: &Note, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&note)?);
        }
        OutputFormat::Human => {
            println!("ID: {}", note.id);
            println!("Title: {}", note.frontmatter.title);
        }
    }
    Ok(())
}
```

---

## Testing Strategy

### Test Coverage Targets
- **Core logic**: 80%+ coverage (required)
- **Error paths**: 100% coverage
- **CLI commands**: 70%+ coverage
- **Agents**: 70%+ coverage

### Test Types

**Unit Tests:**
- Test individual functions in isolation
- Mock external dependencies
- Fast execution (<100ms each)

**Integration Tests:**
- Test multiple components together
- Use real temp files/databases
- Test error recovery scenarios

**Property-Based Tests (Future):**
- Use proptest for value generation
- Reserve for complex algorithms

### Mock & Fixture Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_vault() -> (TempDir, Vault) {
        let temp = TempDir::new().unwrap();
        let vault = Vault::open(temp.path()).unwrap();
        (temp, vault)
    }

    #[test]
    fn test_list_notes() {
        let (_temp, vault) = setup_test_vault();
        // Test implementation
    }
}
```

---

## Security Guidelines

### API Keys & Secrets
- Store in config files with 0600 permissions
- Never log API keys (use structured logging)
- Validate key format before use
- Support environment variable override

### File Permissions
- Vault root: 0755 (readable by user)
- Config file: 0600 (user only)
- Note files: 0644 (user readable)
- Database files: 0600 (user only)

### Input Validation
- Sanitize user input before DB queries
- Validate ULID format strings
- Check file paths are within vault root
- Limit query string length to prevent DOS

---

## Performance Guidelines

### Optimization Priority
1. **Correctness** — Always prioritize correct logic
2. **Readability** — Choose clear algorithms over clever ones
3. **Performance** — Optimize only when profiling shows bottleneck

### Specific Targets
- **Note creation**: <50ms per note
- **FTS indexing**: <100ms per note
- **Graph queries**: <200ms for typical queries
- **CAS snapshots**: <2s for 5k notes

### Common Patterns
```rust
// Good: pre-allocate for known size
let mut notes = Vec::with_capacity(estimate);

// Good: batch database operations
let tx = db.transaction()?;
for note in notes {
    insert_note(&tx, note)?;
}
tx.commit()?;

// Avoid: multiple round-trips
for note in notes {
    db.insert(note)?;  // N queries
}
```

---

## Documentation Standards

### README Requirements
- [ ] Quick start with examples
- [ ] Feature list
- [ ] Architecture overview
- [ ] Configuration guide
- [ ] Troubleshooting section

### API Documentation
- [ ] All public items have doc comments
- [ ] Include examples for complex functions
- [ ] Document errors in `# Errors` section
- [ ] Cross-reference related items

### Architecture Documentation
- [ ] System diagram
- [ ] Component responsibilities
- [ ] Data flow
- [ ] Dependency graph

---

## Continuous Integration

### Pre-commit Checks
```bash
cargo check                    # Compile check
cargo clippy -- -D warnings    # Linter
cargo fmt -- --check          # Format check
cargo test                     # Tests
```

### Commit Message Format
```
<type>(<scope>): <subject>

<body>

Closes #<issue_number>
```

**Types:** `feat`, `fix`, `refactor`, `test`, `docs`, `chore`

**Example:**
```
feat(agent): add zettelkasten-linker agent

Implements automatic link extraction from note bodies. Detects [[id]]
references and suggests new links based on note similarity.

Closes #42
```

---

## Dependency Management

### Workspace Dependencies
- Define all dependencies in `[workspace.dependencies]`
- Reference with `{ workspace = true }` in crates
- Ensures version consistency

### Adding Dependencies
1. Justify the dependency (cost/benefit)
2. Check for security advisories with `cargo audit`
3. Prefer crates with active maintenance
4. Avoid duplicate functionality (DRY)

### Allowed Dependencies
- See `Cargo.toml` for current list
- Core: tokio, serde, anyhow, thiserror
- Search: tantivy, rusqlite
- LLM: reqwest, serde_json
- CLI: clap, tracing, chrono

---

## Version Management

### Semantic Versioning
- **0.1.0**: Initial MVP release
- **0.x.y**: Pre-1.0, breaking changes allowed
- **1.0.0**: Stable API, semantic versioning enforced

### Release Process
1. Update version in `Cargo.toml` and `Cargo.lock`
2. Update `CHANGELOG.md` with all changes
3. Tag commit as `v0.1.0`
4. Update `README.md` with new features
5. Announce in release notes

---

## Code Review Checklist

Before submitting a PR:
- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] Error handling complete
- [ ] Documentation updated
- [ ] No secrets committed
- [ ] YAGNI/KISS/DRY principles followed
- [ ] Performance targets met
- [ ] Security guidelines followed

