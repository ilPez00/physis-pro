# Ontology Expansion — Domains, Modes & Facets

The 5×14 semiotic grid (HEAL/CONSTRUCT/FABRICATE/BOND/STUDY × 14 modes) is a
**base layer**, not a complete map of action. It is a curated, human-activity
slice and was never meant to classify "whatever possible action." This doc lays
out (1) the facet schema we added to carry dimensions the grid cannot, and (2)
a proposed *expanded* taxonomy — offered as additive packs/trees, never by
breaking the 5×14 grid.

## Why not just add more domains/modes to the grid?

Exhaustiveness over all possible action is infinite and counterproductive: the
classifier scores by cosine over the entries in each cell, and the 70 cells are
already mostly empty. More axes → more empty cells → worse discrimination. The
grid is a **2-D projection of an N-dimensional action space**; the missing
dimensions belong in facets and in axis-kind packs, not in the grid itself.

## Facet schema (implemented)

Carried on every `OntologyEntry` / `DomainDef` / `GridPosition` as
`facets: Facets` (all fields optional, `#[serde(default)]` ⇒ back-compatible).

| Facet | Type | Meaning |
|-------|------|---------|
| `sub_domain` | `Option<String>` | Tree-nest under a domain, e.g. `"Restore"` under `HEAL`. |
| `sub_mode` | `Option<String>` | Tree-nest under a mode, e.g. `"Focused"` under `WORK`. |
| `lifecycle` | `Option<LifecyclePhase>` | `Design \| Build \| Operate \| Retire` — *when* in a process, orthogonal to manner (mode). Stops "phase" overloading a mode. |
| `agency` | `Option<Agency>` | `Self \| Other \| Automated \| Collective` — who executes. |
| `scale` | `Option<Scale>` | `Personal \| Interpersonal \| Organizational \| Civil` — scope. |
| `abstraction` | `Option<Abstraction>` | `Concrete \| Abstract` — grain. |

`CellClassifier` aggregates member facets per cell (`Facets::aggregate` —
majority vote over enums, most-common over sub-strings) into `CellScore.facets`,
and `classify_filtered(embedding, &FacetFilter)` up-weights cells whose facets
match a caller-supplied constraint. The base vector scoring is unchanged, so
downstream behavior is preserved when no filter is given.

### Example entry (JSON)

```json
{
  "name": "Pump Maintenance",
  "category": "reliability",
  "domain": "CONSTRUCT",
  "mode": "MAINTAIN",
  "axis_kind": "machine",
  "axis_name": "operational",
  "unit": "tasks",
  "hints": ["lubricate", "inspect", "replace seal"],
  "facets": {
    "sub_domain": "Repair",
    "sub_mode": "Preventive",
    "lifecycle": "Operate",
    "agency": "Automated",
    "scale": "Organizational",
    "abstraction": "Concrete"
  }
}
```

## Proposed expanded taxonomy (additive)

The grid stays at 5×14. Expansion happens two ways:

1. **Axis-kind packs** (already supported): `office`, `machine`, `agent` packs
   reuse the DOMAIN×MODE scaffold with their own vocabularies. Grow these
   instead of bloating the human grid.
2. **Domain/mode trees**: sub-domains nest under a domain, sub-modes under a
   mode. A cell becomes a *path*; the 70-grid is just depth-2.

### Fuller domain set (proposal — add as packs, not the base enum)

The base 5 cover production/relation/cognition. A universal action space needs
at least:

- `HEAL` (wholeness), `BOND` (connection), `STUDY` (truth) — keep.
- `CONSTRUCT` (structure), `FABRICATE` (craft) — keep.
- **`GOVERN`** — rule, law, adjudicate, allocate. *(missing: no civic/legal sphere)*
- **`EXCHANGE`** — trade, buy/sell, negotiate value. *(missing: commerce ≠ production)*
- **`EXPRESS`** — communicate, signal, art-as-language. *(missing)*
- **`DEFEND`** — secure, protect, pre-empt harm (vs HEAL's post-hoc restore). *(missing)*
- **`MEANING`** — ritual, worship, transcend, commemorate. *(missing)*
- **`EXPLORE`** — navigate, orient, discover the unknown. *(missing)*

### Fuller mode set (proposal)

The 14 modes cover execution/creation/perception. Add:

- **`DECIDE`** / **`WILL`** — agency & intention (vs WORK the execution).
- **`REASON`** — deliberate, infer (vs LEARN acquire, BRAINSTORM generate).
- **`REMEMBER`** — recall, archive.
- **`MEASURE`** — assess, diagnose (vs SENSE perceive).
- **`TEACH`** — impart (vs GUIDE steer).
- **`PERSUADE`** — negotiate, convince.
- **`IMAGINE`** — envision pre-creation.
- **`SHARE`** — give, distribute.
- **`PROTEST`** — resist, rebuke.
- **`CELEBRATE`** — mourn, ritualize affect.

### Canonical ordering (view hint, not a constraint)

Order domains by a needs pyramid
`HEAL < BOND < STUDY < CONSTRUCT < FABRICATE < GOVERN < EXCHANGE < EXPRESS < DEFEND < MEANING < EXPLORE`
and modes by rising agency/energy
`REST < SENSE < REMEMBER < LIFT < MOVE < WORK < DECIDE < REASON < CREATE < IMAGINE < TEACH < GUIDE < PLAY < BRAINSTORM < MAINTAIN < PLAN < PROTEST < CELEBRATE`.
The ordering drives heatmap/browse order; the engine ignores position and
scores by cosine.

## Migration

- Existing ontology JSON without `facets` parses unchanged (`#[serde(default)]`).
- `GridPosition::cell_index` / `SemioticGrid` stay 5×14 — the base grid is
  untouched; expansion rides on facets + axis-kind packs.
- Classification callers get `CellScore.facets` for free and may opt into
  `classify_filtered` for facet-aware ranking.
