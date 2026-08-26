# MCP Registry Submission — physis-pro

> Prepared 2026-08-26 for the official registry
> (`registry.modelcontextprotocol.io`) and downstream directories (Smithery,
> LobeHub, PulseMCP, Glama). Validate field names against the live registry
> schema at submission time — the manifest format has churned; the *content*
> below is stable.

## Identity

| Field | Value |
|-------|-------|
| Name | `io.github.ilPez00/physis-pro` |
| Display name | Physis Pro |
| Version | match crates.io (`0.1.10` at writing) |
| Repository | https://github.com/ilPez00/physis-pro |
| Install | `cargo install physis_pro` |

## One-line descriptions

- Registry (≤160 chars): **Semiotic memory for AI agents — classify anything against a 70-cell meaning grid, query why-made history, track coherence over time.**
- Longer: Physis Pro gives an LLM persistent meaning: every input becomes a vector in one embedding space, classified into DOMAIN×MODE cells with per-client interpretants. A causal ledger records where/when/why each artifact exists; a dream loop replays failures into corrective proposals. HTTP API + MCP stdio transport.

## Transport & launch

```
transport: stdio
command:   physis-pro
args:      ["serve", "--stdio"]
```

Notes:
- The stdio path is dispatched **before** the licence gate specifically so the
  handshake answers inside client timeouts — no licence needed to connect.
- Tool surface (Phase 10c A1): 23 tools — classify/embed/pack/goals/PDCA/
  dreams/gantt/wiki plus the memory family added 2026-08-26: `remember`,
  `recall` (write/read semiotic memory over the shared sled store), `why`,
  `serve`, `causes` (causal ledger). Agents get the full pitch, not just
  classification.

## Environment

None required to start. Optional:
- `PHYSIS_DATA_DIR` — persistence root (sled)
- Provider keys (`PHYSIS_*`) enable grounded synthesis in `/api/v1/ask`

## Submission order (per docs/business/ADOPTION.md)

1. Official registry — others index its metadata
2. Smithery (CLI installer)
3. LobeHub → PulseMCP → Glama → mcp.directory
4. awesome-mcp-servers PR (needs ≥10 stars on some lists)

## Prerequisite commands (run as the repo owner)

```bash
gh auth refresh -h github.com -s repo   # once; the CI token lacks repo scope
gh repo edit ilPez00/physis-pro \
  --add-topic mcp-server --add-topic mcp --add-topic ai-agents \
  --add-topic semantic-search --add-topic knowledge-graph \
  --add-topic ontology --add-topic rust --add-topic industrial-automation
```
