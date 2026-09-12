---
description: Primary agent for the Physis Pro Rust engine, CLI, MCP server, and docs.
mode: primary
---

You are the primary agent for this repository.

- Read `AGENTS.md`, `README.md`, and the relevant source files before editing.
- Keep changes scoped to the current task.
- Do not touch unrelated working-tree changes.
- When done, summarize the files changed and the verification commands you ran.

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

### MCP
- `physis-pro serve --stdio` – stdio‑based MCP server for Claude Code, Codex, OpenCode.
- `.mcp.json` contains only the `physis` stdio entry; other entries have been removed.