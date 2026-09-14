# E78 — states of reference to one object, as a spectrum that moves

The proposal: log the coherent states that refer to the same object, embed each,
and watch how the set moves through the vector field along some variable *x* —
time, or any metadata axis.

That is testable, and it can fail in three independent ways. Each was measured
separately rather than asserted:

1. **IDENTITY** — do states of one object sit closer to each other than to
   another object's states? If not, "the same object" is not a region of the
   field and there is nothing to track.
2. **DIRECTION** — does distance from the first state grow with *x* beyond what
   a shuffled *x* produces? If not, the motion is scatter with a timestamp
   attached.
3. **STRUCTURE** — does the trajectory live in a few dimensions (a spectrum with
   a knee) rather than filling 768 of them like noise?

## Setup

Object = one source file. Its states = its successive git revisions, whole,
chunked at 1500 characters and mean-pooled into one vector per revision.
*x* = commit time. A second file gives the between-object scale.

Embeddings come from the engine's own `embed::select`, through a new example
(`physis-core/examples/reference_spectrum.rs`) that prints the embedder label
beside the vectors — on the random-projection fallback none of this is about
meaning, and the harness stops rather than reporting numbers. All runs below:
`onnx-explicit`, bge-base-en-v1.5, dim 768, max_len 512.

## Results — six objects

| object | revisions | within | between | Spearman(x, drift) | beats shuffled-x | straightness | var in first 2 dims |
|---|---:|---:|---:|---:|---|---:|---:|
| `hypothesis.rs` | 12 | 0.0118 | 0.1639 | **+0.930** | YES | 0.552 | 0.766 |
| `main.rs` | 27 | 0.0289 | 0.1143 | **+0.626** | YES | 0.491 | 0.621 |
| `studio.rs` | 17 | 0.0245 | 0.1507 | **+0.939** | YES | 0.296 | 0.761 |
| `act.rs` | 8 | 0.0256 | 0.1009 | **+0.976** | YES | 0.678 | 0.817 |
| `ontology.rs` | 8 | 0.0321 | 0.1639 | **+0.929** | YES | 0.329 | 0.740 |
| `quality.rs` | 9 | 0.0175 | 0.1281 | +0.250 | **NO** | 0.120 | 0.569 |

Null band for the direction test, per object, from 2000 permutations of *x*
against the same drift values; `hypothesis.rs`'s was [−0.385, +0.748].

**1. Identity: 6/6.** Within-object distances are 4× to 14× smaller than
between-object distances, with non-overlapping spreads. A file's revisions are a
tight region of the field and a different file is elsewhere.

**2. Direction: 5/6.** Five objects' drift tracks commit time far outside the
shuffled-*x* band. The sixth, `quality.rs`, does not — and its straightness of
0.120 says why: it wandered and came back, so distance-from-origin stops being a
clock. That is the honest failure mode of this measure, and it is visible in the
same table rather than averaged away.

**3. Structure: 6/6.** The first two eigenvalues of the centred Gram matrix
carry 0.569–0.817 of the variance. These trajectories are two-dimensional
objects living in a 768-dimensional space: a spectrum with a knee, not noise.

## What this changes

E77 found that the engine's belief store has no temporal semantics — replay is
indexed by write time, and a claim true at t0 and false at t1 cannot be
represented. E78 shows that **the arrow is recoverable from the embedding field
without adding a transition object at all**: the ordered set of references to
one subject already carries a direction, measurable against its own null, in the
two dimensions the trajectory actually uses.

That is a cheaper answer than Phase 25's `Transition { from_state, to_state,
alternatives, selection_criterion … }`: for *"what direction is this subject
moving in"*, the states plus *x* suffice. What the spectrum does **not** give is
the reason for a move — no alternatives, no selection criterion, no cause. Those
remain unmeasured, and Phase 10 and Phase 12 have to run before any of them is
worth building.

## Two defects this experiment produced first

* **The first run embedded only the first 1200 bytes of each revision** — a
  source file's module header, which barely changes. Twelve revisions came out
  as the same vector, within-object distance read 0.0000, and the "spectrum" was
  the constant function. Whole-revision chunking fixed it; the median byte delta
  between revisions is now printed so a static input is visible.
* **The direction null was degenerate in that state.** With every drift value
  identical, Spearman and its permutation band both collapsed to ±1.000, and the
  harness reported "beats its null: NO" from tied ranks rather than from data. It
  now checks the spread first and reports NOT MEASURED.

## Limits

One repository, one kind of object (source files), *x* = commit time only — the
metadata axes the proposal allows (author, branch, subsystem) are untested. Each
revision is truncated to 8 chunks (12000 characters), so the tail of a large
file is invisible to its vector. Six objects is enough to show the effect is not
one file's accident and not enough to characterise when it fails: `quality.rs`
is currently a single counterexample, not a class.

## Reproduction

```sh
cargo build --offline --release -p physis-core --features embed-onnx,cli \
    --example reference_spectrum
python3 benchmarks/conatus/e78_reference_spectrum.py \
    --file src/hypothesis.rs --other src/ontology.rs
```
