# Proposal — `/notebook`: a dashboard the user composes

**Status:** proposed, not started. Written 2026-08-29 at the user's request;
nothing in this document is implemented yet.

---

## The problem

Every surface in this product is a composition *we* authored. Pulse decides
which vitals matter. Console decides that seventeen tabs is the right shape of
an operator's day. Demo decides the order of a walkthrough. Each was a
reasonable guess, and each is wrong for somebody:

- A **maintenance lead** wants OEE, the anomaly feed and the causal ledger on
  one screen, and has no use for the corpus themes.
- A **quality engineer** wants the failure trend, the demoted cells, and a
  saved content search for `"no fault found"` — a combination that exists on
  three different pages today.
- An **archivist** wants the file map, place/period facets and lineage, and
  would happily never see a machine.

Today, giving any of them their screen means editing HTML and shipping a
binary. That is the wrong cost for a layout question. And it shows in the
opposite direction too: Console keeps growing tabs because there is nowhere
else to put a panel, so the page that was meant to be the operator's home is
now the product's junk drawer.

A composable notebook turns "which panels, in what order" from a release
decision into a user decision — which is where it belongs, because we do not
know their job and they do.

## What it is

`/notebook` — a page holding an ordered grid of **cells**. A cell is one
widget from a registry, bound to parameters the user chose. Users can add,
remove, reorder and resize cells; name and keep several notebooks; and export
one as JSON to hand to a colleague.

The default notebook, for a user who has never opened the editor, is the
Pulse composition. Nobody starts at a blank page.

## Design decisions

These are the choices that matter; the rest is layout code.

### 1. A widget is a reference, never a script

A notebook document contains widget **ids** and **parameters**, validated
server-side against the registry. It never contains HTML, JavaScript, a URL to
fetch, or a template string.

This is the decision the whole feature stands on. The moment a notebook can
carry a fetch URL or a snippet, a shared notebook becomes an exfiltration
vector: "open my dashboard" would be enough to run a request against the
victim's own authenticated session. Notebooks are meant to be passed around —
that is half their value — so they must be inert by construction rather than
by sanitization.

### 2. The registry is derived from endpoints that already exist

Each entry declares what it reads and how it renders:

```json
{
  "id": "coherence.distribution",
  "title": "Coherence distribution",
  "reads": ["GET /api/v1/coherence/snapshot"],
  "render": "bars",
  "params": {},
  "default_span": 4,
  "writes": false
}
```

Widgets are thin readers over the endpoints the dashboards already use. If a
widget needs an endpoint that does not exist, that is a separate piece of work
with its own justification — not something smuggled in as "a new panel". The
registry ships as one module served as JSON, so the picker, the renderer and
the server validator read exactly one source and cannot drift into three.

### 3. Layout lives on the server, per tenant

`localStorage` is tempting and wrong: a notebook that exists only in one
browser cannot be opened on the machine wired to the projector, cannot be
handed to a colleague, and vanishes with a cache clear. Storage goes in sled
under `notebook:{uid}:{notebook_id}`, tenant-scoped like every other
per-user structure, behind:

```
GET    /api/v1/notebooks              → list (id, title, cells, updated_at)
GET    /api/v1/notebooks/:id          → one notebook
POST   /api/v1/notebooks/:id          → create or replace (validated)
DELETE /api/v1/notebooks/:id
```

### 4. Unknown widgets fail loudly, in place

Registries drift: a widget is renamed, an endpoint is retired, a notebook
written last quarter references something gone. The cell then renders as an
explicit "this widget no longer exists (`oee.rollup.v1`)" card with a remove
button — it does not silently disappear, and it does not blank the notebook.
A layout that quietly loses a panel teaches users not to trust it.

The same applies to a widget whose endpoint returns nothing: every widget
declares its own empty state, phrased as the next action ("no shifts imported
yet — import a CSV from Console"). Half the perceived brokenness in the
current dashboards is panels that look identical when empty and when failing.

### 5. Writes are marked and confirmed

Most widgets read. A few worth having — assert a verdict, pin a cell, run a
dream cycle — write. Those carry `"writes": true`, render with a distinct
border, and confirm before acting. A dashboard is somewhere people click
quickly; a mutation must never be one careless click inside a grid of
read-only tiles.

### 6. One scheduler, not N pollers

Each cell declares a refresh policy (`manual` by default, or an interval with
a floor of 15s). A single page-level scheduler batches due cells per tick and
skips refreshes while the tab is hidden. Twenty widgets each running their own
`setInterval` is how a dashboard becomes the reason the engine is busy.

### 7. Saved queries are what make it more than layout

The highest-value cell type is not a KPI tile — it is a **pinned query**:

- `search.content` with `{query: "\"no fault found\"", place: "line-2"}`
- `files.query` with `{mechanism: "corrected", within_days: 7}`
- `coherence.facet` with `{verdict: "failure"}`

A pinned query is a standing question the user cares about, answered live
every time they open the page. That is the difference between rearranging our
dashboard and building theirs.

## Cell model

```json
{
  "id": "nb_ops",
  "title": "Line 2 — daily",
  "cells": [
    {"widget": "oee.plant",          "span": 6, "height": "m", "params": {}},
    {"widget": "search.content",     "span": 6, "height": "l",
     "params": {"query": "\"no fault found\"", "place": "line-2", "top_k": 8},
     "refresh": {"mode": "interval", "seconds": 60}},
    {"widget": "ledger.recent",      "span": 12, "height": "m", "params": {"within_days": 7}}
  ],
  "updated_at": "2026-08-29T14:00:00Z"
}
```

`span` is 3/4/6/12 of a twelve-column grid; `height` is a preset (s/m/l/xl),
not a pixel value, so a notebook composed on a 27" screen still reads on a
laptop. Reordering is drag-and-drop **and** move-up/move-down buttons —
drag-only is an accessibility failure, and it is also unusable on the tablets
that actually live on a shop floor.

## Widget catalogue for v1

All of these read endpoints that exist today:

| Widget | Reads |
|---|---|
| `coherence.kpis`, `coherence.distribution` | `/coherence/snapshot` |
| `coherence.nodes` | `/coherence/nodes/all` |
| `communities.grid` | `/api/v1/communities` |
| `atlas.mini` | `/api/v1/atlas/graph` |
| `search.content` (pinned query) | `/api/v1/search/content` |
| `ask.box` | `/api/v1/ask` |
| `corpus.themes` | `/api/v1/corpus/summaries` |
| `quality.trend`, `quality.cells` | `/api/v1/quality` |
| `semiotic.heatmap` | `/api/v1/semiotic/heatmap` |
| `oee.plant`, `oee.machine` | `/api/v1/oee/*` |
| `shifts.report` | `/api/v1/shifts/report` |
| `ledger.recent`, `files.query` | `/api/v1/files/query` |
| `timeline.operational` | `/api/v1/operational/timeline` |
| `interest.feed` | daemon endpoint (degrades when absent) |
| `wiki.tour` | `/api/v1/wiki/tour` |
| `packs.status` | `/api/v1/ontology/packs` |
| `planner.scenarios` | `/api/v1/planner/scenarios/:id` |

Renderers are shared and few — `kpis`, `bars`, `list`, `table`, `heat`,
`graph`, `text`, `feed`. A widget picks one; nobody writes bespoke markup per
widget, which is what keeps the catalogue cheap to extend.

## Build order

| # | Step | Test |
|---|---|---|
| N1 | Registry module + `GET /api/v1/notebooks/registry`; server-side validation of a posted notebook (unknown widget → 400 naming it) | unit: validation accepts the catalogue, rejects unknown ids, rejects params outside the declared schema |
| N2 | Persistence: sled per tenant, CRUD endpoints | e2e: create → list → read → replace → delete, plus tenant isolation |
| N3 | `/notebook` page: render an ordered grid from a notebook document, shared renderers, per-widget empty and error states | page served self-contained; a notebook with an unknown widget renders the explicit card |
| N4 | Editor: add (picker), remove, reorder (drag + buttons), resize, rename, multiple notebooks, export/import JSON | e2e over the endpoints; import rejects a document that fails validation |
| N5 | "Add to notebook" affordances on Search, Atlas, Console panels; scheduler with hidden-tab suppression | e2e: pinning a content search from `/search` produces a cell whose params reproduce the query |

Roughly one focused pass per step; N1–N3 is the demo-able core, N4–N5 is what
makes it stick.

## Risks and the honest limits

- **It can become a worse Console.** A user who adds thirty cells has built the
  junk drawer again. Mitigation is a soft cap with a warning, and defaults that
  start from a coherent composition rather than an empty grid.
- **Registry drift is permanent maintenance.** Every endpoint change now has a
  second consumer. The mitigation is that widgets are thin readers with no
  logic of their own; if a widget needs logic, it belongs in the engine.
- **A notebook is not a report.** It is a live view. Exporting a *snapshot*
  (PDF/HTML with the data frozen and timestamped) is a different feature, and
  conflating them would produce a document that silently changes after it was
  circulated. If snapshots are wanted, they go through the existing evidence
  export path, which already stamps provenance.
- **Layout is not insight.** Composability makes the product fit more people;
  it does not make any single answer better. It should not be sold as, or
  prioritized over, work on what the panels actually say.
