# What we know — right, wrong, open, working

One page, updated 2026-09-14. Every line names its number and where to check it.
Longer records: `physis-core/research/E*.md`, `research/RESEARCH_STATUS.md`,
`PLAN.md` §25.

## Right — measured against a control that could have failed

| claim | number | where |
|---|---|---|
| Order in a text carries recoverable structure | transitions 0.486 vs shuffled null 0.291, z = +4.88 | E56 |
| Structure + order + similarity beats every single leg | 0.700 vs ~0.013 each | E57 |
| A word-level search function matches a 7B interpreter on real text | BM25 0.535 vs 0.488 (docs); 0.447 vs 0.465 (log) | E63 |
| A counted word table matches the transformer on filtered arms | Δ ≤ 0.031 top-1, **2400× cheaper to build** | E64 |
| Which filter to use can be measured from the corpus | margin correct on all 3 corpora where the arms differ | E65 |
| Both grid axes carve | every class clears its own null; 5-axis 0.672/0.209, 14-axis 0.536/0.121 | E66, D4 |
| `propose` works where `judge` failed | top-3 0.712 vs null 0.136 | §4 of `WHAT_PHYSIS_IS.md` |
| Structure-derived analogy beats a degree-preserving control | +0.291 / +0.189 top-3 | E53 |
| Coverage and discovery ranking beat their nulls | Welch *t* = +12.11; P@25 0.480 vs 0.132 | RESCOPE §1 |
| Semantic retrieval beats lexical on the honest benchmark | hit@5 0.57 vs 0.14 | `benchmarks/retrieval/` |

## Wrong — settled negatives, not to be relitigated

| claim | why it died | where |
|---|---|---|
| Embedding geometry yields stable ontology structure | cross-embedder ARI ≈ 0.10 | E12–E18 |
| Structure can be *certified* cheaply | every scorer at chance on real misfilings (0.519/0.536/0.534) | E14–E18 |
| Cross-model disagreement carries epistemic signal | paired *t* = −10.53, 0/22 | E1 |
| The machine can judge whether an entry belongs | AUC 0.410 | RESEARCH_STATUS |
| Cosine separates agreement from contradiction | precision 0.667 with perfect recall; MemStrata AUROC 0.59 | C2, arXiv 2606.26511 |
| Independent structure transfers across representations | matcher at its null (0.014 vs 0.028) | E55 |
| Call the model only where cheap rules fail | 0.326 vs 0.488 full coverage | E61, narrowed by E62 |
| A high-recall link rule is safe | 0.700 → 0.013 under redundancy | E58, narrowed by E59 |
| Merging modes fixes the mode axis | −0.187 at matched granularity | E27 |
| The mode axis expands on proposal alone | 6 of 14 fail an external verb inventory | D2a |

## Open — believed, not measured

- **Cost of the interpreter.** 388 calls ≈ 8 min; nothing says the gain is worth it.
- **The ontology as proposal.** Domain anchor still needs IOF/SUMO, not WordNet (D1). Mode anchor should be a *tool/action* inventory, not verb supersenses (E66's sharpening of D2a).
- **The revision loop.** `hypothesis.rs` + `TemporalValidity` exist; nothing revises a belief while you work.
- **Cross-corpus transfer of the counted table.** Built on the corpus it navigates; transfer untested (E64).
- **The `ledger` → `build_structural` join.** Both halves exist, still three call sites, none the log.
- **Adversarial corpus.** 4 of 15 required pairs; navigation work is blocked until re-frozen (E57).
- **Everything rests on one machine's log, one week, mostly one repo.** Every interval in the series overlaps its neighbour.

## Working — running now

- **`physis-world`** — observe · inspect · locate · neighbors · history · changed · compare · trace · why · retrieve · copy · paste. 3232 observations, ~2 s, **no neural inference**, filesystem untouched, every answer prints its provenance chain. `docs/WORLD_SHELL.md`.
- **Append-only observation log** with fs / git / shell watchers; duplicates collapsed at read, never deleted.
- **Copy/paste with structure** — text plus `seq`, source, timestamp, links; system clipboard or file, and it says which.
- **Open domain axis** — `PHYSIS_DOMAINS`, `cell_count()`; 5 × 14 is a default pack, not a law.
- **Test suites green** — physis-pro 854 pass, physis-core 317 pass, clippy clean on lib and bins.

## The rule that governs all of it

An item without a control is not a result. A benchmark that cannot fail is a bug
report. A failure comment gets updated, never deleted.
