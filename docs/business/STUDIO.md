# Ontology Studio — demo quickstart

`physis-pro studio` is the sellable demo wedge: drop a folder of a prospect's
documents in, get back (1) one self-contained HTML they can open in a browser,
(2) a loadable `discovered_ontology.json` that *grows the grid itself*, and
(3) a typed `ontology_graph.json` — the Palantir-like layer.

## Build (needs the optional ingestion backends)

```bash
cargo build --features pdf,email --bin physis-pro
```

Plain text/markdown works without those features; PDF and email extraction are
gated behind `pdf`/`email` so the default build stays light.

## Run

```bash
./target/debug/physis-pro studio <DOCS_DIR> --html-out <DOCS_DIR>/studio.html
```

What it does, in order:
1. **Ingest** every document in the folder — real text + structured metadata
   (PDF → invoice vendor/date/number; email → From/Subject/thread-id). The
   Studio skips its own previously-generated `.html`/`.json` so re-runs stay clean.
2. **Build the file→theme graph** against the live 370-cell ontology (the grids
   a buyer already understands: OEE, maintenance, inventory, finance…).
3. **Discover** what the folder's vocabulary *can't* be named by the current
   grid — these are the gaps a bespoke ontology consultant would charge to model.
4. **Emit** `studio.html` (open it) and `discovered_ontology.json` (drop into
   `EXTRA_ONTOLOGIES` and re-run to watch the grid absorb the new domain).

## What the buyer sees

- A coverage card: how many of their documents already map to a known cell.
- A per-document table proving ingestion worked (real extracted text + fields).
- "Where your ontology is thin": the discovered domains the grid can't name yet.

## The pitch it demonstrates

> Palantir models your ontology by hand, with consultants. physis grows it from
> your own documents — offline, deterministic, and yours to inspect.

## Outputs

| File | What it is |
|------|------------|
| `studio.html` | Self-contained, openable report (coverage, document table, gaps). |
| `discovered_ontology.json` | The discovered domains as loadable grid cells (`EXTRA_ONTOLOGIES`). |
| `ontology_graph.json` | The **typed** graph — nodes tagged `theme` / `document` / `concept`, links tagged `maps_to` / `discovered` / … with property bags. The Palantir-like substrate, inspectable and diff-able. |
| `ontology_events.json` | The **provenance ledger** — every discovery/merge recorded with a deterministic `seq`, the affected node, and the source documents. Audit trail of what the ontology learned, and from what. |

The typed graph is the real differentiator: instead of one flat "theme↔file
score" edge, every relationship has a named type and every node carries
free-form attributes. Re-run after loading `discovered_ontology.json` and the
new Concept nodes become first-class Theme nodes — the grid visibly learns.

## Try it

```bash
mkdir -p /tmp/demo && cp <some-invoices-and-emails> /tmp/demo
./target/debug/physis-pro studio /tmp/demo --html-out /tmp/demo/studio.html
xdg-open /tmp/demo/studio.html
```

Add an off-grid document (e.g. a cybersecurity or HR report) to watch discovery
surface a brand-new domain, then load `discovered_ontology.json` to prove the
grid learns.
