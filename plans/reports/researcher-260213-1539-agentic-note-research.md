# Agentic Note-Taking: Technical Research Report
**Date:** 2026-02-13 | **Slug:** agentic-note-research

---

## 1. Google "Antigravity" Agent Mode

No public product named "Google Antigravity agent mode" found in training data (Jan 2025 cutoff). Likely an internal codename, post-cutoff release, or misremembered term. Closest verified concepts:

### Google ADK (Agent Development Kit) - April 2025
- Framework for building multi-agent systems on Gemini
- Key model: **orchestrator + sub-agent hierarchy** with explicit tool delegation
- Supports "human-in-the-loop" via interrupt/approval steps
- Agent-to-agent communication via `AgentTool` (one agent calls another as a tool)
- Safety: each agent has scoped permissions; escalation requires human approval

### Project Astra / Gemini Agent Mode
- Ambient AI concept: always-listening, context-aware agent with persistent memory
- "Proactive" trigger model: agent acts when it detects relevant events, not just when asked
- Key principle: **low-interruption autonomy** — agent proceeds unless uncertain, then asks
- Memory model: episodic (session), semantic (facts), procedural (how-to)

### Inferred "Antigravity" Principles (if it exists)
Likely refers to removing friction ("gravity") from human-AI workflows:
- Agent acts without needing explicit commands
- Human sets intent once; agent executes + reports
- Interruption only on ambiguity or risk
- Correction via natural language, not re-configuration

---

## 2. Agentic Note-Taking Tools — State of the Art

### Mem.ai
- **Model:** fully autonomous organization — no folders, agent auto-tags/links
- **Works well:** zero-friction capture, smart search, automatic cross-referencing
- **Doesn't work:** opaque organization logic, hard to trust/verify agent decisions
- **Pattern:** reactive ingestion + background semantic indexing

### Notion AI
- **Model:** embedded LLM in editor — on-demand summarize, draft, translate
- **Works well:** in-context assistance, familiar UX, low adoption friction
- **Doesn't work:** not truly agentic — requires explicit invocation every time
- **Pattern:** human-initiated, single-turn assistance

### Reflect
- **Model:** linked thought graph + AI daily review
- **Works well:** structured bi-directional linking, daily summaries push
- **Doesn't work:** limited automation depth, mostly reactive
- **Pattern:** scheduled batch processing + human review

### Tana
- **Model:** supertag-based semantic structure + AI "commands"
- **Works well:** explicit schema lets AI act predictably on structured data
- **Doesn't work:** steep learning curve to define schemas upfront
- **Pattern:** schema-first — human defines structure, AI fills/queries it

### Obsidian + AI Plugins (Copilot, Smart Connections)
- **Model:** local-first, plugin-driven, fully extensible
- **Works well:** privacy, customizability, open ecosystem
- **Doesn't work:** fragmented experience, requires manual orchestration
- **Pattern:** tool composition — human orchestrates multiple plugins

### Key Takeaway
No tool has cracked fully autonomous note management that users trust. The gap: **auditability + reversibility of agent actions** on personal knowledge.

---

## 3. Human-AI Collaboration Patterns for Knowledge Management

### Autonomy Spectrum
```
Level 0: No AI
Level 1: On-demand (human asks → AI responds) — Notion AI, ChatGPT
Level 2: Triggered (human captures → AI processes background) — Mem.ai
Level 3: Proactive (AI suggests/acts unprompted) — Project Astra concept
Level 4: Ambient (AI continuously monitors + maintains) — not production-ready
```

### Most Effective Patterns (2024-2025)

**1. Capture-Now, Process-Later**
- Human captures raw thought quickly (no structure required)
- Agent processes asynchronously: tags, links, summarizes, extracts tasks
- Human reviews diffs/suggestions, approves or dismisses
- Best for: high-volume note-takers

**2. Human-in-the-Loop with Explicit Checkpoints**
- Agent stages work in a review queue
- Human approves before writes/moves/deletes
- Risk-tiered: reads are autonomous, writes require confirmation
- Best for: personal knowledge bases where trust matters

**3. Schema-Guided Autonomy**
- Human defines templates/schemas upfront (note types, required fields)
- Agent has predictable, bounded action space
- Reduces "what did the AI do?" anxiety
- Best for: structured domains (projects, meetings, research)

**4. Ambient + Interrupt Model**
- Agent runs silently, surfaces only high-confidence insights
- Threshold-based: only interrupts if confidence > X or action is irreversible
- Human can query agent's "pending" queue anytime
- Best for: low-friction daily workflows

### Anti-patterns
- **Over-proactive agents**: constant suggestions create notification fatigue
- **Black-box rewrites**: agent changes content without showing what changed
- **No undo**: agents must support revert; trust depends on reversibility
- **Single-step confirmation**: better to batch low-risk actions for one approval

---

## 4. CLI-First Agent Interfaces

### MCP (Model Context Protocol)
- Anthropic-originated open standard (Nov 2024), now widely adopted
- Architecture: **Host ↔ Client ↔ Server** — LLM host connects to MCP servers that expose tools/resources
- CLI apps expose themselves as MCP servers → LLM can call them programmatically
- Transport: stdio (local) or SSE/HTTP (remote)
- Tool schema: JSON Schema for inputs/outputs — agent knows what to pass and what to expect

### CLI Design for Dual Human+Agent Use

**Principle: Machine-readable output by default, human-readable optionally**
```bash
# Agent-friendly
note add "content" --json           # returns {id, title, tags, created_at}
note search "query" --json --limit 10

# Human-friendly (same commands)
note add "content"                  # returns formatted table/text
note search "query"                 # returns pretty list
```

**Key patterns:**
1. **Structured exit codes** — 0=success, 1=not found, 2=validation error (agent branches on these)
2. **Idempotent operations** — agent can retry safely; use `--upsert` flags
3. **Dry-run mode** — `--dry-run` lets agent preview actions before committing
4. **JSON output flag** — every command supports `--json`; agent parses reliably
5. **Resource identifiers** — every entity has stable ID (UUID); agent references by ID not name
6. **Action log / audit trail** — agent can query what it did: `note history --agent`

### MCP Server Design for Note App
```
Tools to expose:
  - note_create(content, tags?, title?)
  - note_search(query, limit?, filters?)
  - note_update(id, content?, tags?, append?)
  - note_delete(id)         ← require confirmation flag
  - note_link(id_a, id_b, relation?)
  - note_summarize(id | query)

Resources to expose:
  - notes://all             ← paginated list
  - notes://{id}            ← single note
  - notes://tags            ← tag index
  - notes://graph           ← link graph JSON

Prompts to expose:
  - daily-review            ← pre-built prompt for morning review
  - extract-tasks           ← extract action items from recent notes
```

### Key Design Insight
The same CLI that humans use in terminal becomes an MCP server: agent calls `note search` exactly as a human would, but gets JSON back. **No separate API needed** — CLI IS the interface for both.

---

## Synthesized Recommendations for Agentic-Note

1. **CLI as first-class MCP server** — expose all commands as MCP tools with `--json` flag
2. **Autonomy level 2 as default** — agent processes captures in background, human reviews diffs
3. **Explicit approval for destructive ops** — deletes/overwrites go to review queue
4. **Schema-light approach** — auto-infer structure but allow human override; avoid mandatory schemas
5. **Audit trail built-in** — every agent action logged with revert capability
6. **Proactive but quiet** — agent batches suggestions, surfaces once per session not per action

---

## Unresolved Questions

1. "Google Antigravity" — exact meaning unknown; needs clarification from user or post-Jan-2025 source
2. What is the primary user workflow for agentic-note? (capture-heavy vs query-heavy vs review-heavy)
3. Local-first vs cloud sync — affects agent processing model (sync vs async, privacy constraints)
4. Desired autonomy level — how much should agent act without asking?
5. Target shell environment — affects MCP transport choice (stdio vs HTTP)
