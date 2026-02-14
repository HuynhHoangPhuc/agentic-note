# Agentic Architecture Patterns — Research Report
Date: 2026-02-13 | Slug: agentic-architecture-patterns

---

## 1. Google Project IDX / "Antigravity"

Google Project IDX is a browser-based, VSCode-fork hosted dev environment. As of early 2025 it integrates Gemini AI but does **not** expose a named "agent mode" comparable to Claude Code or Cursor. Its agentic features are limited to inline code generation and chat within a workspace panel.

Key architectural facts:
- **Workspace = isolated VM** (Nix-based): one workspace per project/repo
- **Single conversation thread per workspace panel** — no parallel agent spawning in the public product
- "Antigravity" appears to be an internal codename; no public architectural docs
- Agent parallelism: not publicly available; Gemini assists synchronously

Relevance rating: LOW — limited agentic patterns to extract.

---

## 2. Claude Code Agent Architecture (Anthropic)

Most mature and documented agentic system in the group.

**Core model:**
- **Orchestrator → Subagent(s)** hierarchy via `Task` tool
- Each subagent = isolated Claude instance with its own context window
- Orchestrator passes: task description, file paths, working dir, output instructions
- Subagent writes output to file or returns result; orchestrator reads and continues

**Parallelism:**
- Orchestrator spawns N subagents simultaneously by issuing multiple `Task` tool calls in one turn
- Subagents run concurrently; orchestrator awaits all before merging results
- No shared mutable state between subagents — results are files or structured text

**Context management:**
- Each agent has independent context; no shared memory
- Context passed explicitly via prompt injection (task instructions + file references)
- Hooks (`SubagentStart`) inject global context (project paths, rules, naming conventions)

**Human-in-the-loop (HITL):**
- `AskUserQuestion` tool gates dangerous operations
- Permission model: tools can be auto-approved or require approval per invocation
- Agent halts and surfaces question; resumes after user selection

**Tool execution model:**
- Bash, Read, Write, Edit, Glob, Grep — all sandboxed to CWD
- Tool calls are sequential within a single agent turn; parallelism only at agent level

---

## 3. AmpCode (Amp by Sourcegraph)

**Amp** (formerly related to Sourcegraph Cody) is an agentic coding assistant. Architecture as of 2024–2025:

- **Thread-per-task model**: each user request = a "thread" with its own isolated context
- Supports running multiple threads concurrently in the UI
- Each thread has tool use: file read/write, terminal, search
- No explicit orchestrator/subagent hierarchy documented publicly — operates as a single agent per thread with tool loops
- Context window managed via automatic file truncation and relevance ranking (similar to RAG)

Key pattern: **Thread isolation** — parallel conversations don't share state, results merged by user.

---

## 4. Windsurf Cascade (Codeium)

Cascade is Windsurf's multi-step agentic system.

**Core design:**
- **"Flow" model**: agent maintains a persistent action plan across turns (not just chat)
- Cascade operates as a single long-running agent with tool use (no subagent spawning)
- Tools: file edit, terminal, search, web fetch — executed sequentially within a flow
- **Checkpoint system**: saves state at each action, allows rollback
- **Write access gate**: explicit user confirmation before destructive file operations

**Parallelism:** None at the agent level — Cascade is single-threaded per workspace. Speed comes from large context window usage and incremental edits.

**Context management:**
- Maintains a "codebase index" (vector embeddings) for retrieval
- Injects relevant file snippets into context automatically
- Explicit "memories" system: persists cross-session facts

---

## 5. Common Agentic Architecture Patterns (Synthesized)

| Pattern | Claude Code | Amp | Cascade | IDX |
|---|---|---|---|---|
| Workspace scoping | Per-project CWD + hooks | Thread per task | Single workspace | VM per project |
| Parallel agents | Yes (Task tool) | Yes (threads) | No | No |
| HITL gates | AskUserQuestion | Confirmation prompts | Write confirmations | Basic |
| Tool execution | Sandboxed bash + file tools | File + terminal | File + terminal | Gemini inline |
| Context strategy | Explicit injection + files | RAG + truncation | Vector index + memory | Single context |
| Rollback/checkpoint | No | No | Yes | No |
| Subagent hierarchy | Yes (orchestrator→agent) | No | No | No |

### Distilled Patterns (applicable to note-taking app)

**Pattern A — Orchestrator + Parallel Workers**
One coordinator agent decomposes a task, spawns N specialized subagents (e.g., "search notes", "summarize", "link related"), merges results. Best for: complex queries across large note graphs.

**Pattern B — Thread Isolation**
Each user conversation = isolated context. No cross-thread contamination. Results explicitly merged by user. Best for: multi-topic simultaneous research sessions.

**Pattern C — Explicit Context Injection**
No implicit shared state. Each agent receives only what it needs via structured prompt (workspace path, relevant notes, task). Best for: preventing hallucination from stale context.

**Pattern D — HITL Gate on Mutations**
Read operations auto-approved; write/delete operations require explicit user confirmation. Best for: note editing, bulk operations, deletions.

**Pattern E — Persistent Memory Layer**
Separate from conversation context. Stores cross-session facts (user preferences, note taxonomy, recurring topics). Best for: personalization without polluting per-task context.

---

## Application to Note-Taking App

Recommended minimal architecture:
1. **Workspace** = one notebook/project scope (maps to directory)
2. **Orchestrator** handles user intent routing → spawns specialized agents (search, write, summarize, link)
3. **Thread isolation** for parallel conversations within same workspace
4. **HITL** on: note creation, bulk edits, deletions, cross-note refactors
5. **Memory layer** (lightweight JSON or SQLite): stores user patterns, note graph metadata — separate from agent context

---

## Unresolved Questions

1. Google "Antigravity" — no public architecture docs; may be entirely internal. Pattern claims require verification when/if released.
2. AmpCode parallel thread implementation details not publicly documented — assumed from UI behavior.
3. Windsurf Cascade checkpoint/rollback depth: unclear if full state or just file diffs.
4. Whether Claude Agent SDK (MCP) exposes inter-agent communication beyond file passing — relevant if notes app needs real-time agent collaboration.
