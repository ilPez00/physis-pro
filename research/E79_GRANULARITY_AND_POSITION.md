# E79 — scanning one object at several granularities, with position as metadata

E78 gave each *revision* of an object one vector and watched the set move with
time. That discards two axes: **granularity** (document / section / paragraph /
window) and **position inside the text**. E79 scans all of them, tags every
vector with its metadata, and asks three questions that can each fail.

Instrument: `physis-core/examples/reference_spectrum.rs` now passes a `meta`
object through untouched beside `x`, so any axis — granularity, position,
commit, author, subsystem — travels with the vector.
Harness: `benchmarks/conatus/e79_granularity.py`. Embedder: `onnx-explicit`,
bge-base-en-v1.5, dim 768, max_len 512.

Two objects, deliberately different in genre:

* `docs/WHAT_PHYSIS_IS.md` — a **reference** document: nine sections that each
  restate the same system from a different angle.
* `docs/plans/2026-09-14-world-map-machine.md` — a **developmental** document:
  full form, then subtraction, then the experiment that survives.

Controls: a *near* text (another physis document) and a *far* text (sourdough
baking). Without the far control there is no scale for "different", and a ratio
of 1.1 reads as dissolution when it may be two documents that genuinely overlap.

## A. Nesting — is the coarse vector near the mean of the fine ones?

| granularity | doc ↔ own pooled | doc ↔ other doc's pooled | |
|---|---:|---:|---|
| section | 0.1887 | 0.2136 | nested |
| paragraph | 0.2380 | 0.2615 | nested |
| window | 0.2601 | 0.2795 | nested |

Nested at every level, by a thin margin (0.02–0.03). Granularities share one
space and can be mixed in one index — but the coarse vector is not a summary of
the fine ones in any strong sense; it is merely closer to its own than to a
stranger's.

## B. Position — does a text have a direction along its own length?

Spearman(position, distance from the opening unit), against a 2000-permutation
shuffled-position null.

| granularity | reference doc | developmental doc |
|---|---|---|
| section | +0.083, null [−0.683, +0.667] — **no direction** | **+0.665**, null [−0.538, +0.566] — **directed** |
| paragraph | +0.120, null [−0.219, +0.260] — no direction | +0.122, null [−0.255, +0.250] — no direction |
| window | +0.040, null [−0.459, +0.492] — no direction | **+0.476**, null [−0.384, +0.376] — **directed** |

**Positional direction exists, and it is a property of the genre, not of text.**
The document that develops an argument moves away from its opening as it
proceeds; the document that restates one system from nine angles returns to
where it started. Paragraph level is at the null in both — too fine a unit to
carry the movement.

This is the asymmetry worth keeping: E78 found strong *temporal* drift (5 of 6
objects, ρ up to +0.976) because revisions accumulate. Position only drifts when
the text is an argument rather than a description.

## C. Scale — where does the object stop being distinguishable?

| granularity | within | near doc | ratio | far doc | ratio |
|---|---:|---:|---:|---:|---:|
| section | 0.3028 | 0.3402 | 1.12 | 0.4827 | **1.59** |
| paragraph | 0.3793 | 0.4045 | 1.07 | 0.4789 | **1.26** |
| window | 0.2960 | 0.3376 | 1.14 | 0.4703 | **1.59** |

(The developmental document gives 1.08/1.03/1.13 near and 1.65/1.27/1.77 far.)

**Sub-document units carry topic, not document identity.** A paragraph of one
physis document is nearly as far from another paragraph of the same document
(0.379) as from a paragraph of a different physis document (0.405) — but clearly
closer than to a paragraph about bread (0.479). E78's identity result — a file's
revisions as a tight region — does **not** transfer to the parts of a document.
Whatever "the same object" means at revision scale, it is not recoverable from a
paragraph.

## What this instrument now makes testable, and what it does not

The reading list maps onto measurements that this scanner can now carry. None of
these have been run; they are named so the next instalment has a target rather
than an intuition.

| anchor | what it would require | status |
|---|---|---|
| **Allen**, interval relations | units with extent, which sections and windows now have (start, end); and validity intervals on revisions | instrument ready, **untested** |
| **Reichenbach**, S / R / E | event time separable from ingestion time — `meta` can now carry a content date beside the write time that E77 showed is all the store has | instrument ready, **untested** |
| **Vendler**, aspectual classes | step-distance signatures per unit: a state should be flat, an achievement a jump | instrument ready, **untested** |
| **Davidson**, events as entities | role-tagged participants in `meta`, then a relational test | needs a role tagger, **not built** |
| **TransE**-style relations | whether *h + r ≈ t* holds for physis's own relations in this space | needs the relation set exported, **not built** |
| **TimeML** | the label set for the Allen test, rather than labels invented here | **not adopted** |
| **Hume → Pearl**, cause | intervention, not correlation. The only place this repository holds interventions is `system run`: argv, exit status, and what was believed before it (E71, E73) | partially available |
| **cause vs reason** | they are different fields already: evidence justifies a claim (reason), a run's exit changes the world (cause). Nothing yet tests whether they are separable from geometry plus context | **untested, and the most interesting one** |
| **Whitehead** | an object as a trajectory rather than a thing is what E78/E79 measure; that is an interpretation of the instrument, not a result | `[PHILOSOPHY]` |

## Limits

Two documents, one repository, one embedder. The far control is a single short
text, so "far" is one sample. Paragraph splitting is `\n\n`, section splitting is
a heading heuristic — both are crude against TimeML-grade annotation. The
nesting margins (0.02–0.03) are small and have no interval estimate; they are
reported as a direction, not a size.
