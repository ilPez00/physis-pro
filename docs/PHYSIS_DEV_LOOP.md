# Developing physis with physis (dogfood loop)

The engine's own dev loop runs on the engine: outcome memory (`note` /
`node-search`, twins of the MCP `physis_remember` / `physis_recall` tools)
plus the standard gates. No server needed — everything hits the shared sled
store (`PHYSIS_DATA_DIR`, default `.physis/`).

## The loop

```
just recall "<task>"          # 1. what did we already try? (verdicts ride along)
... work ...                  # 2. pack/find/classify as needed
just dev-loop "<task>"        # 3. gate + verdict-stamped note (-1 RED / +1 GREEN)
```

`dev-loop` defaults to `cargo check --all-targets`; pass any gate after `--`:

```bash
./scripts/physis-dev-loop.sh "fix clippy" -- cargo clippy --all-targets -- -D warnings
```

Failure never fails silent: the script prints the gate tail (expected vs
actual) and exits with the gate's code. Only **notes** are written —
never `fail`/`pass`: the quality tracker's penalties/boosts are about
classification output, and dev-gate noise must not pollute that signal.

## Agent attachment (MCP)

`.mcp.json` at the repo root spawns `physis-pro serve --stdio` (feature
`mcp`) with `PHYSIS_SCAN_ROOT=.`, so any MCP client gets all 30 tools
against this repo. Agent session protocol:

1. **Start**: `physis_recall` the task — a top hit with `verdict: -1` means
   "already tried, already failed", read it first.
2. **Orient**: `physis_pack` (bounded context) / `physis_find` (answer inside
   a file) / `physis_workflow` (who did what where, docs coverage).
3. **During**: `physis_classify` ambiguous changes; `physis_route` when the
   change should trigger a harness action.
4. **End**: `physis_remember` the outcome with a verdict; `physis_serve` to
   link artifacts to goals; `physis_themes` for "what do we keep failing at".
5. **Ontology gaps**: `physis_discover` over dev logs drafts new domains
   (`EXTRA_ONTOLOGIES`) — how the grid grows from its own work.

CLI equivalents for scripts/harness: `note` (= remember), `node-search`
(= recall), `pack`, `find`, `workflow`, `where` (= ledger facets),
`fail`/`pass`/`quality` (classification feedback only).
