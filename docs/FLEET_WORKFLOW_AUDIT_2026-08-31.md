# Fleet Workflow Audit — 2026-08-31

Results of the fleet-wide test campaign: the physis workflow scanner run across
every top-level directory and drive mount on every reachable machine, plus the
cargo feature gate and the agentic-flow probes. Tooling:
`scripts/fleet-workflow-scan.sh`. Raw per-root JSON reports live in
`~/physis-fleet-reports/{machine}_{root}.json`.

## Machines

| Machine | Status | Roots scanned | Docs | Agent docs | Human docs | Both |
|---------|--------|---------------|------|-----------|------------|------|
| pansa   | scanned | 76  | 92,661  | 155  | 9,056 | 7   |
| omo     | scanned | 117 | 352,958 | 1,835| 15,301| 50  |
| feather | scanned | 45  | 58,084  | 2,591| 5,613 | 4   |
| air     | unreachable (tailscale timeout) | — | — | — | — | — |
| **Fleet** | | **238** | **503,703** | **4,581** | **29,970** | **61** |

## Harness documentation found (fleet total)

| Harness | Agent docs | Where |
|---------|-----------|-------|
| autoresearch | 90 | `.autoresearch/` on all machines |
| agents       | 2,664 | feather's `.agents/` bulk (2,513) |
| opencode     | 21 | `.opencode/`, praxis_webapp |
| claude       | 12 | `.claude/settings.local.json` |
| commandcode  | 4 | `.commandcode/` |
| cursor       | 4 | omo `.cursor/` |
| gemini       | 2 | omo |
| codex        | 2 | `.codex/` |

## Cargo feature gate

On pansa (the merged tree): `cargo check --features full` clean, full test
suite green — **811 lib tests + 80 http_surface + 12 integration suites, 0
failed** (incl. omo's Phase 15 licensing, Phase 16 I1–I10 event stream /
typed ontology / audit log, and the new workflow scanner tests).

## Agentic-flow probes

- **MCP stdio handshake** (`physis-pro serve --stdio`): 30 tools listed,
  including `physis_workflow`, `physis_scan`, `physis_pack`, `physis_find`,
  `physis_recall`. Verified on pansa and feather.
- **`physis_workflow` over MCP**: returns the audit report (total, by_audience,
  by_harness, per-doc rows) for a target path.
- **`decisions-mine` over `~/.claude/projects`**: parsed Claude Code session
  transcripts on all machines without panicking.

## Workflow scanner (new in this change)

`src/workflow.rs` + `src/web/workflow.rs` + CLI `physis-pro workflow` + MCP
`physis_workflow` + `POST /api/v1/workflow/scan` + ledger/event recording.

- Walks harness dirs (`.claude`, `.opencode`, `.codex`, `.hermes`, …) the base
  scan skips; classifies each doc by audience (agent / human / both / other).
- Stamps timestamps + metadata; attributes git authors via one `git log`
  subprocess (`parse_git_attribution`, tested pure).
- Projects onto `OperationalKind::Workflow` events → shared timeline/ledger.
- 9 unit tests + 1 http_surface e2e; licence slug `workflow` on Core+.