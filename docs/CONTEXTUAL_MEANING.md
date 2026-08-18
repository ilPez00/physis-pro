# Contextual Meaning — Interpretants, Overlays, and Salience

An ontology entry is a sign. Physis models all three of Peirce's legs
explicitly, and this document covers the two that were added after the first:

| Leg | Where it lives | Answers |
|-----|----------------|---------|
| Signifier | `hints` on a `DomainDef` | What surface forms point at this? |
| Signified | the entry's embedding centroid | What region of meaning is it? |
| **Interpretant** | `interpretant` block on an ontology file | What effect does using it have? |

Two mechanisms build on that third leg:

- **Overlays** scope *what an item means* to a sector, client, or project.
- **Salience** records *what an item has been worth*, and biases classification.

Both are opt-in. With no overlays on disk and no observations recorded, the
engine behaves exactly as it did before.

---

## 1. Why the grid alone is not enough

The base ontologies place 928 entries into a 5 × 14 `DOMAIN × MODE` grid — 70
cells, all populated. Measurement of that arrangement motivated everything
below:

| Measure | Value |
|---|---|
| Mean similarity, random pairs corpus-wide | 0.567 |
| Mean similarity within a cell | 0.620 |
| `STUDY × REST` (n=6) | 0.743 |
| `CONSTRUCT × WORK` (n=14) | 0.560 |
| `BOND × CREATE` (n=15) | **0.549 — less alike than chance** |

Ten of the thirty measurable cells carry no signal. The pattern is consistent:
every coherent cell is small, every large cell is incoherent. `HEAL × LIFT`
holds Barthes' *punctum*, Peirce's *qualisign*, DevOps incident response, and
strength training.

A cell is a coordinate, and coordinates were being used as a filing cabinet.
Overlays and salience add the two axes a coordinate system was missing:
*where you are standing* and *what has worked*.

> **Also worth knowing:** `classification_domains()` covers only `human` +
> `custom`. The other nine maps — machine, semiotic, agent, natural, social,
> category, abstract, engineering, office — are loaded and rendered but never
> scored against. 569 of 928 entries are unreachable by classification. The
> Ontology Studio flags each one. This is a separate, larger lever on
> classification quality than anything in this document.

---

## 2. Interpretants

Each ontology file declares what counts, *in its own universe of discourse*, as
one of its signs having meant something. Outcomes bind to the existing
`CoherenceRating` vocabulary, so the rule stays deterministic and auditable —
the same reason the scoring path avoids an LLM.

```json
{
  "kind": "machine",
  "interpretant": {
    "confirms": ["success", "inert"],
    "refutes": ["failure"],
    "half_life_days": 0,
    "weight": 1.0,
    "note": "an inspection that found nothing still confirms the routine"
  },
  "domains": [ ... ]
}
```

| Field | Default | Meaning |
|---|---|---|
| `confirms` | `["success"]` | Outcomes that raise salience |
| `refutes` | `["failure"]` | Outcomes that lower it |
| `half_life_days` | `90` | Days for salience to halve. `0` disables decay |
| `weight` | `1.0` | Influence on ranking. `0` records without biasing |
| `note` | — | What confirmation means here, shown in the studio |

Unknown outcome names are rejected at load with the offending string named, so
a typo like `"succeeded"` cannot sit in config silently matching nothing.

### Declared examples

Three ontologies ship with genuinely divergent readings — the proof the
abstraction earns its keep:

| Ontology | confirms | refutes | half-life | why |
|---|---|---|---|---|
| `machine` | success + **inert** | failure | **0** | A check that found nothing still confirms the routine. Plant reality changes slowly. |
| `software_dev` | success | failure | 60d | Inert is *not* confirmation — a change that ran and did nothing is no evidence the frame was right. |
| `health_fitness` | success | **nothing** | 21d | A missed week is absence of evidence, not evidence of a wrong frame. |

`machine` and `software_dev` read `inert` in opposite directions. No single
global rule can express that.

### Declared versus inherited

`interpretant` is `Option`, not a defaulted struct. Absence survives
deserialisation on purpose: the studio reports an undeclared interpretant as an
open question rather than a decision. Currently 3 declared, 30 inheriting.

Interpretants are keyed by each ontology's own name, not by the domain map it
lands in — all 20 extras share `custom_domains`, and bucket-keying would give
twenty universes of discourse one shared rule.

---

## 3. Overlays — what an item means *here*

`Margin` is a machining allowance to a manufacturing client and a gross-profit
line to a publisher. A single global ontology has to pick one, and picking one
is what collapses distinct meanings into a single cell.

An overlay is an ontology scoped to a sector, client, or project. Resolution
walks narrowest to broadest, first definition wins:

```
project  →  client  →  sector  →  global
```

An overlay **shadows only the names it mentions**. A client overlay with six
entries does not cost you the other nine hundred.

### Layout

```
~/.physis/ontologies/
  sector/manufacturing.json
  client/evotec.json
  client/publisher.json
  project/line3-retrofit.json
```

The file format is an ordinary ontology, `interpretant` block included — so a
client can declare that for its work, an inert outcome still confirms.

### Selecting a context

```bash
export PHYSIS_CONTEXT="sector:manufacturing,client:evotec,project:line3-retrofit"
```

Both `key:value` and `key=value` parse. Unknown keys and empty values are
ignored rather than silently producing a context. An empty context resolves
exactly like the base ontology.

### Worked example

`Release` with four overlays loaded:

| Context | Cell | Reading |
|---|---|---|
| `sector:manufacturing` | FABRICATE × MOVE | release to production |
| `client:evotec` | HEAL × GUIDE | batch release, QP sign-off |
| `client:publisher` | CONSTRUCT × CREATE | publication date |
| `project:line3-retrofit` | FABRICATE × MOVE | hand the line back to operations |

### API

```bash
# Overlays present, plus names defined in more than one place.
curl localhost:19876/api/v1/ontology/meaning

# What one name means in every context that defines it.
curl "localhost:19876/api/v1/ontology/meaning?name=Margin"
```

The Ontology Studio tab in `/ui` renders both, alongside per-source reachability
and facet coverage.

---

## 4. Salience — what an item has been worth

An entry starts as an assertion by whoever authored it. Salience is what it has
since earned.

### The signal is work, not self-report

| File event | Signal | Outcome |
|---|---|---|
| Edited after classification | `Edited` | `success` |
| Deleted | `Deleted` | `failure` |
| Present at a later sweep | `Retained` | `inert` |

Editing a file classified into a cell is evidence the frame was useful;
deleting it is evidence it was not. Both are emitted by work the user was doing
anyway. Asking for a grade per classification would yield a trickle of
judgements biased toward the moments the user felt like grading.

Two deliberate omissions:

- **New files are not observed.** Nothing was classified there yet, so there is
  no prior frame to confirm.
- **Unchanged files are not recorded as `Retained` by `/watch`.** Doing so would
  make salience a function of how often the operator polls, not of anything that
  happened in the world. Survival is already captured by the absence of a
  deletion. The `Retained` signal exists for callers with a real retention
  event to report.

Note that `Retained` maps to `inert`, so **whether survival confirms is the
ontology's call** — the machine ontology says an inspection that found nothing
confirms the routine; the software ontology says a change that did nothing does
not.

### It biases the score

A well-confirmed entry can beat a semantically closer one. That is real
adaptation, and also how a system entrenches its own past, so the shift is
bounded:

```
bias = BIAS_STRENGTH · weight · tanh(salience / 8)      BIAS_STRENGTH = 0.05
```

Measured separation between a coherent cell and a random pair is about 0.15, so
a 0.05 cap reorders genuine close calls and cannot overturn a clear semantic
win. Two tests pin exactly that pair of properties; either alone would be
worthless, since the first passes with an unbounded bias and the second with no
bias at all.

Saturation matters as much as the cap: ten net confirmations buy ~76% of the
available bias, a hundred do not buy ten times more. Without it, one
heavily-edited file would pin a cell permanently.

### Scoping and decay

Salience is keyed by **context** and by **entry**.

- By context, so evidence accumulated for one client never biases another's
  classification. The same entry name means different things in different
  contexts, so what confirmed it in one is not evidence about the other.
- By entry rather than cell, because spreading one file's evidence across a
  cell's members — up to 87 of them — would record nothing about anything.

Decay is applied lazily on read from `updated_at`; the engine has no scheduler
to sweep with. Recording decays what is already there *before* adding, so a
year-old `+10` is not treated as current when today's `+1` arrives.

Stored at `~/.physis/salience.json`, written atomically (temp file, fsync,
rename) for the same reason as the embedding cache: a truncating write that
fails leaves an empty file and silently discards every observation ever made.

### Observing it

```bash
# Sweep a directory; `observed` counts observations folded in.
curl -X POST localhost:19876/api/v1/watch \
  -H 'Content-Type: application/json' \
  -d '{"dir":"/abs/dir"}'
# → { new, changed, deleted, registered, removed, total_nodes, observed }

# Ranked salience for the ambient context.
curl localhost:19876/api/v1/ontology/meaning | jq '.salience'
```

Worked run: registering two files observed 0; editing them observed 2; six
edits of one file accrued `Incident Response +6.00 (6 observations)` while the
other sat at `Strength Training +1.00 (1)`; deleting a file recorded a
refutation.

---

## 5. Watch cache scoping

`NetworkScanner` kept **one** hash cache for every watched directory, keyed by
basename. Sweeping directory B therefore reported all of directory A's files as
deleted — they are simply absent from this sweep. That already dropped their
coherence nodes, and once the interpretant loop landed it would have recorded a
refutation against every entry those files ever matched (measured: 114 spurious
deletions). Two files sharing a basename across directories collided outright.

Each directory now gets its own cache, `watch_hash_cache_<hash>.json`. Watching
`src` reports 0 deleted where it previously reported 114.

---

## 6. Limits

- **569 of 928 entries remain unreachable** by classification. Overlays and
  salience do not address this; it is the largest single lever on classification
  quality.
- **0 of 928 entries carry any facet.** The orthogonal axes
  (lifecycle / agency / scale / abstraction / sub-domain / sub-mode) are defined
  in the type system, queryable from the CLI, and aggregated per cell — over
  data that does not exist. Every `physis-pro facet` query returns zero.
- The cell-coherence measurement covered 30 of 70 cells, limited to those with
  at least three cached embeddings.
- Cosine on BGE-base has a compressed dynamic range. Treat 0.567 as the
  baseline, not 0.5.
