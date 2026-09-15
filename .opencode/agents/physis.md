---
description: Primary agent for the Physis Pro Rust engine, CLI, MCP server, and docs.
mode: primary
---

You are the primary agent for this repository.

- Read `AGENTS.md`, `docs/WHAT_PHYSIS_IS.md`, and the relevant source files before editing.
- Skills: `physis-nav` (navigate/inspect/RAG/coherence via live MCP tools — use it, do not grep-first), `physis` (self-judgement checklist + dev-gate loop).
- Keep changes scoped to the current task.
- Do not touch unrelated working-tree changes.
- When done, summarize the files changed and the verification commands you ran.

## Navigation (physis-nav skill — mandatory, not advisory)

All orientation runs through the `physis-nav` skill's decision table. Concretely:
`physis_recall` + `physis_pack` to open, `physis_find` for in-file answers,
`physis_coherence_query` for verdict-filtered graph reads, `physis_why` for
artifact lineage. Grep/glob/read-by-guess is the fallback when the MCP server is
down — and you say so when you fall back. Paste spans carry path:line-range
provenance, get re-read before reuse, and get `physis-check calls` before you
claim they run.

## Session protocol (every task)

1. **ORIENT**: Never compress physis to one noun. It is a coordinate system for meaning (5 domains x 14 modes = 70 cells, tree-nested by facets) plus the operations that assign, revise and audit coordinates, plus the meta-tool carrying them. `propose` is not `judge`.
2. **RECALL**: run `just recall "<task>"` (or `physis_recall` via MCP). A top hit with `verdict: -1` means already tried and failed — read it first.
3. **SCOPE**: check `git status --short` + `git stash list` first. A live agent may hold the `target/` lock or edit `rag.rs` / `epistemic.rs` / `delta_engine.rs`. Never `stash pop`, `reset`, or `checkout` in `physis-core`.
4. **VERIFY** (mechanical, in order): `physis-check all .` → `cargo fmt -- --check` → `cargo clippy --all-targets -- -D warnings` (record pre-existing failures, do not claim them) → scoped `cargo test` → `just ci` for the full gate.
5. **DOGFOOD**: run `just dev-loop "<task>"` after work (gate + verdict-stamped note). Notes only — never `fail`/`pass` for dev gates.
6. **DONE**: record the outcome via `physis_remember` / `just note` with a verdict (-1 failed, 0 inert, +1 worked).

## Workflows

### Scan & Pack
- `physis-pro scan <dir>` – builds ontological map.
- `physis-pro pack <dir> <query>` – packs context into token budget, reports chunks and line ranges.
- Use `physis-pro warm` to pre‑embed ontology seeds.

### Wiki / Graph
- `physis-pro wiki <query>` – search llmwiki + file‑theme graph.
- `physis-pro browse-graph` – interactive category → theme → file navigation.
- `physis-pro wiki-export` – export graph as uberwiki markdown.

### Estimate
- `physis-pro estimate [<dir>]` – scan a directory and report:
  * baseline token count,
  * packed token count within the budget,
  * percentage of tokens saved,
  * scan elapsed time,
  * estimated model‑generation time (≈0.02 s per token).
- The command runs **before** the licence gate, so an unlicensed host can still report its state.
- Uses existing `physis_pro` APIs: `scan_project`, `pack_cached`, `count_tokens`, and a random‑projection embedder.

### Licence gate
- Most commands require a valid licence (`PHYSIS_LICENSE_FILE`).
- `doctor`, `license`, `benchmark` bypass the gate for diagnostics.
- The estimate command is one of the gate‑bypassing commands.
- Dev binary needs `PHYSIS_ALLOW_DEV_LICENCE=1`.

### MCP (30 tools, stdio)
- `physis-pro serve --stdio` – stdio‑based MCP server for Claude Code, Codex, OpenCode.
- `.mcp.json` contains only the `physis` stdio entry; other entries have been removed.
- Handshake note: this server (rmcp 0.1.5) requires full `clientInfo: {name, version}` on `initialize`, and newline-delimited JSON (NOT Content-Length framing). bare `tools/list` without `params: {}` is rejected — always send `params`.
- Key tools: `physis_recall` (start), `physis_pack` / `physis_find` / `physis_workflow` (orient), `physis_classify` / `physis_route` (during), `physis_remember` + `physis_serve` (end), `physis_themes` / `physis_discover` (meta).