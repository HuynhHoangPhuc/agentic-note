# Phase 08: Agent Core + Built-in Agents + Review Queue

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 04 (search), Phase 07 (AgentSpace engine)
- Research: [Embeddings & LLM](research/researcher-embeddings-llm-crypto.md), [AgentSpace Patterns](../reports/researcher-260213-1604-agentspace-pipeline-patterns.md)

## Overview
- **Priority:** P1 (core agentic functionality)
- **Status:** pending
- **Effort:** 10h
- **Description:** LLM provider trait + registry, tool registry, 4 built-in agents (para-classifier, zettelkasten-linker, distiller, vault-writer), SQLite review queue, CLI approve/reject, dual-mode approval gate, trust levels.

## Key Insights
- LLM provider trait: `async fn chat(messages, opts) -> String` — same shape for OpenAI/Anthropic/Gemini/Ollama
- Ollama mimics OpenAI API at `/v1/chat/completions` — reuse OpenAI provider with different base_url
- Built-in agents are AgentHandler implementations — each calls LLM with specific prompts
- Review queue = SQLite table: pending agent outputs awaiting human approval
- Trust levels (manual/review/auto) determine whether changes queue or apply immediately
- Interactive mode: each action queues for immediate review
- AgentSpace mode: pipeline stages execute, results batch-queue

## Requirements

**Functional:**
- LLM provider trait with implementations: OpenAI, Anthropic, Ollama (Gemini stretch)
- Provider registry: configure in config.toml, select active provider
- 4 built-in agents implementing AgentHandler:
  - `para-classifier`: suggest PARA category + tags from note content
  - `zettelkasten-linker`: find related notes via embeddings, suggest links
  - `distiller`: extract key ideas, generate summary
  - `vault-writer`: apply all stage outputs (move file, update frontmatter, add links)
- Review queue: store proposed changes, show diff, approve/reject
- Interactive mode: `agentic-note agent classify <note>` → show suggestion → approve
- `agentic-note review list` / `review show <id>` / `review approve <id>` / `review reject <id>` / `review approve --all`
- Trust level config per pipeline and per stage

**Non-functional:**
- LLM call timeout: 30s default (configurable)
- Review queue queries < 50ms
- Agent prompts are templates — no hardcoded strings

## Architecture

```
crates/agent/src/
├── lib.rs                  # pub mod re-exports
├── engine/                 # (from Phase 07)
├── llm/
│   ├── mod.rs              # LlmProvider trait, Message, ChatOpts
│   ├── registry.rs         # ProviderRegistry
│   ├── openai.rs           # OpenAI + Ollama provider
│   └── anthropic.rs        # Anthropic provider
├── agents/
│   ├── mod.rs              # register_builtin_agents()
│   ├── para_classifier.rs  # PARA category + tags suggestion
│   ├── zettelkasten_linker.rs  # Related notes via embeddings
│   ├── distiller.rs        # Key ideas extraction + summary
│   └── vault_writer.rs     # Apply changes to vault files
└── prompts/
    ├── mod.rs              # Prompt template loading
    ├── classify.txt        # PARA classification prompt
    ├── link.txt            # Link suggestion prompt
    └── distill.txt         # Distillation prompt

crates/review/src/
├── lib.rs                  # pub mod re-exports, ReviewQueue struct
├── queue.rs                # SQLite review queue CRUD
├── diff.rs                 # Generate human-readable diff
└── gate.rs                 # Approval gate (trust level → queue or apply)
```

## Related Code Files

**Create:**
- `crates/agent/src/llm/mod.rs`
- `crates/agent/src/llm/registry.rs`
- `crates/agent/src/llm/openai.rs`
- `crates/agent/src/llm/anthropic.rs`
- `crates/agent/src/agents/mod.rs`
- `crates/agent/src/agents/para_classifier.rs`
- `crates/agent/src/agents/zettelkasten_linker.rs`
- `crates/agent/src/agents/distiller.rs`
- `crates/agent/src/agents/vault_writer.rs`
- `crates/agent/src/prompts/mod.rs`
- `crates/agent/src/prompts/classify.txt`
- `crates/agent/src/prompts/link.txt`
- `crates/agent/src/prompts/distill.txt`
- `crates/review/Cargo.toml` (update stub)
- `crates/review/src/lib.rs`
- `crates/review/src/queue.rs`
- `crates/review/src/diff.rs`
- `crates/review/src/gate.rs`

**Modify:**
- `crates/agent/Cargo.toml` — add LLM deps
- `crates/cli/src/commands/mod.rs` — add Agent, Review subcommands
- `crates/cli/Cargo.toml` — add review dep

## Cargo.toml Dependencies (agent crate additions)
```toml
# Additional deps for agent crate
reqwest = { version = "0.12", features = ["json"] }
async-trait = "0.1"
agentic-note-search = { path = "../search" }
agentic-note-review = { path = "../review" }
```

```toml
# review crate
[dependencies]
agentic-note-core = { path = "../core" }
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
anyhow = { workspace = true }
```

## Implementation Steps

1. **LLM Provider Trait (`llm/mod.rs`):**
   ```rust
   #[async_trait]
   pub trait LlmProvider: Send + Sync {
       fn name(&self) -> &str;
       async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String>;
   }
   pub struct Message { pub role: String, pub content: String }
   pub struct ChatOpts { pub model: Option<String>, pub temperature: Option<f32>, pub max_tokens: Option<u32> }
   ```

2. **Provider Registry (`llm/registry.rs`):**
   - `ProviderRegistry { providers: HashMap<String, Arc<dyn LlmProvider>>, active: String }`
   - `register()`, `get()`, `active()` methods
   - `from_config(config: &LlmConfig) -> Result<Self>` — build from config.toml

3. **OpenAI Provider (`llm/openai.rs`):**
   - `OpenAiProvider { client: reqwest::Client, api_key: String, base_url: String }`
   - Works for OpenAI (api.openai.com) AND Ollama (localhost:11434/v1) — same API shape
   - POST `/chat/completions` with messages array
   - Parse `choices[0].message.content`

4. **Anthropic Provider (`llm/anthropic.rs`):**
   - `AnthropicProvider { client: reqwest::Client, api_key: String }`
   - POST `/v1/messages` with `x-api-key` header
   - Different JSON shape: `{model, messages, max_tokens}`
   - Parse `content[0].text`

5. **Built-in Agents:** Each implements `AgentHandler` trait (from Phase 07)

   **`para_classifier.rs`:**
   - Takes note content, sends to LLM with classification prompt
   - Returns `{ para: "projects", tags: ["mvp", "planning"], confidence: 0.9 }`

   **`zettelkasten_linker.rs`:**
   - Uses SearchEngine::search_semantic() to find similar notes
   - Sends note + candidates to LLM for link rationale
   - Returns `{ links: [{ id, title, similarity, reason }] }`

   **`distiller.rs`:**
   - Sends note to LLM with distillation prompt
   - Returns `{ summary, key_ideas: [...], atomic_notes: [...] }`

   **`vault_writer.rs`:**
   - Reads all previous stage outputs from StageContext
   - Generates proposed changes: frontmatter updates, file move, link insertions
   - Returns `ProposedChanges { frontmatter_diff, new_path, link_additions }`
   - Sends to review queue (or applies directly if trust=auto)

6. **Review Queue (`review/queue.rs`):**
   - SQLite table: `reviews(id TEXT PK, pipeline TEXT, note_id TEXT, proposed_changes JSON, status TEXT, created TEXT, resolved TEXT)`
   - Status: pending, approved, rejected
   - `ReviewQueue::open(db_path) -> Result<Self>`
   - `enqueue(pipeline, note_id, changes) -> Result<ReviewId>`
   - `list(status: Option<Status>) -> Result<Vec<ReviewItem>>`
   - `get(id) -> Result<ReviewItem>`
   - `approve(id) -> Result<ProposedChanges>` — returns changes to apply
   - `reject(id) -> Result<()>`
   - `approve_all() -> Result<Vec<ProposedChanges>>`

7. **Approval Gate (`review/gate.rs`):**
   - `fn gate(trust: TrustLevel, changes: ProposedChanges, queue: &ReviewQueue) -> Result<GateResult>`
   - `Manual` → enqueue, block until approved
   - `Review` → enqueue, return immediately (batch review later)
   - `Auto` → skip queue, return changes for immediate application
   - `GateResult { action: Apply | Queued, review_id: Option<String> }`

8. **Diff Display (`review/diff.rs`):**
   - `fn format_diff(changes: &ProposedChanges) -> String` — human-readable diff
   - Show: frontmatter changes, new file path, added links, added tags
   - Color output (termcolor or inline ANSI) for terminal

9. **CLI Commands:**
   - `agentic-note agent classify <note-id>` — interactive classification
   - `agentic-note agent link-suggest <note-id>` — interactive link suggestion
   - `agentic-note review list [--json]`
   - `agentic-note review show <id>`
   - `agentic-note review approve <id> | --all`
   - `agentic-note review reject <id>`

10. **Wire up agents to AgentSpace executor:** register all 4 agents in `register_builtin_agents()`

## Todo List
- [ ] Define LlmProvider trait and Message/ChatOpts types
- [ ] Implement ProviderRegistry
- [ ] Implement OpenAI provider (+ Ollama reuse)
- [ ] Implement Anthropic provider
- [ ] Implement para-classifier agent
- [ ] Implement zettelkasten-linker agent
- [ ] Implement distiller agent
- [ ] Implement vault-writer agent
- [ ] Implement SQLite review queue
- [ ] Implement approval gate with trust levels
- [ ] Implement diff display
- [ ] Add agent/review CLI commands
- [ ] Wire agents to AgentSpace executor
- [ ] Write tests for review queue CRUD

## Success Criteria
- `agent classify <note>` returns PARA suggestion from LLM
- `agent link-suggest <note>` returns related notes with rationale
- `review list` shows pending items from agent runs
- `review approve <id>` applies proposed changes to vault
- Trust level auto: changes applied immediately without queuing
- Pipeline run with all 4 stages: classify → link → distill → write queues for review

## Risk Assessment
- **LLM API key management:** keys in config.toml — warn about file permissions
<!-- Updated: Validation Session 1 - JSON mode + schema validation for LLM output -->
- **LLM response parsing:** use JSON mode (OpenAI `response_format: json_object`, Anthropic equivalent). Validate response against expected JSON schema. Retry up to 2x on parse failure with error feedback in prompt.
- **Prompt engineering:** classification/linking quality depends on prompts — iterate post-MVP
- **Rate limits:** OpenAI/Anthropic rate limits could block pipeline — add retry with exponential backoff

## Security Considerations
- API keys stored in config.toml (0600 permissions recommended)
- LLM calls send note content to external service — document in privacy notice
- Ollama (local) avoids data exfiltration — recommend as default for sensitive vaults
- Review queue prevents unauthorized vault modifications

## Next Steps
- Phase 09 (MCP) exposes agent + review tools
- Future: custom agent plugins, streaming responses, tool use
