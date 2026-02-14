# PKM Methods & AI Automation Opportunities
Date: 2026-02-13 | Researcher report

---

## 1. PARA Method (Tiago Forte)

**Structure:** Four top-level categories, mutually exclusive, collectively exhaustive:

| Bucket | Definition | Example |
|---|---|---|
| **Projects** | Active goals with deadline | "Launch MVP", "Write article" |
| **Areas** | Ongoing responsibilities, no end date | "Health", "Finance", "Work" |
| **Resources** | Reference material for future use | "React docs", "Recipes", "Book notes" |
| **Archives** | Inactive items from the above | Completed projects, dropped interests |

**Core rules:**
- Items flow Projects → Archives on completion
- Areas never "complete" — only expand/contract
- Actionability determines placement: Projects > Areas > Resources > Archives
- Same structure mirrored across all tools (files, notes, tasks, email)

**Workflow:**
1. Capture → Inbox (unsorted)
2. Weekly review: sort inbox into PARA buckets
3. On project completion → archive entire folder
4. On new project → create folder, pull from Resources/Areas

**AI automation opportunities:**
- **Auto-classification**: LLM reads note metadata + content → suggests PARA bucket
- **Project detection**: scan notes for deadline language ("by Friday", "launch") → auto-promote to Projects
- **Stale project detection**: no edits in 30d → flag for archival
- **Cross-tool sync**: keep PARA structure consistent across Notion/Obsidian/filesystem
- **Weekly review assistant**: generate inbox triage queue sorted by suggested bucket

---

## 2. CODE Method (Tiago Forte)

**The knowledge pipeline for "Building a Second Brain":**

### C — Capture
Take notes on anything resonant: quotes, ideas, highlights, voice memos.
- Rule: capture only what "resonates" — subjective signal of value
- Sources: web clips, Kindle highlights, meeting notes, audio transcripts

**AI:** Auto-capture from browser history, auto-highlight PDF/ebook passages, speech-to-text ingestion, web scraper agents triggered by bookmarks.

### O — Organize
File captures into PARA. Move out of inbox.
- Rule: organize by actionability, not by topic
- Key insight: organize for your future self who will *use* the note

**AI:** LLM classifies captures → suggests PARA location; auto-tags with entity extraction (people, concepts, projects); deduplication against existing notes.

### D — Distill
Progressive summarization: bold key passages → highlight bolded → executive summary.
- Layers: raw → bolded → highlighted → summary → remix
- Goal: preserve only the "most interesting" 10–20%

**AI:** Summarization per layer (extractive → abstractive); generate TL;DR; extract key claims as bullet points; identify contradictions with existing notes; generate flashcards (Anki/SRS format).

### E — Express
Turn distilled notes into output: posts, reports, code, decisions.
- "Return on investment" of PKM — output is the goal
- Intermediate packets: reusable chunks ready to compose into output

**AI:** Draft generation from note clusters; outline builder from related notes; auto-suggest "intermediate packets" relevant to current writing task; citation linker.

---

## 3. Zettelkasten Method (Niklas Luhmann)

**Core concepts:**

**Atomic notes (Zettels):** One idea per note. Self-contained, rewritable in own words. Never copy-paste — forces understanding.

**Three note types:**
- *Fleeting notes*: quick captures, temporary, processed within 24–48h
- *Literature notes*: per-source summaries in own words, minimal
- *Permanent notes*: standalone ideas that connect to existing network

**Linking:** Each note gets a unique ID. Links are bidirectional. Value emerges from connection density, not note count.

**Slip-box (Zettelkasten):** The physical/digital box. Notes gain value by being linked — an idea isolated is an idea lost.

**Index notes / Structure notes (MOCs):** Entry points into clusters of linked notes. Not hierarchical folders — associative maps.

**Workflow:**
1. Write fleeting note immediately (don't lose the thought)
2. Process daily: convert fleeting → literature or permanent
3. Link every new permanent note to ≥1 existing note
4. Never file by topic — file by connection
5. Periodically review orphan notes

**AI automation opportunities:**
- **Link suggestion**: embed all notes → cosine similarity → surface top-N candidates for linking
- **Atomization assistant**: detect multi-idea notes → suggest splits; score "atomicity" of a note
- **Orphan rescue**: find notes with 0 links → suggest connections from semantic neighborhood
- **Concept extraction**: NER + topic modeling → build concept index automatically
- **Knowledge graph**: auto-generate visual graph from wikilinks + semantic clusters
- **Fleeting-to-permanent**: LLM rewrites fleeting note into permanent note format
- **Contradiction detector**: flag notes that contradict each other semantically

---

## 4. Other Relevant Methods

### GTD (Getting Things Done — David Allen)
Capture → Clarify → Organize → Reflect → Engage.
Focus: task/action management, not knowledge.
**AI:** Auto-clarify next actions from vague captures; weekly review prompts; context-based task surfacing ("@computer" tasks when at desk).

### Evergreen Notes (Andy Matuschak)
Notes as living, evolving concepts — not snapshots. Titles as claims ("Evergreen notes should be atomic"). Dense linking. Never "finished."
**AI:** Suggest title rewrites as assertions; detect when note has grown stale vs. still relevant; suggest merges when two notes converge.

### MOCs (Maps of Content — Nick Milo)
Index notes that aggregate links to related notes by theme. Middle layer between atomic notes and top-level index.
**AI:** Auto-generate MOC drafts from semantic clusters; detect when a cluster is dense enough to warrant a new MOC; suggest MOC entry points for new notes.

### Commonplace Book
Chronological quotes/ideas collection, topic-tagged. Oldest method.
**AI:** Auto-tag on ingest; generate thematic compilations on demand; cross-reference quotes with current writing.

---

## 5. AI/LLM Automation Matrix

| Task | PARA | CODE | Zettelkasten | Tools |
|---|---|---|---|---|
| Auto-classify/route | bucket suggestion | O step | fleeting→permanent | LLM classifier |
| Summarization | project briefs | D step | literature notes | extractive+abstractive LLM |
| Link/relation discovery | cross-project deps | E step | link suggestion | embeddings + cosine sim |
| Duplicate detection | inbox triage | O step | orphan rescue | embedding dedup |
| Entity extraction | tag generation | O step | concept index | NER pipeline |
| Knowledge graph | area map | E step | slip-box graph | graph DB + embeddings |
| Flashcard generation | resource review | D step | — | LLM → Anki format |
| Stale content detection | archive trigger | — | orphan rescue | last-modified + LLM |
| Draft generation | — | E step | structure notes | RAG over note corpus |
| Progressive summarization | — | D step | — | multi-pass LLM |

**Key technical patterns:**
- **Semantic embeddings** (OpenAI/local): power link suggestion, dedup, clustering
- **RAG pipeline**: retrieve relevant notes → LLM generates/augments
- **Graph database** (Neo4j/simple adjacency list): bidirectional links + traversal
- **Streaming ingestion**: webhook/watcher on note changes → re-embed → update index
- **Agent loop**: capture trigger → classify → link suggest → user confirms → commit

---

## Unresolved Questions

1. Which note-taking tool is the target? (Obsidian, Notion, plain markdown files) — affects plugin/API approach.
2. Local LLM vs. API? Matters for privacy of personal notes and latency of real-time suggestions.
3. Should AI actions be fully automatic or require user confirmation? (Trust level affects UX design.)
4. Persistence layer for knowledge graph: SQLite + vector extension (sqlite-vec) vs. dedicated vector DB?
5. Scope: single method or hybrid (e.g., PARA structure + Zettelkasten linking)?
