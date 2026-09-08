# E2 — The mapper hypothesis, tested

`research/mapper/` · run 2026-09-08 · reproduce with

```
cargo run -p physis-core --features embed-onnx --release --example mapper_corpus_export
python3 research/mapper/run_e2.py     --tokenizer <Qwen3-8B tokenizer.json>
python3 research/mapper/emendation.py <Qwen3-8B tokenizer.json>
```

**Hypothesis under test, independent of E1 and not predicted by it:**

> Language can be treated initially as a relational representational topology
> rather than a fixed semantic ontology; sparse grounding and corrective
> interaction can progressively constrain that topology into a more useful map
> of externally observable phenomena.

**Decisive criterion, fixed before running:** not whether the map becomes
internally coherent, but whether progressive grounding makes it increasingly
predictive of something *outside* the map.

**The external target.** The human-authored 5×14 grid cell of a **held-out**
entry. It is authored by people against `docs/GRID_AXES.md`, is not derivable
from the text distribution by construction, and no arm ever sees a test entry's
label. 658 entries, 70 occupied cells, 30% held out; the majority cell covers
**0.102** of the test half, which is the floor any arm must beat.

---

## A. Technical implementation — how the dictionary works

`Qwen/Qwen3-8B/tokenizer.json`, 11.4 MB. **Tokenizer only: no weights were
downloaded, no inference was ever run.**

| | |
|---|---|
| alphabet | bytes — Qwen3 is a byte-level BPE, 256 roots |
| unit | one BPE subword token |
| context | the pre-tokenizer regex splits words/punctuation first, so a token never spans a word boundary except through the leading-space marker |
| vocabulary | **151,643** entries |
| merge rules | **151,387** — the derivation DAG |
| stored per entry | token id, surface form, **merge rank**, **merge parents** |
| lookup returns | the token ids of a text, and through the DAG their derivation ancestors |

**What makes this Qwen-derived rather than generic**, and it is the whole point
of the arm:

- **Merge rank** is a statistic of *Qwen's training corpus*, not of ours. Merges
  are stored in the order BPE learned them, so a late rank means the string only
  became worth merging after a lot of text — a rarer, more specific unit. It is
  an IDF-shaped weight readable off Qwen without estimating anything from 658
  short entries, which is exactly what a corpus this small cannot do for itself.
- **The merge DAG** is a relation Qwen supplies: every token above the 256 byte
  roots was *constructed* from two earlier tokens.

A tokenizer-only baseline has the vocabulary but neither. A corpus-local n-gram
model has none of the three. That separation is what arms A / B / C test.

The corpus tokenizes to 14,811 tokens, 5,852 distinct, mean 22.5 per entry.

## B. Map representation

| level | definition | pruning | size |
|---|---|---|---|
| **L0** | Qwen tokens present in the corpus, weighted by merge rank | df ≥ 2 — co-occurrence is undefined for a token seen once | 2,357 nodes |
| **L0+** | L0 plus each token's merge-DAG ancestors at half weight, depth ≤ 4 | as L0 | 31,636 added incidences |
| **L1** | pairwise association = **positive PMI** of token co-occurrence across entries | PPMI ≥ 0 | 170,944 edges |
| **L2** | relations between relational profiles = cosine between L1 rows | > 0.05 reported | 753,116 edges |

Entry representation at each level is its token vector propagated through that
level's matrix, then reduced to 256 dimensions by truncated SVD so every arm is
compared at identical dimensionality.

## C. Grounding protocol

An anchor is exactly `(entry index → cell string)`. Nothing else. The human never
edits the map, never writes a rule, never touches a relation — they pin one
point. Two propagation rules were tried, both identical across arms so that any
difference is the *representation*:

1. **cosine k-NN to the anchors** (k=5, similarity-weighted vote)
2. **label propagation over the map graph** — Zhou normalized-Laplacian on a
   10-NN entry graph, α = 0.9. This is "pinning data points" stated literally:
   labels diffuse through the *unlabelled* structure.

## D. Emendation protocol

A correction replaces one already-pinned anchor's label with the true one — the
same operation as a grounding, applied to a point already pinned. 160 anchors,
40 deliberately mislabelled with a uniformly-sampled wrong cell, then corrected
one at a time, 10 replicates.

---

## E. Ablation results — full supervision (461 anchors), held-out accuracy

| arm | k-NN | propagation |
|---|---:|---:|
| A tokenizer-only (Qwen vocab, no merges) | 0.315 | 0.193 |
| B ordinary char 3–5 gram (**no Qwen**) | **0.355** | 0.228 |
| C Qwen dictionary, L0 rank-weighted | 0.340 | 0.254 |
| C+ Qwen dictionary + merge-DAG ancestors | 0.228 | 0.147 |
| L1 Qwen dictionary + PPMI relations | **0.355** | **0.274** |
| L2 relations between relations | 0.279 | 0.188 |
| **E existing embedding (BGE-base)** | **0.447** | **0.299** |
| CONTROL shuffled map | 0.056 | 0.117 |
| CONTROL random gaussian | 0.051 | 0.091 |
| FORMAT CONTROL Qwen L0, names only | 0.322 | 0.279 |
| FORMAT CONTROL BGE, names only | 0.401 | 0.320 |
| *majority-class floor* | *0.102* | *0.102* |

**The controls hold.** A shuffled map and a random gaussian map sit at or near
the floor under k-NN (0.056, 0.051). Everything above that is representation,
not propagation rule.

**The Qwen dictionary beats the tokenizer alone but not an ordinary n-gram
model.** C 0.340 against A 0.315 — the merge-rank weighting is worth about
+0.025, so Qwen's corpus statistics do carry something. But B, plain character
3–5 grams computed from this corpus with no Qwen anywhere, reaches 0.355 and
beats it. The Qwen-derived weight does not buy what a generic local n-gram model
supplies for free.

**The merge DAG is actively harmful.** C+ collapses to 0.228 from C's 0.340.
Derivation ancestors are generic substrings shared by unrelated words; expanding
each token into them washes out the signal. This is a clean negative on the most
distinctively "Qwen-structural" component of the design.

**Nothing built here approaches the existing representation.** BGE-base reaches
0.447 against the best Qwen arm's 0.355.

## F. Higher-order ablation — does correlation-of-correlations contribute?

| level | k-NN | propagation |
|---|---:|---:|
| L0 | 0.340 | 0.254 |
| **L1** | **0.355** | **0.274** |
| L2 | 0.279 | 0.188 |

**L1 helps slightly and consistently** (+0.015 / +0.020). **L2 hurts, and by
more than L1 helped** (−0.076 / −0.086). Higher-order relations were not
constructed past L2, because the ladder's own stop condition fired.

**The pre-registered caveat applies and must not be dropped.** This corpus is
658 short entries. L1 pairwise statistics off that are already sparse and L2 is
sparser by roughly a square. **The L2 result is a sample-size finding, not
evidence that second-order structure cannot help.** A corpus two or three orders
of magnitude larger could reverse it, and this experiment cannot say.

## G. Sparse grounding — the mapper question

Held-out accuracy as anchors are added (label propagation; k-NN curves are in
`e2_curves.json` and behave the same way):

| arm | 1 | 5 | 10 | **20** | 40 | 80 | 160 | 461 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| C Qwen L0 | 0.053 | 0.077 | 0.082 | 0.092 | 0.129 | 0.193 | 0.239 | 0.254 |
| L1 Qwen + PPMI | 0.053 | 0.081 | 0.082 | 0.107 | 0.151 | 0.204 | 0.249 | 0.274 |
| E BGE-base | 0.053 | 0.086 | 0.089 | 0.132 | 0.181 | 0.222 | 0.275 | 0.299 |
| *floor* | *0.102* | *0.102* | *0.102* | *0.102* | | | | |

**Twenty anchors do not beat guessing the largest cell.** At k = 20 every
Qwen arm is at or below the 0.102 majority floor; only BGE clears it, at 0.132.

Fraction of the full-supervision gain captured by the first 20 anchors — 4.3% of
the available labels:

| arm | captured at k=20 |
|---|---:|
| L1 Qwen + PPMI | 3.2% |
| C Qwen L0 | −6.1% |
| A tokenizer-only | −21.8% |
| E BGE-base | 15.2% |

**The mapper hypothesis predicts this number is large.** It is not. The best
arm in the whole experiment converts 4.3% of the labels into 15.2% of the gain —
mildly superlinear, entirely ordinary for a learning curve on a representation
with real structure, and nowhere near "10 anchors ≈ 1000 anchors". Every Qwen
arm is at or below proportional.

**Label propagation over the map graph is worse than ignoring the map.**

| arm | k-NN | propagation | Δ |
|---|---:|---:|---:|
| E BGE-base | 0.447 | 0.299 | **−0.147** |
| B ordinary n-gram | 0.355 | 0.228 | −0.127 |
| C Qwen L0 | 0.340 | 0.254 | −0.086 |
| CONTROL shuffled | 0.056 | 0.117 | **+0.061** |
| CONTROL random | 0.051 | 0.091 | **+0.041** |

This is the most diagnostic row in the report. Diffusing labels through the map
*helps the structureless controls* and *hurts every representation that carries
signal*. The map's topology is not a channel the labels travel along; it is a
smoother, and smoothing a real signal costs accuracy. The mechanism the
hypothesis proposes — pin points, let structure carry them — measurably does not
work here.

## H. Error propagation and emendation

160 anchors, 40 mislabelled, corrected one at a time:

| representation | corrections | held-out acc | local region | far region | held-out entries fixed per correction |
|---|---:|---:|---:|---:|---:|
| C Qwen L0 | 0 | 0.192 | 0.047 | 0.244 | — |
| | 10 | 0.195 | 0.047 | 0.229 | 0.05 |
| | 40 | 0.229 | — | 0.229 | 0.18 |
| L1 Qwen + PPMI | 0 | 0.209 | 0.066 | 0.262 | — |
| | 20 | 0.226 | 0.082 | 0.247 | 0.17 |
| | 40 | 0.254 | — | 0.254 | 0.22 |
| E BGE-base | 0 | 0.245 | 0.086 | 0.299 | — |
| | 20 | 0.260 | 0.097 | 0.285 | 0.16 |
| | 40 | 0.308 | — | 0.308 | 0.31 |

**Corrections repair only what they directly touch.** Efficiency is 0.05–0.31
held-out entries fixed per correction — one human correction fixes *less than one*
held-out prediction, and often needs three to twenty corrections to fix one.

**Corrupted regions stay corrupted until corrected individually.** The "local"
column — held-out entries whose nearest anchor is still mislabelled — sits at
0.03–0.10 throughout, near the floor, and does not improve as neighbouring
anchors get fixed.

**No catastrophic distortion**, which is a genuine positive but a weak one: the
"far" column is flat (0.244→0.229, 0.262→0.254, 0.299→0.308). Corrections do not
damage unrelated structure — because they do not propagate at all.

The brief's distinction between *emendation* and *simple relabelling* is exactly
what fails. What was measured is relabelling.

## I. Negative results, recorded

1. Merge-DAG ancestor expansion: −0.112 against L0. The most distinctively
   Qwen-structural component is the most harmful one.
2. L2 correlations-of-correlations: −0.076 against L1 (sample-size caveat above).
3. Label propagation over the map graph: worse than ignoring the map, for every
   arm carrying signal.
4. Sparse grounding: no disproportionate gain; 20 anchors fail to beat the
   majority-class floor for every Qwen arm.
5. Emendation: corrections do not propagate; efficiency below one entry per
   correction.
6. Qwen dictionary vs ordinary character n-grams: 0.340 against 0.355. Loses.

## What was NOT run, and why

- **Ablation D, "Qwen dictionary + Physis"** — the map was never passed into
  Physis's hypothesis / coherence / provenance machinery. Arms A, B, C, C+, L1,
  L2 and E all use the same k-NN or propagation rule, so what is reported is
  *representation quality*, not Physis. This is a real gap in coverage and the
  report does not claim otherwise. It was not run because the ladder's stop
  conditions fired at rungs 3–5: the representation does not beat an ordinary
  n-gram model, higher order does not help, and sparse grounding shows no
  disproportionate gain. Adding an interpretive layer on top of that is the
  "add more machinery to a null" move the brief forbids.
- **Multiple competing interpretations maintained under uncertainty** (§5) —
  same reason.

## Format control — and a result that differs from Stage 13

Stage 13 found a single embedder reading authorial hint format as a semantic
stratum, with separations collapsing 20–150× when hints were dropped. That
threat was carried into this design from the start. It **did not reproduce
here**: names-only reaches 0.279 against 0.254 with hints for the Qwen arm
(names-only is *better*), and 0.320 against 0.299 for BGE. Hints add little to
cell prediction, and what they add is not a format artifact. The pre-registered
prediction that the curve would plateau on a house-style axis is **not
supported** — the plateau is lower than that, at the representation's ordinary
ceiling.

---

## Verdict

```
MAPPER HYPOTHESIS NOT SUPPORTED
```

The measurements that justify it, in order of weight:

1. **Sparse grounding produces no disproportionate gain.** 4.3% of the labels
   buy 3.2% of the gain for the best Qwen arm and 15.2% for BGE. Twenty anchors
   do not beat guessing the largest cell for any Qwen arm. The decisive
   prediction of the hypothesis is not observed.
2. **The map's topology does not carry labels.** Propagation through it is worse
   than ignoring it for every arm with signal (−0.081 to −0.147) and *better*
   than nothing only for the random controls.
3. **Corrections do not propagate.** 0.05–0.31 held-out entries repaired per
   correction; corrupted regions stay corrupted until touched individually. This
   is relabelling, not emendation.
4. **The Qwen-derived structure does not clear the simpler baselines it must.**
   It beats tokenizer-only by +0.025 and loses to ordinary character n-grams by
   −0.015, and the merge DAG — the most Qwen-specific element — costs −0.112.

What this does **not** establish: that a relational linguistic representation
can never function as a map, that higher-order structure is useless in general
(the L2 result is sample-size-bound at n=658), or that Qwen n-gram structure is
without value for other tasks. It establishes that **on this benchmark, with
this dictionary, these map levels and this grounding protocol, the mapper
mechanism did not appear.**

## Next

Not a rescue of this design. **BGE-base remains the strongest representation
this repo has for grid prediction** (0.447 against every constructed
alternative).

*The paragraph that stood here claimed the failure was a data problem and that a
larger corpus was the fix. E3/E4 tested that with 24× the text and refuted it —
see the section above. The claim is left struck rather than deleted, because a
diagnosis that survives one round and dies in the next is worth being able to
find later.*

---

## E3 / E4 — the density diagnosis, tested and REFUTED

E2 closed with a diagnosis rather than a measurement:

> *"on a 658-entry corpus spread over 70 cells, structure-based diffusion has
> too little density to work. Any future version of this hypothesis needs a
> corpus large enough for L1 and L2 to be estimated, and that is a data problem
> before it is an architecture problem."*

**That was wrong, and this section is the test that shows it.**

### What was added

| corpus | units | how |
|---|---:|---|
| labelled entries (evaluation, unchanged) | 658 | human-authored cells |
| physis codebase | 5,237 | chunked, embedded, classified by physis's **own** `CellClassifier` |
| document library (`~/Documenti`) | 10,167 | 342 files, 21.1 M characters — philosophy, ebooks, sociology, semiotics, phenomenology |

Volume rose 24×, and the *variation* rose far more: the codebase is one register
written by one team; the library is natural prose across a dozen subjects.

**Privacy, applied not assumed.** Directories that are personal, financial,
medical or private correspondence were excluded by default, with a filename
filter for WhatsApp exports, invoices, contracts, CVs, ID and bank documents.
No document text and no filenames are committed: the artifact carries only a
machine-assigned cell, its score, a coarse top-level tag, and the vector.

**Circularity avoided.** The cells physis assigns to the codebase and the
documents are MACHINE-PRODUCED. They are never evaluated against and never used
as anchors. The evaluation set stays the 658 human-authored cells on E2's
identical split.

### Leg A — denser relations made every level worse

L1 positive edges went from 170,944 to **21,268,718 (124×)**; matrix density
0.031 → 0.332. The relations really did get denser.

| level | PPMI from 658 entries | PPMI from the union | Δ |
|---|---:|---:|---:|
| C Qwen L0 | 0.340 | 0.203 | **−0.137** |
| L1 Qwen + PPMI | 0.355 | 0.183 | **−0.173** |
| L2 relations-of-relations | 0.279 | **0.066** | **−0.213** |

L2 on the union corpus scores **0.066, below the 0.102 majority floor** — worse
than guessing the largest cell.

### The two confounds, separated before attributing anything

The union arm changed two things at once: relations got denser, and the
vocabulary changed (2,357 ontology-drawn tokens → the 8,000 most frequent over a
corpus of philosophy and Rust). Holding the vocabulary **fixed** at the original
tokens and estimating PPMI on the union isolates them:

| arm | k=20 | k=80 | k=320 | full |
|---|---:|---:|---:|---:|
| (a) original vocabulary, PPMI from 658 | 0.080 | 0.128 | 0.210 | **0.218** |
| (b) original vocabulary, PPMI from union | 0.068 | 0.118 | 0.157 | 0.188 |
| (c) union vocabulary, PPMI from union | 0.075 | 0.110 | 0.142 | 0.183 |

**(a) → (b) is density alone: −0.030.** Same features, relations estimated from
24× more text, and it gets worse. **(b) → (c) is vocabulary displacement:
−0.005.** So the collapse is primarily *density*, not the vocabulary cap. This
is the opposite of the E2 diagnosis, measured on the arm the diagnosis named.

### Leg B — node density, and why one arm could not benefit

| arm | k=20 | k=160 | full |
|---|---:|---:|---:|
| BGE, 658 nodes only | 0.133 | 0.248 | 0.299 |
| BGE + codebase nodes | 0.129 | 0.289 | **0.350** |
| BGE + documents | 0.129 | 0.260 | 0.315 |
| BGE + codebase + documents | 0.127 | 0.280 | 0.345 |
| L1 Qwen union, 658 nodes only | 0.090 | 0.142 | 0.168 |
| L1 Qwen union + codebase nodes | 0.090 | 0.142 | **0.168** |

Two things here.

**BGE gains from unlabelled substrate, but only where anchors are already
plentiful** — +0.051 at full supervision, **nothing at k=20** (0.133 → 0.129).
Extra nodes help a dense problem, not a sparse one. That is the opposite of what
the mapper hypothesis needs.

**The Qwen rows are byte-identical with and without the extra nodes**, which
looked like a bug and is not. Counting edges in the 10-NN graph:

| representation | labelled→extra edges | labelled→labelled | cross share |
|---|---:|---:|---:|
| BGE | 1,293 | 7,296 | **15.1%** |
| Qwen L1 union | **10** | 12,000 | **0.1%** |

The Qwen n-gram map places ontology entries and prose chunks in effectively
disjoint regions. Labels cannot diffuse through vertices nothing connects to, so
the null for that arm is a **property of the representation**, not a wiring
error — and it is a sharper statement of E2's finding than E2 managed.

### What this changes

**E2's verdict stands and its caveat is now closed.** The verdict was
`MAPPER HYPOTHESIS NOT SUPPORTED`; the caveat was that it might be a data
problem. It is not. 24× the text, from a corpus vastly more varied than the
original, estimated 124× more relations — and every level got worse, with the
highest-order level falling below the trivial floor.

**The replacement explanation, which the data supports and the old one did
not:** the relations that help are the ones estimated *within the target
domain*. PPMI over 21 M characters of philosophy and Rust encodes the
co-occurrence structure of philosophy and Rust, which is orthogonal to what
distinguishes `HEAL/LIFT` from `STUDY/SENSE`. **Domain-matched sparse statistics
beat domain-mismatched dense ones**, and adding text from outside the domain
actively dilutes the signal rather than sharpening it.

The pre-registered caveat that the L2 null was "sample-size bound" also does not
survive: given 24× the sample, L2 did not recover, it collapsed further.

---

## Artifacts (for the physis webview)

| file | what it holds |
|---|---|
| `corpus.json` | 658 labelled entries: name, hints, human-authored cell, category/axis, BGE vector, and a name-only BGE vector for the format control |
| `e2_curves.json` | every grounding curve in this report — both propagation rules, all arms, all anchor counts, with the majority floor |
| `e3_curves.json` | the E3/E4 density re-test |
| `docs_corpus.json` | document-library chunks — **no text, no filenames**: machine cell, score, coarse tag, length |
| `docs_vecs.f32` | raw f32, 768 per document chunk |
| `selfcorpus.json` | physis's own cell assignment for each codebase chunk — **machine-produced, not ground truth** |
| `selfcorpus_vecs.f32` | raw little-endian f32, 768 per chunk, in chunk order |

Published pages: **Mapper Ladder** (this experiment) and **E1 Divergence Inspector**
(the preceding one).
