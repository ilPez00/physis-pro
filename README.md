# Physis Pro

**A Rust runtime engine for ontological classification, coherence tracking, and predictive ("dream") planning.**

Physis is the reasoning core of the Ayu / Praxis ecosystem. It reduces any input —
text, image, video, audio, a code log, a human action — to a vector in one shared
embedding space, classifies it against a 70-cell semiotic grid, and tracks each
thing's *coherence* (does it work? +1 / 0 / −1). A background "dream" loop replays
the failures to propose corrective actions. Beyond meaning, every ingested file
carries a **space·time·cause address** — where it lives, when it matters, why it
exists — recorded in an append-only causal ledger you query in plain facets
(`physis-pro where …`, `POST /api/v1/files/query`).

It runs as:
- a **CLI** (`physis-pro`) for scanning, querying, and one-off dreams,
- an **HTTP server** (`physis-pro-web`, default `127.0.0.1:19876`) that Praxis and
  Aura clients call,
- an **MCP server** (feature `mcp`) for agent tool-use,
- a **TUI** and a **voice** binary for live ontological mapping.

> Crate: `physis_pro` v0.1.0 · edition 2021 · the Praxis backend talks to it through
> `src/lib/physisProClient.ts`.

## License

`physis-core` is released under the [MIT License](LICENSE).

`physis_pro` (this crate) is dual-licensed:
- **MIT** — for the open-source crate published on [crates.io](https://crates.io/crates/physis_pro).
- **Proprietary EULA** — for commercial deployments. See `LICENSE.txt` for the full
  commercial terms, including the licence-key requirement.

The `physis-core` sub-crate remains MIT-only. When using `physis_pro` from crates.io,
the MIT licence applies to the crate API; the runtime still enforces the commercial
licence gate when `PHYSIS_LICENSE` is not set.

---

## Repository layout — one repo per project

This repository is the **only** working copy of the Physis engine.

| Piece | Where |
|---|---|
| Engine, CLI, HTTP/MCP surface, licence gate | this repository |
| `physis-core` (MIT, the lean embed/classify/quality core) | git submodule at `physis-core/`, repo [`physis-core`](https://github.com/ilPez00/physis-core) |

**Do not clone this repo per deployment node.** Three such clones existed on omo
(`~/dev/air-ts`, `~/dev/pansa-ts`, `~/dev/physis-global`), named after the machines
they were meant to run on. They drifted three development phases apart, they read
as separate projects in any directory listing, and `pansa-ts` accumulated two
commits that were reachable from no remote branch and were nearly lost — they are
now on `node/pansa-deploy`. The `physis-global` repository is archived upstream.

If you genuinely need a second checkout, use a worktree rather than a clone, so
there is one object store and one set of refs:

```bash
git worktree add ../physis-<node> <branch>
```

**`.physis-vendor/` must never be committed.** It holds the Ed25519 licence
signing key. A copy is currently tracked in this repository's history (introduced
by `745a3d5`) and byte-identical material had propagated into the `air-ts` clone;
removing it needs `git filter-repo --path .physis-vendor --invert-paths`, a
force-push, and a re-clone of every working copy. The ignore rule is in
`node/pansa-deploy`.

---

## Core model — three functions over one state

```
                         ┌──────────────────────────┐
                         │   PRAXIS / AURA clients   │
                         │  classify · pdca · dream  │
                         └───────────┬──────────────┘
                                     │ HTTP / MCP
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          PHYSIS (runtime core)                            │
│                                                                           │
│   UBERWIKI (state):  DynamicVectorTrie + CoherenceNode (+1.0 / 0 / -1.0)  │
│                                    │                                       │
│            ┌───────────────────────┴───────────────────────┐             │
│            ▼                                                ▼             │
│   COMPRESSION (waking):                       DREAM (rest/async):         │
│   strip noise from raw input,                 replay Inert/Failure nodes, │
│   keep minimal causal tokens                  simulate fixes, recalibrate │
│                                               the coherence index         │
└──────────────────────────────────────────────────────────────────────────┘
```

### 1. UberWiki — the state
A vector store of causal nodes. Each node carries a **coherence weight**: stable
flows are `+1.0`, frictions/failures are `-1.0`. Backed by a `DynamicVectorTrie`
of `CoherenceNode`s.

### 2. Compression — the waking/input phase
Strips raw input (logs, commands, telemetry, prose) of identity and redundancy,
leaving the minimal causal tokens before they reach the host AI. Keeps the working
context clean and cheap. `filter_context()`, `compress_logs()`.

### 3. Dream — the rest/async phase
Runs in the background. Takes accumulated Inert (`0.0`) and Failure (`-1.0`)
fragments, simulates the missing causal link, and emits corrective **Effective
Actions** while recalibrating the global coherence index. `dream()`.

> Waking-compression and rest-dream are the two phases of one metabolism —
> discrimination (separate signal from noise) and integration (replay,
> recombine, synthesise). `docs/EMPEDOCLES_DYAD.md` develops this as the
> Empedoclean dyad (Strife/Neikos vs Love/Philotes) and tests it.

### Coherence ontology — two axes

A node carries **two** coherence numbers, and they answer different questions:

| Axis | Field | Range | Who sets it | Question |
|------|-------|-------|-------------|----------|
| **Derived** | `coherence_score` | `0 … 1` | the engine (mean cosine to the 5 nearest neighbours) | *how typical is this?* |
| **Asserted** | `asserted` | `-1 … 1`, `None` until judged | the caller | *did it work?* |

Only the asserted axis can express failure — derived density is clamped at `>= 0`
by construction. Keeping them separate is deliberate: a node can be perfectly
typical (density `1.0`) and still be a reported disaster (`-1.0`).

The three asserted states apply identically to machine functions and human behaviour:

| State   | Weight | Machine                                  | Human                                      |
|---------|:------:|------------------------------------------|--------------------------------------------|
| Success |  +1.0  | Compiles + functional test passes        | Action completed with a real, detected effect |
| Inert   |   0.0  | Compiles but produces no expected effect | Task done without cognitive progress       |
| Failure |  −1.0  | Compiler error / refuted pattern         | Self-imposed rule violated (e.g. diet slip)|

```rust
// A human action is judged with the same logic as a software function.
let id = core.register_node_from_text("ate cake at midnight", &embedder);
core.assert_coherence(&id, CoherenceRating::Failure.weight());   // -1.0
core.assert_coherence(&study_id, CoherenceRating::Inert.weight());  // 0.0

core.asserted_index();     // mean verdict over JUDGED nodes only (None if none)
core.asserted_failures();  // what to replay, worst first
core.coherence_index();    // unrelated: mean derived density over every node
```

`dream()` replays a reported failure however dense its neighbourhood, and skips a
reported success however isolated it is — a caller's verdict outranks a guess
drawn from geometry. Nodes nobody has judged fall back to the density proxy.

When a new categorization conflicts with an established Success node, the engine
suspends it and returns a **Constructive Refutation** payload for the PDCA loop to
recalibrate, instead of silently overwriting state.

---

## HTTP API

Default bind `127.0.0.1:19876` (`PHYSIS_PORT`, `PHYSIS_HOST`). All `/api/v1/*`
routes below the line are **protected** (see [Auth](#auth)); `/health` and
`/api/v1/embedder` are public for healthchecks/ops.

**Per-user state** is selected by the `X-Physis-User: <id>` header. Each id gets an
isolated `RuntimeState` (its own goals / PDCA / dreams / coherence). A missing or
`_default` id routes to the shared global tenant that the holarchy threads drive.
Stateless routes (classify, embed, semiotic, ontology) ignore the header.

| Method | Route | Tenant-aware | Purpose |
|--------|-------|:---:|---------|
| GET  | `/health` | — | Liveness (public) |
| GET  | `/api/v1/embedder` | — | Active embedder provenance (public) |
| POST | `/api/v1/classify` | — | Embed text (or base64 `image`) → ranked `{domain, mode, score, entries}[]` |
| POST | `/api/v1/discover` | — | Ontology-gap discovery over `{texts[]}` → coverage report + draft ontology JSON for the texts the grid covers poorly |
| POST | `/api/v1/embed` | — | Raw embedding vector + dimension (text or image) |
| POST | `/api/v1/embed/video` | — | ffmpeg-sampled clip vector → `{embedding, dim, frames}` |
| GET  | `/api/v1/ontology/list` | — | Domain counts per loaded ontology |
| GET  | `/api/v1/ontology/studio` | — | Every authored entry with its source, cell, hints, facets, and whether classification can reach it |
| GET  | `/api/v1/ontology/meaning` | — | Loaded overlays, contested names, and ranked salience; `?name=X` returns what `X` means in every context that defines it |
| GET  | `/api/v1/semiotic/grid` | — | 70-cell DOMAIN×MODE grid with entries |
| GET  | `/api/v1/semiotic/triangle` | — | Peircean triangle (Mermaid) |
| GET  | `/api/v1/semiotic/square` | — | Greimas square (Mermaid) |
| GET  | `/api/v1/semiotic/heatmap` | — | Activation heatmap table + matrix |
| POST | `/api/v1/category/diagram` | — | Custom category diagram (objects + morphisms) |
| GET  | `/api/v1/coherence/snapshot` | ✓ | Current coherence index + nodes |
| POST | `/api/v1/coherence/register` | ✓ | Register a node `{input, coherence_score?}` — `input` is embedded **and** kept as the label; optional `coherence_score` (−1…1) is the asserted verdict. Deduped by label, so re-posting the same `input` returns the same id and **updates** its verdict |
| POST | `/api/v1/coherence/edit` | ✓ | **Exchange-parts edit** `{id, label, cell?}` — swap content while preserving structure (id, verdict, edges); `cell: {domain, mode}` pins the semiotic cell (auto re-derivation otherwise). Returns `before_cell`/`auto_cell`/`pinned_cell` |
| POST | `/api/v1/coherence/delete` | ✓ | Delete a node + every reference (dedupe index, holarchy node/edges, sled row) `{id}` |
| POST | `/api/v1/search/nodes` | ✓ | **Fixed-token search** over coherence + goal nodes `{query, token_budget?, top_k?}` — response `total_tokens ≤ token_budget` (same contract as `/rag/retrieve`) |
| POST | `/api/v1/ask` | ✓ | **Plain-English graph query**: question → embedder-consistent retrieval (token-packed) → grounded synthesis via the LLM cascade with `[n]` citations. Never errors for lack of a provider — degrades to ranked matches + a `note` |
| GET  | `/api/v1/quality` | global | **Quality feedback ledger**: learned per-cell penalties/boosts + per-failure records (newest first, embeddings stripped) with open/resolved counts and mean severity |
| GET  | `/api/v1/communities` | ✓ | Label-propagation communities over the tenant's labelled coherence graph, largest first |
| GET  | `/api/v1/pdca/plan` | ✓ | Current Plan-Do-Check-Act plan |
| POST | `/api/v1/pdca/act` | ✓ | Apply an action `{action, goal_id?}` |
| GET  | `/api/v1/pdca/stats` | ✓ | PDCA cycle stats |
| POST | `/api/v1/oee/machine` | plant | Register a machine `{machine_id, shift_length_secs, ideal_cycle_time_secs}` |
| POST | `/api/v1/oee/event` | plant | Record a shop-floor event: `{kind:"stop", machine_id, reason, duration_secs, note?}` (reason ∈ six big losses), `{kind:"production", machine_id, good, scrap?}`, or `{kind:"start_shift", machine_id}` |
| GET  | `/api/v1/oee/:machine` | plant | OEE report (A×P×Q + six-big-losses Pareto) for one machine |
| GET  | `/api/v1/oee/plant` | plant | Planned-time-weighted plant OEE rollup + merged Pareto |
| POST | `/api/v1/maintenance/observe` | plant | Fold a healthy reading into a machine's learned baseline |
| POST | `/api/v1/maintenance/check` | plant | Score a reading (σ from baseline) + optional RUL; anomalies auto-register as coherence nodes |
| GET  | `/api/v1/maintenance/:machine` | plant | Health baseline summary for one machine |
| GET  | `/api/v1/report/shift` | plant | Italian shift report (Markdown, or HTML via `?format=html`) from the live OEE board + graded dream countermeasures |
| POST | `/api/v1/report/shift` | — | Shift report from a supplied `ShiftReportInput` (OEE + anomalies + RUL + countermeasures); stateless, `?format=html` for HTML |
| POST | `/api/v1/config/reload` | ✓ | Re-read `config.json`; apply PDCA knobs + scan_root live (embedder/embed_dim need restart — dim change corrupts vectors) |
| POST | `/api/v1/goals` | ✓ | Ingest a prompt as a goal → `{goal:{id}}` |
| POST | `/api/v1/dream/generate` | ✓ | Generate dreams `{count, force, embeddings?}` → `{id, kind, proposal, grade}`; vectors only with `embeddings:true` |
| POST | `/api/v1/dream/replay` | ✓ | Nodes the dream loop is replaying, **named**: `{node_id, label, asserted, density, outcome, prevented_failure}` — reported failures first, unjudged nodes by sparse neighbourhood |
| POST | `/api/v1/dream/evaluate` | ✓ | Grade a dream `{id, grade}` (0.0–1.0) |
| GET  | `/api/v1/stats` | ✓ | Engine statistics |
| POST | `/api/v1/context/filter` | ✓ | Compression: filter a raw input |
| POST | `/api/v1/compress/logs` | ✓ | Compression: logs → dense causal rules |
| POST | `/api/v1/reconstruct` | ✓ | Nearest-neighbour reconstruction + LLM interpretation |
| POST | `/api/v1/translate` | ✓ | Linguistic routing (Wenyan / Pirahã / Sanskrit) |
| GET  | `/api/v1/query` | ✓ | Query the trie |
| POST | `/api/v1/scan` | ✓ | Incremental scan of a dir; response reports changed/reused work (confined to `PHYSIS_SCAN_ROOT`) |
| GET  | `/api/v1/sql/schema` | ✓ | Discover SQLite tables and columns (read-only, confined to `PHYSIS_SCAN_ROOT`) |
| POST | `/api/v1/sql/scan` | ✓ | Read-only SQLite table ingestion with stable keys, row-level diffs, and incremental embeddings |
| POST | `/api/v1/sql/postgres/schema` | postgres | Discover PostgreSQL public tables and columns. DSN goes in the body, never a query string — it embeds the password |
| POST | `/api/v1/sql/postgres/scan` | postgres | Incremental PostgreSQL table ingestion using the shared row-delta contract |
| POST | `/api/v1/watch` | ✓ | **Watch-mode auto-ingest**: one incremental sweep over a dir (confined to `PHYSIS_SCAN_ROOT`) — registers new/changed files as labeled core nodes, drops nodes for removed files, and folds edits/deletions into salience → `{new, changed, deleted, registered, removed, total_nodes, observed}` |
| POST | `/api/v1/vault/import` | ✓ | **Vault import**: reduce a markdown/notes dir (+ optional `git` commits) to labeled core nodes (confined to `PHYSIS_SCAN_ROOT`) → `{docs, notes, commits, registered, deduped}` |
| POST | `/api/v1/history/import` | ✓ | **Personal-history import**: browser bookmarks/history, OPML feeds, chat JSONL → labeled core nodes (confined to `PHYSIS_SCAN_ROOT`) → `{file, items, registered, deduped}` |
| POST | `/api/v1/praxis/backfill` | ✓ | **Praxis backfill**: notebook entries + trackers → labeled behavioural nodes with asserted success/inert/failure verdicts (confined to `PHYSIS_SCAN_ROOT`) → `{items, successes, inert, failures, registered, deduped}` |
| POST | `/api/v1/mail/import` | ✓ | **Mailbox import**: a dir of `.eml` files + RFC-4155 `.mbox` bundles → one labeled node per message (`mail:<subject> — <from>`, native Date parsed); records `received(mailbox)` per source file (confined to `PHYSIS_SCAN_ROOT`) |
| POST | `/api/v1/files/query` | ✓ | **Space·Time·Cause query** over the tenant's causal ledger: facets `query`/`place`(path prefix)/`cause`(mechanism or actor)/`within_days`/`during YYYY-MM` → newest-first matches + `by_place`/`by_period`/`by_mechanism` rollups |
| POST | `/api/v1/files/serve` | ✓ | **Purpose link**: `{object, goal_id}` records a `serves` event — the final cause of an artifact, validated against the tenant's goals (404 otherwise) |
| GET  | `/api/v1/files/lineage` | ✓ | Why-made chain for one artifact `?path=`: upstream lineage, descendants, and purposes (serves links never enter derivation chains) |
| GET  | `/api/v1/goals/list` | ✓ | Every tenant goal `{id, label, progress}` — the purpose-link picker |
| POST | `/api/v1/spanner/sync` | spanner | **Spanner Graph sink**: mirror the tenant's coherence graph up (upsert + optional `mirror` orphan-delete); `ledger:true` also mirrors causal history as `cause:`-namespaced artifact nodes + directed cause edges (no orphan sweep — the ledger is append-only) |
| POST | `/api/v1/spanner/hydrate` | spanner | **Spanner hydrate**: pull durable rows, re-embed under the live embedder, persist into the tenant core → `{pulled, re_embedded}` |
| POST | `/api/v1/holarchy/ingest` | global | Push a node/edge into the holarchy graph |
| POST | `/api/v1/holarchy/edge` | ✓ | Assert a relation between two coherence nodes `{source, target, weight?, semantic_type?}` (bridges both) |
| GET  | `/api/v1/holarchy/snapshot` | ✓ | Holarchy node/edge counts for the tenant |
| GET  | `/api/v1/holarchy/trajectory` | ✓ | State-vs-goal trajectory |
| GET  | `/api/v1/holarchy/graph` | ✓ | Full graph export for the tenant |
| GET  | `/api/v1/holarchy/neighbors` | ✓ | Labelled neighbours of a coherence node `?id=<node_id>` — the walkable view (`/graph` returns opaque `RawNodeKey`s) |
| POST | `/api/v1/chat` | ✓ | LLM provider-cascade chat proxy |
| GET  | `/api/v1/providers` | — | Configured LLM providers |

### Example — classify

```bash
curl -X POST http://127.0.0.1:19876/api/v1/classify \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"text":"OEE dropped due to unplanned downtime on the CNC press"}'
# → top: FABRICATE×WORK (0.85) — OEE Metrics, Manufacturing Processes
#   (exemplar entries are ranked by their own similarity to the query)
```

### Example — ask (plain-English graph query)

```bash
curl -X POST http://127.0.0.1:19876/api/v1/ask \
  -H "Content-Type: application/json" \
  -H "X-Physis-User: plant-1" \
  -d '{"question":"what do I know about the hydraulic press?"}'
# → {"answer":"The hydraulic press on line 2 drifts above its nominal
#    tonnage when it is cold. [1]","provider":"groq", ...}
# Without a configured provider the same call returns the ranked matches
# plus a note — it never errors.
```

At startup every ontology entry (name + hints) is embedded and averaged per
DOMAIN×MODE cell. At query time the same embedder scores the input against all
centroids; each cell's score is the **max cosine to any member entry**.

**Embedder matters.** With real weights (BGE-base via
`scripts/download_models.sh`) top-1 DOMAIN accuracy on the curated core set is
0.891 and scores discriminate. The deterministic random-projection fallback is
wiring-plumbing only: it is **non-semantic** — scores compress into a flat
~0.88–1.00 band and top cells are often wrong. The CLI prints a warning when it
is active; don't demo or gate on RP output.

### Auth

Protected routes require `Authorization: Bearer <PHYSIS_API_TOKEN>` (constant-time
compared).

- **Token unset** + loopback bind → auth is skipped (local dev convenience).
- **Token unset** + non-loopback bind → the server **refuses to start** (it would
  publish an unauthenticated API that also proxies paid LLM providers — an open
  relay). Set the token or bind `127.0.0.1`.

### Contextual meaning — overlays, interpretants, salience

An ontology entry is a sign: `hints` are its signifier, its embedding centroid
the signified. The third leg, the **interpretant**, is declared per ontology
file — what counts *in that universe of discourse* as one of its signs having
meant something:

```json
{ "kind": "machine",
  "interpretant": { "confirms": ["success","inert"], "refutes": ["failure"],
                    "half_life_days": 0, "weight": 1.0,
                    "note": "an inspection that found nothing still confirms the routine" } }
```

`machine` and `software_dev` read `inert` in opposite directions, which no
single global rule can express.

**Overlays** scope what an item *means* to a sector, client, or project.
Resolution runs `project → client → sector → global`, narrowest wins, and an
overlay shadows only the names it mentions:

```bash
# ~/.physis/ontologies/{sector,client,project}/<id>.json
export PHYSIS_CONTEXT="sector:manufacturing,client:evotec"
curl "localhost:19876/api/v1/ontology/meaning?name=Release"
# → manufacturing: FABRICATE×MOVE (release to production)
#   evotec:        HEAL×GUIDE     (batch release, QP sign-off)
#   publisher:     CONSTRUCT×CREATE (publication date)
```

**Salience** records what an entry has been worth. `/watch` folds file events
into it — editing a classified file confirms its entry, deleting it refutes —
and the accumulated evidence biases classification within a bounded budget
(0.05 cosine, `tanh`-saturated), so it reorders close calls but cannot overturn
a clear semantic win. Salience is keyed by context, so one client's evidence
never biases another's.

Full detail, including the measurements that motivated all of it:
[`docs/CONTEXTUAL_MEANING.md`](docs/CONTEXTUAL_MEANING.md).

### Semiotic node editing ("exchange parts") & fixed-token search

A coherence node is **structure** (id, `asserted` verdict, holarchy edges,
embedder provenance, grid position) + **content** (label + embedding). Because
the two are compatible, an edit swaps the content while the function survives:

```bash
# Exchange a node's content; the id/verdict/edges never change.
curl -X POST http://127.0.0.1:19876/api/v1/coherence/edit \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"id":"<node_id>","label":"welding robot joint recalibration",
       "cell":{"domain":"FABRICATE","mode":"MAINTAIN"}}'
# → { ok, node_id, label, auto_cell, pinned_cell, before_cell, asserted, coherence_score }

# Fixed-token search over coherence + goal nodes (cost is bounded up front).
curl -X POST http://127.0.0.1:19876/api/v1/search/nodes \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"query":"spindle bearing","token_budget":600}'
# → { query, budget, total_tokens ≤ 600, truncated, results:[{kind,id,label,score,tokens}] }
```

See [docs/SEMIOTIC_EDITING.md](docs/SEMIOTIC_EDITING.md) for the invariant, the
hybrid pin semantics, and the full API contract.

### Vendor license generator

The vendor-only Ed25519 key and license generator is available through the
wrapper below. It requires the `vendor` feature and must run on an offline
vendor machine; never copy `signing.key` to pansa or a customer host.

```bash
scripts/generate-physis-license.sh keygen ~/.physis-vendor
scripts/generate-physis-license.sh issue \
  --key ~/.physis-vendor/signing.key \
  --org "Customer Name" --seats 5 --features web,rag \
  --exp 2027-12-31 --out customer.license.key
```

The customer host receives only `customer.license.key`, normally at
`~/.physis/license.key` with mode `600`. See
[`docs/business/DEPLOY.md`](docs/business/DEPLOY.md) for the pansa deployment
and renewal procedure.

## Docs

- [`docs/CONTEXTUAL_MEANING.md`](docs/CONTEXTUAL_MEANING.md) — interpretants, per-client/sector/project ontology overlays, salience feedback
- [`docs/SEMIOTIC_EDITING.md`](docs/SEMIOTIC_EDITING.md) — exchange-parts node editing + fixed-token search
- [`docs/UMBRELLAPEDIA_GUIDE.md`](docs/UMBRELLAPEDIA_GUIDE.md) — wiring physis to the Umbrellapedia wiki surface
- [`docs/PIPELINE_DIAGRAMS.md`](docs/PIPELINE_DIAGRAMS.md) — ingestion→recall pipeline + live engine phases
- [`docs/PUBLIC_ARCHITECTURE.md`](docs/PUBLIC_ARCHITECTURE.md) — cross-repo public deployment
- [`docs/plans/2026-08-26-space-time-cause-datawork.md`](docs/plans/2026-08-26-space-time-cause-datawork.md) — the datawork design: file addresses (space/time/cause), the causal ledger, place/period ontologies

---

## Run

```bash
# CLI
cargo run --bin physis-pro -- scan <dir>
cargo run --bin physis-pro -- query "<text>"
cargo run --bin physis-pro -- classify <file.txt>          # semiotic-grid map of a text
cargo run --bin physis-pro -- dream --count 5
cargo run --bin physis-pro -- discover <dir> [--threshold T] [--json-out F]  # ontology gaps in a corpus → draft entries
cargo run --bin physis-pro -- report --shift <input.json> [--html]           # Italian shift report (Markdown or HTML)
cargo run --bin physis-pro -- stats

# Semiotic node editing ("exchange parts") — same sled store as the web server
cargo run --bin physis-pro -- node-edit <id> "welding robot joint recalibration" \
    --domain FABRICATE --mode MAINTAIN     # optional pin; auto cell without it
cargo run --bin physis-pro -- node-delete <id>
cargo run --bin physis-pro -- node-search "spindle bearing" --budget 600 --top 12

# Watch-mode auto-ingest: register new/changed files as labeled core nodes
cargo run --bin physis-pro -- watch <dir>            # loop (re-scan on interval)
cargo run --bin physis-pro -- watch <dir> --once     # single sweep (cron/smoke)

# Vault import: Obsidian/markdown notes (+ git log) → labeled core nodes
cargo run --bin physis-pro -- vault <dir>            # notes only
cargo run --bin physis-pro -- vault <dir> --git 50   # notes + last 50 commits

# Personal-history import: browser bookmarks/history, OPML feeds, chat JSONL
cargo run --bin physis-pro -- history bookmarks.html
cargo run --bin physis-pro -- history history.json
cargo run --bin physis-pro -- history feeds.opml

# Praxis backfill: seed coherence from the real life-log
cargo run --bin physis-pro -- praxis export.json   # notebook + trackers → verdicts

# Space·Time·Cause: ask where/when/why about ingested artifacts (causal ledger)
cargo run --bin physis-pro -- where "press"                    # everything mentioning press
cargo run --bin physis-pro -- where --cause mailbox --within 30 # mail that arrived in 30 days
cargo run --bin physis-pro -- where --place ~/plant --during 2026-08            # place + period

# Industry ontology packs (4.2): drop JSON ontology files, no rebuild
mkdir -p ~/.physis/packs && cp docs/examples/logistics_pack.json ~/.physis/packs/
# → entries join classification keyed "logistics_pack:<name>" at next start

# Spanner Graph sink (Phase 7C): durable, cross-device memory mirror
cargo run --bin physis-pro --features spanner -- spanner sync --mirror   # upsert + delete orphans (+ causal ledger)
cargo run --bin physis-pro --features spanner -- spanner hydrate         # pull + re-embed
# env: SPANNER_PROJECT/INSTANCE/DATABASE + GOOGLE_APPLICATION_CREDENTIALS or GOOGLE_OAUTH_TOKEN

# Experiment — Empedoclean dyad criticality (research; see docs/EMPEDOCLES_DYAD.md)
cargo run --release --example empedocles_criticality

# HTTP server (feature: web, on by default)
cargo run --bin physis-pro-web          # binds 127.0.0.1:19876

# Voice — live ontological mapping (features: voice,web), UI on :3000
cargo run --bin physis-pro-voice

# TUI (features: voice,tui)
cargo run --bin physis-pro-tui
```

### Environment

| Var | Default | Purpose |
|-----|---------|---------|
| `PHYSIS_PORT` | `19876` | HTTP port |
| `PHYSIS_HOST` | `127.0.0.1` | Bind host (non-loopback requires a token) |
| `PHYSIS_API_TOKEN` | — | Bearer token for protected routes |
| `PHYSIS_SCAN_ROOT` | data dir | Confines `/api/v1/scan` (path-traversal guard) |
| `PHYSIS_WATCH_DIR` | — | Watch-mode auto-ingest: poll this dir (inside scan root) and register file changes as labeled core nodes |
| `PHYSIS_WATCH_INTERVAL_SECS` | `30` | Poll interval for `PHYSIS_WATCH_DIR` |
| `PHYSIS_WATCH_DIR` | — | Poll this dir (within scan root) and auto-register new/changed files as labeled nodes |
| `PHYSIS_WATCH_INTERVAL_SECS` | `30` | Watch-mode poll interval |
| `PHYSIS_RERANK` | unset | Enable LLM re-rank on `/classify` for ambiguous cells (Gemma/provider cascade); needs provider keys |
| `PHYSIS_RERANK_MARGIN` | `0.05` | Top-2 score gap below which re-rank fires — only ambiguous queries hit the LLM |

**BYOK re-rank**: a caller can activate the re-ranker per request without any server
key by passing `llm_key` (and optional `llm_base_url`, `llm_model`) in the `/classify`
body — physis builds a one-off OpenAI-compatible provider for that call. Defaults to
Gemini (`gemini-2.5-flash`). The caller's key is used only for that request and never
stored. Lets each user bring (and pay for) their own model.

### Config (`~/.physis`)

```json
{
  "data_dir": "~/.physis",
  "dream_batch_size": 5,
  "pdca_stagnant_threshold": 0.2,
  "pdca_stagnant_window": 5
}
```

Default ontologies live in `config/` (`praxis_ontology.json`, human;
`machine_ontology.json`, machine; plus the semiotic set below).

---

## Semiotic grid & ontologies

Every entry maps to one of **70 cells** — **5 domains** (HEAL, CONSTRUCT,
FABRICATE, BOND, STUDY) × **14 modes** (LIFT, REST, WALK, WORK, CREATE, LEARN,
DESTROY, SENSE, GUIDE, PLAY, BRAINSTORM, MAINTAIN, MOVE, PLAN). The last eight
extend the original activity-energy axis: DESTROY completes the Greimas opposition
with CREATE, SENSE is perception/measurement, GUIDE is leading/mentoring others
(vs LEARN, self-study), PLAY is low-stakes generative, BRAINSTORM is divergent
ideation, MAINTAIN is upkeep/repair/preserve, MOVE is relocate/transport/logistics,
PLAN is convergent sequencing/organizing (the converge to BRAINSTORM's diverge).

Ships **10 ontologies** (~630 entries):

| Ontology | Kind | ~Entries | Coverage |
|----------|------|:---:|----------|
| `praxis_ontology` | human | 54 | Body, career, social, finance, … |
| `machine_ontology` | machine | 110 | CNC + 23 industrial monitoring domains |
| `semiotic_ontology` | semiotic | 44 | Peirce, Saussure, Barthes, Eco, Jakobson, Greimas |
| `category_ontology` | category | 54 | Objects, morphisms, functors, monads |
| `agent_ontology` | agent | 69 | AI cognitive architectures + agent workflows |
| `natural_ontology` | natural | 56 | Physics, chemistry, biology, astronomy, ecology |
| `social_ontology` | social | 50 | Sociology, culture, politics, economics, law |
| `abstract_ontology` | abstract | 56 | Math, logic, computation, systems, linguistics |
| `engineering_ontology` | engineering | 65 | Software, civil, mechanical, electrical, chemical |
| `office_ontology` | office | 75 | Comms, finance, procurement, people, projects, … |

Plus **20 extra domain ontologies** baked in via `EXTRA_ONTOLOGIES` and used by
`classify` (`software_dev`, `devops`, `data_science`, `cybersecurity`,
`health_fitness`, `nutrition_cooking`, `personal_finance`, `learning_education`,
`creative_writing`, `music`, `visual_arts`, `relationships`, `productivity`,
`travel`, `home_maintenance`, `mindfulness_spirituality`, `science_research`,
`business_entrepreneurship`, `language_learning`, `outdoors_sports`).

---

## Multimodal — one shared vector space

Text, images, and video land in the **same** embedding space, so a photo and a
sentence are directly comparable and classifiable through the same core.

- **Images** — `embed_image(bytes)` on **CLIP** (`embed/clip.rs`) or **Jina v2**
  (`embed/jina.rs`); `/classify` and `/embed` accept an optional base64 `image`
  and fall back to `text` on failure (never 500).
- **Video** (`src/video.rs`) — ffmpeg frame-sample → `embed_image` per frame →
  temporal mean-pool + L2-norm → one clip vector. `POST /api/v1/embed/video` takes
  a server-side `path` (confined to scan root) or base64 `video_base64`, plus
  optional `fps` (1.0) / `max_frames` (32). Fail-soft → `422`, never a crash;
  needs an image-capable embedder.
- **Voice** (`physis-pro-voice`) — `arecord` 15s chunks → whisper-large-v3
  (provider cascade) → llama3.2 concept extraction → live `OntologicalMap` graph
  UI on `:3000`. Supersedes the standalone `physis_voice` repo.
- **Sensory bus** (`src/sensory.rs`) — `AuraFrame` wire format (`'AURA'` magic,
  µs timestamp, `SignalType`, payload) over a Unix socket. `SignalType`:
  `AudioPCM`, `VisionFeature`, `Acceleration`, `Gyroscope`, `ThoughtCapture`. Any
  source that speaks `AuraFrame` streams into the same engine.

### Embedders

Selection is **self-test-gated**: at startup each candidate must pass a semantic
probe (dog≈puppy must beat dog≈tax-spreadsheet) or it is skipped. The chain:

1. **BGE-base-en-v1.5** (768-d, production default) — `models/bge-base-en-v1.5/`
2. **ONNX MiniLM** (384-d fallback) — `models/model.onnx` + `models/tokenizer.json`
3. **Random projection** (deterministic, no model files) — last resort; the
   server marks itself `degraded:true` on `/api/v1/embedder` and logs at `error`.

Model weights are git-ignored. Locally, export with `optimum-cli`:

```bash
pip install optimum onnx
optimum-cli export onnx -m BAAI/bge-base-en-v1.5 models/bge-base-en-v1.5/
cargo build --features embed-onnx     # (on by default; falls back down the chain)
```

In production, `docker-entrypoint.sh` fetches weights from `MODEL_BASE_URL`
at boot (see PREMORTEM.md P0). Check what's live with `GET /api/v1/embedder`.

### Docker (self-contained)

```bash
# Without Lean (default, smaller image)
docker build -t physis-pro .

# With Lean 4 toolchain (~+500 MB, enables apodeixis machine)
docker build --build-arg WITH_LEAN=1 -t physis-pro:lean .
```

The runtime image is Debian slim + the Rust binary + optional Lean. Models are
downloaded at boot from `MODEL_BASE_URL` unless baked in at build time via S3.

---

## Modules

| Module | Role |
|--------|------|
| `core` | `PhysisCore` — UberWiki, compression, dream, branch certification, context filter |
| `models` | `CoherenceRating`, `CoherenceNode`, `ConstructiveRefutation`, `Goal`, `Experience`, `Dream` |
| `trie` | `DynamicVectorTrie` token storage/retrieval |
| `classify` | Semiotic-grid classification (nearest-entry max-cosine over DOMAIN×MODE cells) |
| `discovery` | Ontology-gap discovery — embeds a corpus, flags texts the grid covers poorly (tunable threshold + surfaced sim distribution), clusters gaps → draft ontology JSON |
| `ontology` / `ontology_nonhuman` | Human + machine/non-human ontology loading |
| `config` | Config + ontology bootstrap |
| `scanner` | Filesystem scan with hash-based change detection |
| `mapper` | Filesystem → goal ontological mapping |
| `vfs` | Read-only ontology-organized view of files via symlinks |
| `actor` | PDCA (Plan-Do-Check-Act) cycle engine |
| `rachmaninov` | PDCA directive engine — `Focus`/`Expand`/`Prune`/`Synthesize`, state-vs-goal tick |
| `dream` | `DreamEngine` — stochastic generation (mutate / graft / prune). Reframed as the Philotes (integration) phase of one metabolism — see `docs/EMPEDOCLES_DYAD.md` |
| `empedocles` | **Research, quarantined.** `NodeField` under Eros (attraction + synthesis) vs Nemesis (repulsion + decay); the `empedocles_criticality` example confirms a phase transition. Wired to nothing on the product path |
| `graph` | Holarchic graph — `DenseSlotMap`, `NodePayload`, `Edge`, mmap storage, ONNX worker |
| `gantt` | `GanttTask` scheduler — start/end, deps, critical path over `RawNodeKey` |
| `sensory` | `AuraFrame` multimodal ingestion bus |
| `video` | ffmpeg frame-sample → clip vector |
| `embed` | `VectorEmbed` trait — BGE-base / ONNX-MiniLM / RandomProjection / CLIP / Jina |
| `quantize` | Product quantizer (f32 → byte codes) |
| `reconstruct` | Nearest-neighbour reconstruction + LLM interpretation |
| `storage` | Zero-copy mmap storage for `Pod` types |
| `output` | Formatters — Wiki / JSON graph / Mermaid / domain report |
| `linguistic` | Routing middleware — Wenyan / Pirahã / Sanskrit |
| `network` | Directory watch + network event detection (feature `network`) |
| `mcp` | MCP server for agent tool-use (feature `mcp`) |
| `ai` | Provider cascade (OpenAI/Anthropic/…), tool-using agent loop, episodic memory, ONNX worker |
| `life_os` | `LifeOSTracker` — goal/habit tracking layer |
| `social` | `SocialAlgorithm` — engagement metrics × coherence scoring |
| `quality` | `QualityTracker` — classify-failure feedback loop, persisted |
| `industrial` | Machine adapters (MQTT / Modbus / serial / REST) + anomaly detection (feature `industrial`) |
| `ingest` | Document classification/ingest pipeline (feature `ingest`) |
| `inventory` | Stock tracker — SMA-30 forecast + human-in-loop reorder |
| `oee` | Overall Equipment Effectiveness — `OeeShift`/`OeeBoard`, A×P×Q, six-big-losses Pareto, plant rollup |
| `maintenance` | Per-machine learned health baselines, σ-anomaly scoring, least-squares RUL projection |
| `report` | Italian shift report — composes OEE + anomalies + RUL + dream countermeasures into Markdown or self-contained HTML |
| `spanner` | Optional Google Spanner sink (feature `spanner`) — durable holarchy mirror: chunked upsert, hard deletes, `sync_full`, SA-JWT auth |
| `wiki` | `llmwiki` CLI fallback with recursive-grep offline retrieval |
| `cli` | clap CLI (feature `cli`) |

---

## Core API (Rust)

```rust
use physis_pro::core::PhysisCore;
use physis_pro::models::CoherenceRating;

let mut core = PhysisCore::new();
core.set_embedder_id("bge-base");   // vector provenance, stamped on every node

// Register by text — the text is kept as the label, so the node is browsable
// by content. Labeled registrations are deduped: same text ⇒ same id.
let id = core.register_node_from_text("exercise: running", &embedder);

// Report the outcome (the asserted axis). Re-asserting corrects a verdict.
core.assert_coherence(&id, CoherenceRating::Success.weight());

// Compression — validate a raw input against state
let result = core.filter_context_embedded(embedder.embed("running does nothing"), 4);
// result.valid == false; result.conflict == Some(ConstructiveRefutation { .. })

let certified  = core.certify_branches();
let isolated   = core.detect_contradictions();
let compressed = core.compress_logs(&daily_logs);   // Compression
let dreams     = core.dream();                      // Dream — replays failures
let density    = core.coherence_index();            // derived, over all nodes
let verdicts   = core.asserted_index();             // reported, judged nodes only
let snap       = core.snapshot();                   // for UI (both axes)

// Durability — nodes (and their verdicts) survive a restart.
core.persist_node(&db, "tenant-a", &id)?;
let (loaded, skipped) = core.load_nodes(&db, "tenant-a");  // skipped = embedder mismatch
```

---

## Build & test

```bash
# Quick build (default features: cli, voice, web, tui, embed-onnx)
cargo build

# Feature matrix
cargo build --features mcp        # + MCP server
cargo build --features network    # + directory watching
cargo build --features full       # everything (industrial, ingest, pdf, email)

# One-shot dependency installer (probes + installs rust, lean, voice deps, models)
./scripts/setup-physis.sh --full

# Then deploy (systemd --user, no root required)
./scripts/install-physis.sh --port 8080
```

> **Deploys:** Railway auto-deploys on green CI. When CI is unavailable, deploy
> by hand with `./scripts/predeploy.sh && railway up --service physis --detach`
> — the script mirrors the CI gate (clippy `-D warnings`, full tests, the
> accuracy floor) and refuses a dirty/unpushed tree, so prod always equals a
> pushed, gated commit (PREMORTEM P9).

### Features

`cli` · `mcp` · `network` · `voice` · `web` · `tui` · `embed-onnx` · `industrial`
· `ingest` · `spanner` · `wiki-embedded` · `full`. Default = `cli, voice, web, tui, embed-onnx`.

### Binaries

`physis-pro` (CLI) · `physis-pro-web` (HTTP) · `physis-pro-tui` · `physis-pro-voice`.

---

## Practical Examples

### Industrial Monitoring — CNC Press OEE

```bash
# 1. Register the machine (once)
curl -X POST http://127.0.0.1:19876/api/v1/oee/machine \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"machine_id":"cnc-press-01","shift_length_secs":28800,"ideal_cycle_time_secs":45}'

# 2. Record production events (every shift)
curl -X POST http://127.0.0.1:19876/api/v1/oee/event \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"kind":"start_shift","machine_id":"cnc-press-01"}'

curl -X POST http://127.0.0.1:19876/api/v1/oee/event \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"kind":"production","machine_id":"cnc-press-01","good":480,"scrap":12}'

curl -X POST http://127.0.0.1:19876/api/v1/oee/event \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"kind":"stop","machine_id":"cnc-press-01","reason":"unplanned","duration_secs":1800,"note":"tool change"}'

# 3. Get OEE report
curl -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  "http://127.0.0.1:19876/api/v1/oee/cnc-press-01"

# 4. Generate Italian shift report (Markdown or HTML)
curl -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  "http://127.0.0.1:19876/api/v1/report/shift?format=html"
```

### Document Ingestion & Knowledge Graph

```bash
# Ingest a vault of markdown notes + git history
cargo run --bin physis-pro -- vault ~/my-notes --git 100

# Import browser bookmarks
cargo run --bin physis-pro -- history ~/bookmarks.html

# Import OPML feed subscriptions
cargo run --bin physis-pro -- history ~/feeds.opml

# Query the resulting knowledge graph
cargo run --bin physis-pro -- query "machine maintenance"
cargo run --bin physis-pro -- where "hydraulic" --cause mailbox --within 30
```

### Quality Feedback Loop

```bash
# Classify a failure report
curl -X POST http://127.0.0.1:19876/api/v1/classify \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"text":"Spindle bearing overheating on CNC lathe line 3"}'

# Report the outcome (failure) — teaches the engine
curl -X POST http://127.0.0.1:19876/api/v1/coherence/register \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"input":"Spindle bearing overheating on CNC lathe line 3","coherence_score":-1.0}'

# View learned penalties
curl -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  "http://127.0.0.1:19876/api/v1/quality"
```

### Rust API — Build a Custom Agent

```rust
use physis_pro::{
    core::PhysisCore,
    models::{CoherenceRating, CoherenceNode},
    classify::OntologyLoader,
    config::PhysisProConfig,
    embed::{VectorEmbed, RandomProjectionEmbedder},
};

fn main() -> anyhow::Result<()> {
    // Load ontologies + config
    let config = PhysisProConfig::default();
    let ontology = OntologyLoader::load_all(&config);
    
    // Use deterministic embedder for reproducibility (no model files needed)
    let embedder = RandomProjectionEmbedder::new(64);
    
    let mut core = PhysisCore::new();
    core.set_embedder_id("random-projection");

    // Register a production incident
    let id = core.register_node_from_text(
        "Hydraulic press drift: tonnage +12% when cold",
        &embedder,
    )?;

    // Assert the outcome — this is the "ground truth" signal
    core.assert_coherence(&id, CoherenceRating::Failure.weight());

    // Run a dream cycle to propose countermeasures
    let dreams = core.dream();
    for dream in dreams {
        println!("Proposal: {}", dream.proposal);
        println!("Grade: {:.2}", dream.grade);
    }

    // Fixed-token search (bounded context for LLM)
    let results = core.search_nodes("hydraulic press", 500, 10)?;
    for r in results {
        println!("[{:.2}] {}", r.score, r.label);
    }

    // Persist to Sled (survives restart)
    let db = sled::open("~/.physis/mydb")?;
    core.persist_node(&db, "plant-1", &id)?;
    
    Ok(())
}
```

### Multi-Tenant Web Deployment

```bash
# Start server (binds 127.0.0.1:19876 by default)
cargo run --bin physis-pro-web

# Client A (tenant "plant-1")
curl -H "X-Physis-User: plant-1" -H "Authorization: Bearer $TOKEN" \
  -X POST http://127.0.0.1:19876/api/v1/goals \
  -d '{"prompt":"Reduce unplanned downtime on line 2"}'

# Client B (tenant "plant-2") — completely isolated state
curl -H "X-Physis-User: plant-2" -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -X POST http://127.0.0.1:19876/api/v1/classify \
  -d '{"text":"Conveyor belt misalignment"}'

# Shared holarchy (global knowledge graph)
curl -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:19876/api/v1/holarchy/snapshot"
```

### Spanner Graph — Durable Cross-Device Memory

```bash
# Set up service account credentials
export SPANNER_PROJECT=my-project
export SPANNER_INSTANCE=physis
export SPANNER_DATABASE=core
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/sa.json

# Sync tenant graph to Spanner (upsert + mirror orphans)
cargo run --bin physis-pro --features spanner -- spanner sync --mirror

# Hydrate from Spanner (pull + re-embed under live embedder)
cargo run --bin physis-pro --features spanner -- spanner hydrate
```

### Docker Production Deploy

```bash
# Build (downloads models at build time via MODEL_BASE_URL)
docker build --build-arg MODEL_BASE_URL=https://my-s3-bucket/models -t physis-pro .

# Run (auto-downloads models from MODEL_BASE_URL if not baked)
docker run -d \
  -p 19876:19876 \
  -e PHYSIS_API_TOKEN=your-secure-token \
  -e PHYSIS_HOST=0.0.0.0 \
  -e PHYSIS_SCAN_ROOT=/data \
  -v /host/data:/data \
  physis-pro

# With Lean 4 (for apodeixis machine proofs)
docker build --build-arg WITH_LEAN=1 --build-arg MODEL_BASE_URL=... -t physis-pro:lean .
```

---

## See also

- `AGENTS.md` — multi-tenancy contract + agent integration notes
- `DEVELOPMENT_PLAN.md` — roadmap (Phase 6: multi-embedder registry, Gemma re-ranker)
- `PREMORTEM.md` — risk register (P4 ONNX pin, P6 open-relay gate)
- `MANUAL_CONFIG.md` — deploy/config steps
- CI: `.github/workflows/ci.yml` (builds on every push/PR)
