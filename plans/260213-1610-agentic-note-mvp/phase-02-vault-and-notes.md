# Phase 02: Vault & Notes

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 01 (core types)
- Research: [Architecture Brainstorm](../reports/brainstorm-260213-1552-agentic-note-app.md)

## Overview
- **Priority:** P1 (blocks CLI, Search, CAS, AgentSpace)
- **Status:** pending
- **Effort:** 10h
- **Description:** File I/O for .md notes, YAML frontmatter parse/serialize, markdown link extraction, PARA folder structure, note CRUD operations.

## Key Insights
- `serde_yaml` for frontmatter — parse YAML between `---` delimiters
- `pulldown-cmark` for markdown parsing — extract `[[wikilinks]]` and standard links
- PARA folders: projects/, areas/, resources/, archives/, zettelkasten/, inbox/
- Note filename: `{ulid}-{slugified-title}.md` for human readability
- `.agentic/` dir inside vault for system metadata (config, index, agent sessions)

## Requirements

**Functional:**
- Create/read/update/delete notes as .md files with YAML frontmatter
- Parse frontmatter into `FrontMatter` struct, body as raw string
- Extract wikilinks `[[target]]` and markdown links from body
- Init vault with PARA folder structure + `.agentic/` dir
- List notes with filtering by para category, tags, status
- Validate vault structure (check required folders exist)

**Non-functional:**
- Handle 10k+ notes without scanning entire vault for single-note ops
- Frontmatter round-trip: parse then serialize preserves all fields

## Architecture

```
crates/vault/src/
├── lib.rs            # pub mod re-exports, Vault struct
├── note.rs           # Note struct (frontmatter + body), CRUD ops
├── frontmatter.rs    # YAML parse/serialize between --- delimiters
├── markdown.rs       # Link extraction (wikilinks + std links)
├── para.rs           # PARA folder structure, category mapping
└── init.rs           # Vault initialization (create dirs, config)
```

## Related Code Files

**Create:**
- `crates/vault/Cargo.toml` (update stub)
- `crates/vault/src/lib.rs`
- `crates/vault/src/note.rs`
- `crates/vault/src/frontmatter.rs`
- `crates/vault/src/markdown.rs`
- `crates/vault/src/para.rs`
- `crates/vault/src/init.rs`

## Cargo.toml Dependencies
```toml
[dependencies]
agentic-note-core = { path = "../core" }
serde = { workspace = true }
serde_yaml = "0.9"
serde_json = { workspace = true }
chrono = { workspace = true }
ulid = { workspace = true }
pulldown-cmark = "0.11"
slug = "0.1"
walkdir = "2"
```

## Implementation Steps

1. **`frontmatter.rs`:**
   - `parse(raw: &str) -> Result<(FrontMatter, String)>` — split on `---`, parse YAML, return (frontmatter, body)
   - `serialize(fm: &FrontMatter, body: &str) -> String` — render `---\n{yaml}\n---\n{body}`
   - Handle missing frontmatter gracefully (create default)

2. **`markdown.rs`:**
   - `extract_wikilinks(body: &str) -> Vec<String>` — regex `\[\[([^\]]+)\]\]`
   - `extract_markdown_links(body: &str) -> Vec<String>` — use pulldown-cmark parser, collect Link events
   - `extract_all_links(body: &str) -> Vec<String>` — combine both

3. **`note.rs`:**
   - `Note { id: NoteId, frontmatter: FrontMatter, body: String, path: PathBuf }`
   - `Note::create(vault: &Path, title: &str, para: ParaCategory, body: &str, tags: Vec<String>) -> Result<Note>`
     - Generate ULID, create frontmatter, compute file path, write file
   - `Note::read(path: &Path) -> Result<Note>` — read file, parse frontmatter + body
   - `Note::update(note: &mut Note) -> Result<()>` — update modified timestamp, write file
   - `Note::delete(path: &Path) -> Result<()>` — remove file
   - `Note::filename(id: &NoteId, title: &str) -> String` — `{ulid}-{slug}.md`

4. **`para.rs`:**
   - `para_path(vault: &Path, category: ParaCategory) -> PathBuf`
   - `detect_category(path: &Path) -> Option<ParaCategory>` — from file path
   - `validate_structure(vault: &Path) -> Result<Vec<String>>` — return list of issues

5. **`init.rs`:**
   - `init_vault(path: &Path) -> Result<()>` — create PARA dirs, .agentic/, default config.toml
   - Skip existing dirs (idempotent)

6. **`lib.rs`:**
   - `Vault` struct holding root path + config
   - `Vault::open(path: &Path) -> Result<Vault>` — validate structure, load config
   - `Vault::list_notes(filter: NoteFilter) -> Result<Vec<NoteSummary>>` — walkdir + frontmatter parse
   - `NoteFilter { para: Option<ParaCategory>, tags: Option<Vec<String>>, status: Option<NoteStatus> }`
   - `NoteSummary { id, title, para, tags, modified, path }` — lightweight, no body

7. **Write unit tests** for frontmatter round-trip, link extraction, PARA path resolution

## Todo List
- [ ] Implement frontmatter parse/serialize
- [ ] Implement markdown link extraction
- [ ] Implement Note CRUD
- [ ] Implement PARA folder structure
- [ ] Implement vault init
- [ ] Implement Vault struct with list/filter
- [ ] Write unit tests

## Success Criteria
- Create note -> read it back -> identical frontmatter + body
- Wikilinks `[[target]]` correctly extracted
- `vault init` creates all PARA folders + .agentic/
- List notes filters by para category correctly
- Frontmatter YAML round-trips without data loss

## Risk Assessment
- **serde_yaml edge cases:** multiline strings, special chars in titles — test thoroughly
- **Large vault scan:** walkdir + frontmatter parse for 10k files could be slow — add `NoteSummary` that only reads first 20 lines

## Security Considerations
- Validate file paths to prevent directory traversal
- Sanitize title for filename (slug) to avoid path injection

## Next Steps
- Phase 03 (CLI) uses Vault for all commands
- Phase 04 (Search) indexes Note data
- Phase 05 (CAS) hashes Note files
- Phase 07 (AgentSpace) watches vault for changes
