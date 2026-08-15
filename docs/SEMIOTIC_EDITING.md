# Semiotic Node Editing & Fixed-Token Search

Physis can edit **individual data nodes** through the semiotic grid — the
"exchange parts" primitive. A node is a pair of independent halves:

- **Structure** — the function: `id`, `asserted` verdict, holarchy edges,
  embedder provenance, grid position (auto or pinned).
- **Content** — the data: `label` (the source text) and the embedding that
  represents it.

Because the two halves are compatible (separable), you can **swap the content
without losing the function**. Edit the label, re-embed it, and the node keeps
its identity, its verdict, and its relations; only its meaning (and thus its
auto-derived grid cell) changes.

This resolves the long-standing TODO in `DEVELOPMENT_PLAN.md`:
"can we make physis ontology studio semiotics editing via file affinity scan
and adaptation … hotswapping conceptual objects without losing identity (or
function)?" — the spiderweb surface (`/api/v1/fs/graph` + `graph_fm.html`)
scans files into the grid; the **edit** half now exists as the endpoints below.

## The structure / content invariant

| Field | Axis | Preserved on edit? |
|-------|------|:---:|
| `id` | structure | ✅ same id, always |
| `asserted` (verdict) | structure | ✅ |
| holarchy bridge + edges | structure | ✅ |
| `embedder` provenance | structure | ✅ re-stamped to the live embedder |
| `cell_pin` (manual anchor) | structure | ✅ (cleared unless re-supplied) |
| `coherence_score` | derived | 🔁 recomputed from the new content |
| `label` | content | 🔁 replaced |
| `embedding` | content | 🔁 replaced (re-embedded) |

The dedupe index (`label_index`) is a **structure** field too: it moves with the
label. After an edit the old label is free to register again (a new node), and
the new label dedupes to the edited node.

## Edit semantics — hybrid with manual pin

1. **Auto cell** — the new content is embedded and classified (nearest-entry
   scoring) to derive its `DOMAIN×MODE` cell.
2. **Pin override** — the editor may supply `cell: {domain, mode}`; this is
   stored on the node (`cell_pin`) and survives restarts. A pin is the studio's
   "anchor an entry to a cell" behaviour applied to a data node.
3. The edit response reports both cells so a client can show the **before →
   after** delta and whether the pin won over the auto result.

## API

All three endpoints are tenant-aware (`X-Physis-User`).

### `POST /api/v1/coherence/edit`

Exchange a node's content while keeping its structure.

```json
{
  "id": "<node_id>",
  "label": "welding robot joint recalibration",
  "cell": { "domain": "FABRICATE", "mode": "MAINTAIN" }
}
```

`cell` is optional (omit → auto-derive only). Response:

```json
{
  "ok": true,
  "node_id": "<same id>",
  "label": "welding robot joint recalibration",
  "auto_cell": { "domain": "FABRICATE", "mode": "CREATE" },
  "pinned_cell": { "domain": "FABRICATE", "mode": "MAINTAIN" },
  "before_cell": { "domain": "FABRICATE", "mode": "WORK" },
  "asserted": -1.0,
  "coherence_score": 0.42
}
```

Errors: `400` bad/empty id-or-label, `400` invalid cell (domain/mode not in the
semiotic grid), `404` unknown id.

### `POST /api/v1/coherence/delete`

Remove a node **and** every structure referencing it: dedupe index, holarchy
node + edges, persisted sled row.

```json
{ "id": "<node_id>" }
```

`404` for an unknown id; cross-tenant deletes always `404` (isolation).

### `POST /api/v1/search/nodes`

Fixed-token-cost search over the tenant's coherence **and** goal nodes.

```json
{ "query": "spindle bearing", "token_budget": 600, "top_k": 12 }
```

The query is embedded once; node/goal labels are ranked by cosine, then greedily
packed so **`total_tokens ≤ token_budget`** regardless of store size — the same
contract as `/api/v1/rag/retrieve`, applied to the live node store instead of
on-disk files. The token counter is the deterministic `HeuristicTokenizer`
(`rag::count_tokens`), so the cost is predictable with no model files.

Response:

```json
{
  "query": "spindle bearing",
  "budget": 600,
  "total_tokens": 42,
  "truncated": false,
  "results": [
    { "kind": "node", "id": "...", "label": "milling spindle bearing wear", "score": 0.9, "tokens": 7 }
  ]
}
```

`kind` is `"node"` (coherence) or `"goal"`. Unlabeled goals (pure vectors) are
skipped — nothing browsable to return.

## Implementation

- `src/models.rs` — `CoherenceNode.cell_pin: Option<(String, String)>`
  (`serde(default)`, so legacy rows load).
- `src/core.rs` — `PhysisCore::edit_node(id, label, embedding, pin, embedder)`
  and `PhysisCore::delete_node(id)`; `IdBridge::unlink`/`unlink_raw`; tests in
  `core::tests`.
- `src/web.rs` — `coherence_edit_handler`, `coherence_delete_handler`,
  `search_nodes_handler`; routes `/api/v1/coherence/edit`, `/delete`,
  `/api/v1/search/nodes`.
- `tests/http_surface.rs` — integration tests: edit preserves identity+verdict,
  pin + invalid-cell rejection, delete cleanup + tenancy, token budget bound.

## Why this matters

Editing a node no longer means deleting and re-adding it. A task registered
optimistically, an OEE anomaly promoted to a coherence node, a scanned file's
label — all can be **re-corrected in place**, and every consumer that holds the
id (dream replay, PDCA, holarchy edges, persistence) keeps working through the
swap. And because search is token-fixed, a caller always knows the cost of the
answer before it is asked.
