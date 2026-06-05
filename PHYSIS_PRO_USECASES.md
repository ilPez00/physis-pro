# Physis Pro — Use Cases & UberWiki Cookbook

How to feed the UberWiki, browse it by **content** (not name/type), and what else
the engine is good for. Everything here builds on shipped primitives: the shared
embedding space, semiotic-grid `classify`, coherence weights (+1/0/−1), the
holarchy graph, and the `dream` loop. See `README.md` for the API.

---

## 1. Populating the UberWiki

The UberWiki is just coherence nodes + holarchy edges in the shared vector space.
Anything you can turn into a vector becomes browsable and classifiable.

### 1.1 Already shipped

| Path | What it ingests | How |
|------|-----------------|-----|
| `physis scan <dir>` | Files on disk → ontology nodes | `scanner` + `mapper`, hash-based change detection |
| `POST /api/v1/holarchy/ingest` | Arbitrary node/edge | Programmatic push (Praxis uses this) |
| `POST /api/v1/coherence/register` | A node + `coherence_score` | Behavioural/causal facts |
| `POST /api/v1/goals` | A prompt → goal | Per-user tenant |
| `POST /api/v1/compress/logs` | Raw logs → dense causal rules | Compression function |
| `AuraFrame` sensory bus (Unix socket) | Live audio / vision / motion / `ThoughtCapture` | Streaming, multimodal |
| `embed` / `classify` (image, video) | CLIP/Jina images, ffmpeg video frames | Same vector space as text |

### 1.2 To build — connectors (each: source → `classify` → node)

- **Watch mode** — `network`/`notify` feature: auto-ingest on file create/modify;
  the wiki stays live without re-scanning.
- **Vault importers** — Obsidian/markdown tree, plain notes, `git log`, code repos.
- **Personal-history importers** — browser history + bookmarks, email/chat exports,
  RSS/read-later, photo library (EXIF + CLIP embedding).
- **Praxis backfill** — `notebook_entries` + trackers → `register_behavioural_vector`
  (Success/Inert/Failure) so the real life-log seeds coherence, not just text.

> Design rule: a connector's only job is `bytes/text → embed → register node (+ edges)`.
> Keep them thin; the engine does the reasoning.

---

## 2. Browsing by content — the TUI Wiki Browser

**Problem:** you remember *what a thing was about*, not its filename or type.
**Why physis solves it:** every node is a vector, so cosine similarity ranks by
*meaning*. `reconstruct` returns the nearest item + an LLM interpretation even when
you can't name it.

### 2.1 The tool

A new ratatui binary (`ratatui` is already a dependency) that talks to the HTTP
server over `reqwest` — works against a local **or** remote engine, no engine
changes required.

```
┌ search: red bridge tax thing________________________ ┐  ← type CONTENT you recall
│ results (cosine-ranked)        │ preview              │
│ ● Q3_invoice_venice.pdf   .91  │ <node content>       │  ● coherence color:
│ ○ note_2026-03-11         .78  │ domain × mode  HEAL  │    +1 green / 0 grey / −1 red
│ ○ bridge_photo.jpg        .74  │ neighbors →          │
│ ○ "ponte" voice memo      .71  │ ── LLM reconstruct ──│
├ semiotic heatmap (5×6) ────────┴──────────────────────┤  ← arrow keys filter by cell
└ j/k move · enter open · / search · g graph · d dream ─┘
```

### 2.2 Panes & behaviour

- **Semantic palette** (top) — as you type, embed the query and live re-rank.
  Forgot name/type? Content still finds it.
- **Results** (left) — cosine-ranked; each row shows a DOMAIN×MODE chip and a
  coherence color dot.
- **Preview** (right) — content + metadata + nearest neighbors + the `reconstruct`
  LLM interpretation.
- **Heatmap** (bottom) — the 30-cell grid; arrow into a cell to scope results to it.
- **Graph walk** (`g`) — open a node's holarchy edges, traverse neighbors like a
  file browser (vim keys: `j/k` move, `enter` open, `backspace` up).
- **Dream** (`d`) — for an Inert/Failure node, show dream-generated corrective
  actions.

### 2.3 Endpoints it uses (all exist)

`/api/v1/classify`, `/api/v1/query`, `/api/v1/reconstruct`,
`/api/v1/holarchy/snapshot`, `/api/v1/holarchy/graph`, `/api/v1/semiotic/heatmap`,
`/api/v1/stats`. Auth via `Authorization: Bearer $PHYSIS_API_TOKEN`,
tenant via `X-Physis-User`.

---

## 3. Other use cases

Grouped by the primitive they lean on.

### Content recall & search (embeddings + `reconstruct`)
- **Forgotten-file finder** over the whole disk (the TUI, standalone).
- **Codebase semantic map** — scan a repo → engineering-ontology cells → "where's
  the thing that does X" by content, not grep.
- **Near-dup / dedup** — cosine flags duplicate notes, images, files.

### Clustering & connection (semiotic grid + `dream`)
- **Reading/research inbox** — ingest papers/articles → cluster by cell → dream
  surfaces cross-domain links (Praxis notebook-rhymes, generalized).
- **Idea constellation** — periodic dream pass over recent notes proposes
  non-obvious links.

### Coherence tracking (behavioural vectors + PDCA)
- **Decision journal** — log decisions as behavioural vectors; learn which patterns
  end Success vs Failure *for you*.
- **Log triage / incident** — `compress_logs` on prod logs → coherence drift → dream
  proposes the fix.
- **Habit / Life-OS** — `life_os` tracker + coherence = "what's actually working."

### Memory for agents & devices (MCP + sensory bus)
- **Agent long-term memory** — MCP server gives an LLM episodic recall *keyed by
  coherence* (remember what worked, not just what was said).
- **Cross-device memory** — wearable + phone + desktop all stream `AuraFrame`s into
  one personal UberWiki.

### Industrial backoffice (shipped modules)
- **Inventory** — SMA-30 forecast + human-in-loop reorder.
- **Document ingest** — invoices / contracts / manuals classified and routed.
- **Machine anomaly** — MQTT/Modbus readings → anomaly detection → coherence.

---

## 4. Durable storage — Spanner Graph tie-in

Physis is the **brain** (embed / classify / cohere / dream, single-node, local).
For durable, multi-device, queryable memory, persist holarchy nodes + edges into
**Google Spanner Graph**:

- physis computes vectors + coherence locally and emits node/edge mutations;
- a sink writes them to Spanner Graph (property graph over Spanner SQL);
- other devices / services read them back via GQL, or re-hydrate a local tenant.

They layer, not compete: physis = reasoning, Spanner Graph = durable distributed
memory. A vector column in Spanner can even hold the embedding for server-side ANN,
with physis as the classification/coherence layer on top.

---

## 5. Build order (see `DEVELOPMENT_PLAN.md` Phase 7)

1. **TUI Wiki Browser** — highest leverage, no engine changes; unlocks content recall.
2. **Watch-mode + vault importers** — keep the wiki fed automatically.
3. **Praxis backfill** — seed coherence from the real life-log.
4. **Spanner Graph sink** — durability + sync.
