# Physis, re-scoped

> **Superseded as a definition by `docs/WHAT_PHYSIS_IS.md`.** This file is the
> record of a measurement pass and of three wrong compressions made during it
> (§0 → §7 → §8). It is kept because the corrections are the useful part. For
> what Physis *is*, read the canonical file, not §0.


Written 2026-09-12, against the code as measured that day, not as documented.
Every number here was produced by a command in this repository during the pass
that wrote it. Claims without a run behind them are labelled as open.

---

## 0. The one-paragraph version

**Revised 2026-09-12, later the same day. The first version of this section said
"Physis is a context compiler." That was wrong, and the correction is the most
useful thing in this document — see §7.**

Physis is an **epistemic ledger for agent work**: a record of what was believed,
when, on what evidence, what it predicted, and whether the prediction held —
with point-in-time replay over that record. It contains a context compiler, and
the compiler is the *commodity* half: several exist, one of them is MIT-licensed
and beats physis on physis's own measured terms. The ledger is the half nothing
else ships. The thesis worth betting the next year on is that an agent working
against a ledger stops rediscovering what it already established and stops
confidently repeating what it already refuted — and that this is measurable.

---

## 1. What is actually true today

### Shipped and measured

| Capability | Evidence |
|---|---|
| Token-fixed retrieval — context provably fits the window at any corpus size | `src/rag.rs`, `TokenFixedRetriever` |
| Deterministic structural map — repeat families, differences, contradiction pairs | same corpus ⇒ same `structure_hash` |
| Context compression | 220 → 165 tokens (25%) on ground truth; 284 → 197 (30.6%) on the demo corpus |
| Structure recovery on synthetic ground truth | repeat 100% (3/3), anomaly 100% (2/2), contradiction 50% (1/2) |
| Interchangeable model and n-gram backends | `ModelProvider` + `NGramTable` traits, checksum registry, `PHYSISNG1` format, 338k lookups/s, 14.8 KB on disk |
| Config-driven pipeline | `RunConfig` swaps model and table with no source edit |
| Discovery candidate *filtering* | duplicate control Welch *t* = +12.11; 2000 candidates → 141 |
| Discovery candidate *ranking* | precision@25 = 0.480 vs 0.132 base, exact *p* = 2.3e-5 |
| Agent surface | 30 MCP tools over stdio, verified live |
| Test suite | physis-core 194 pass; clippy clean at `-D warnings` |

### Settled negatives — earned, and not to be relitigated

`research/RESEARCH_STATUS.md` records a multi-day controlled program, E1–E18.
These are results, and the re-scope must be built on top of them rather than
around them:

- Embedding geometry does **not** supply stable ontology structure. Cross-embedder
  ARI ≈ 0.10; a 25% corpus increase replaced half the anchors.
- No cheap signal certifies discovered structure. On real misfilings every
  scorer sits at chance (cosine 0.519, CONSTRAINT 0.536, ANCESTRY 0.534).
- Cross-model disagreement carries no epistemic signal. Refuted at *t* = −10.53,
  0/22 configurations; the mechanism is known — `|s1 − s2|` is a folded read-out
  of `support_2`, and the folding costs ~0.10 AUC.
- The machine cannot judge whether an entry *belongs*. AUC 0.410, chance inside
  the CI.

What a second embedder buys is a better baseline, nothing more: BGE-base
`support_2` alone scores 0.866, +0.132 over MiniLM, 22/22 configurations.

**The standing rule of this track, earned seven times: treat any positive as an
artifact until a construction-matched control says otherwise.** That rule is the
most valuable thing in the repository and it should govern the re-scope.

---

## 2. What this pass found, and why it changes the scope

### 2.1 The CLI had never used a semantic embedder

`load_embedder()` in `physis-core/src/main.rs` returned
`RandomProjectionEmbedder::new(384)` unconditionally. `cmd_context`, `cmd_demo`,
`cmd_benchmark` and `cmd_run` each built their own
`RandomProjectionEmbedder::new(64)`. `OnnxEmbedder` was exported from `lib.rs`
and called by nothing.

Every number in `benchmarks/results/` was produced by a lexical hash.

Fixed: `embed::select` now resolves a cascade — explicit override, then ONNX
candidates each gated on `semantic_self_test`, then random projection *labelled
as such* — and `provenance.json` records which embedder produced the run.

### 2.2 The benchmark cannot fail

With a real embedder finally available, the obvious control became possible:
run the same benchmark twice, once per embedder.

| Metric | random-projection | MiniLM (ONNX) |
|---|---|---|
| Repeat recovery | 1.00 | 1.00 |
| Anomaly recall | 1.00 | 1.00 |
| Contradiction recall | 0.50 | 0.50 |
| Context reduction | 25.0% | 24.0% |
| Baseline → physis tokens | 220 → 165 | 221 → 168 |

Identical. The synthetic corpus's repeat families are lexically near-identical,
so string overlap recovers them exactly as well as a sentence transformer does.

**A benchmark whose score does not move when retrieval quality moves is not
measuring retrieval quality.** This is the dog/puppy failure one level up: the
repository fixed a self-test that could not fail (PH-004) while running a
benchmark that could not fail.

Consequence: no context-quality claim in this repository currently has evidence
behind it. Not because the claim is false — because the instrument was blind.

### 2.3 An instrument that does discriminate

Source code, queried in a developer's words, is the discriminating corpus the
synthetic one is not. The question says "how does the engine refuse a model that
loads but produces nonsense"; the answer says `semantic_self_test`,
`is_available`, `MARGIN`. Almost no shared tokens.

Same query, same 657 candidate chunks under `physis-core/src`:

| arm | top result | correct |
|---|---|---|
| random-projection | `delta_engine.rs:401-440`, score **0.923** | no |
| ONNX MiniLM | `embed.rs:321-360`, score **0.764** | **yes** |

The wrong answer came back with higher confidence. Same shape as H2: more
confident, no more correct.

Built this pass: `benchmarks/retrieval/` — ten such questions with the answering
file established by reading the code, scored `hit@k`, run per arm, printing
`DISCRIMINATION: semantic − rp` and saying so when the difference is zero.

**It discriminates.** Seven cases in scope under `physis-core/src`, budget 3000,
top-k 30, hit@5:

| arm | hit@5 | wall clock |
|---|---|---|
| random-projection | **0.14** (1/7) | 0.5 s |
| ONNX MiniLM | **0.57** (4/7) | 657 s cold, 9.7–15.5 s per query warm |

`DISCRIMINATION: semantic − rp = +0.43`. This is the first measurement in the
repository whose value depends on retrieval quality. It is also the honest
ceiling report: 0.57 is not good, and three of seven questions are still missed
by a real model. That is a number worth improving against, which is more than
the 100%/100%/24% row ever offered.

The cost profile matters for every downstream arm: `pack` carries a persistent
content-addressed `EmbedCache`, so the first pack over a tree pays for the whole
corpus (583 s for 657 chunks) and subsequent queries pay ~12 s. Warm the cache
once per corpus, not once per question.

### 2.4 The cost of being semantic was 6× larger than it needed to be

- `max_length` was 512 where the documents are short: MiniLM measures 18.3
  ms/embed at 128 versus 108.1 ms at 512, a 5.9× tax paid entirely in padding.
- `compile_context` calls `retrieve_from_texts` twice and `build_map` once over
  the same corpus — roughly 800 embed calls for ~30 distinct texts.

Fixed both (`MemoEmbedder`, `max_length` 128). `context` over the 30-document
corpus: **86.2 s → 14.3 s**. The same padding tax is still unpaid on the
physis_pro side (`src/embed/registry.rs` sets 512), where one `pack` over 657
chunks took **376 s**.

---

## 3. The thesis, stated so it can be killed

> Physis plus a small model plus a table imitates a big model.

As written this is not testable — "imitate" has no metric and "big model" has no
referent. The testable form:

> **On tasks whose difficulty is knowledge-location rather than reasoning-depth,
> a small model reading a physis-compiled context reaches the task-success rate
> of a large model reading the naive context, at a fraction of the token cost.**

The qualifier is not a hedge, it is the substance. Retrieval can substitute for
knowledge stored in weights. It cannot substitute for reasoning depth or for
latent skills. A claim that does not carve at that joint will be refuted by the
first task that needs five reasoning steps, and the refutation will be
uninformative.

### The 2×2 that settles it

|  | naive context | physis-compiled context |
|---|---|---|
| **large model** | **B** — the ceiling | **A** — does compression lose the answer? |
| **small model** | **D** — the control | **C** — the claim |

- **A ≈ B** says compression is lossless for the task. Without it, C means nothing.
- **C ≈ B** is the thesis.
- **C ≈ D** says physis contributed nothing and the task was easy. This is the
  control the research track's standing rule demands, and it is the one that
  gets skipped.

Today: A is reachable (the agent is the large model, `physis_pack` is the
compiler). B, C, D are not built. The oracle leg — the transport, four probe
cases, token-Jaccard agreement, evidence-citation check — exists in
`physis-core/src/oracle.rs` and reports `not_configured`.

**Therefore the thesis is currently unsupported by anything in this repository.
It is the hypothesis under test.**

### What a positive would be worth

The commercial question in every AI deployment in 2026 is "can a cheaper model
do this?" and nobody can answer it with evidence. A tool that answers it *per
query*, with a measured control, is worth more than an ontology engine. That is
the maximal expression: not a better model, a **decision procedure about which
model is enough** — and the credibility to be believed, which comes from a
repository that visibly kills its own positives.

---

## 4. Re-scope

### Ring 0 — the asset (harden, keep small, Apache)

The context compiler and the honesty machinery around it. `rag.rs`, `map.rs`,
`embed.rs`, `bench.rs`, `oracle.rs`, `model_provider.rs`, `ngram_table.rs`.

Work: raise `hit@5` from 0.57; make `benchmarks/retrieval/` the gate that must
discriminate before any context claim ships; keep the `NOT MEASURED` label
mandatory.

Note on the physis_pro padding tax: `src/embed/registry.rs` hardcodes
`max_length: 512` for BGE where `config.rs` defaults to 128. Unlike the core
fix, cutting it here is **not** free — pro packs source-code chunks that are
genuinely long, so truncating to 128 tokens would drop content. The cost is
already paid by `pack_cached`'s persistent cache. Leave it.

### Ring 1 — the experiment (the actual product of the next months)

Build B, C and D. Concretely:

1. Configure the oracle leg. An OpenAI-compatible endpoint is all it needs;
   a local ollama at `http://localhost:11434/v1` works for arm C and D, and is
   honestly a *peer*, not an oracle — labelling a 4B local model a big-model
   reference is the exact dishonesty the gate exists to prevent.
2. Add the naive-context arm to `bench.rs`. Today `oracle_cases` feeds the
   oracle the *compiled* context only, so the benchmark can measure agreement
   but not loss. One extra call per case closes it.
3. Score on `packets/` — this repository's own task suite, where each packet's
   `gate:` line is objective pass/fail that no participant can argue with.

### Ring 2 — the maximal expression

The compiler becomes a **router**: per query, decide which model tier can answer
from the compiled context, and report the confidence with the control attached.
Output is not an answer, it is an answer plus "a 4B model was sufficient here,
and here is the paired run that shows it."

This is reachable only if Ring 1 returns a positive. If Ring 1 returns a
negative, that is also a result — the research track has produced seven of them
and is stronger for it — and the scope collapses back to Ring 0 as an honest
context compiler, which is a real product on its own.

### What to stop

- ~~Treating the semiotic grid as a discovery mechanism.~~ **Struck. This was
  my error, not a finding — see §8.** E1-E18 refuted *certification*, and
  `propose.rs` had already recorded why that does not generalise.
- Quoting `benchmarks/ground-truth` numbers as retrieval evidence until that
  corpus contains items semantically related and lexically disjoint.
- New research branches while `main` sits 181 commits behind `agent/PH-001`.

---

## 5. The dogfood loop

The corpus is this repository. The tasks are `packets/`. The gate is the
packet's own `gate:` line. The agent is the large model. Setup and arms:
`docs/AGENT_DOGFOOD.md`.

The loop is not a demo. It is the cheapest available source of task instances
whose success is objectively checkable, in a domain where the answer is
knowledge-location — which is precisely the class the thesis claims.

---

## 6. Open, ranked

Struck items were closed later the same day; kept visible so the list reads as a
record rather than a tidy plan.

1. ~~**Put `transform` in front of a null.**~~ **Done — E53, SUPPORTED,
   replicated.** Link prediction over the `crate::` dependency graph: top-3
   0.500 vs a degree-preserving null of 0.209 (core, n=86) and 0.446 vs 0.256
   (pro, n=294). On the permuted graph analogy *loses* to popularity on both
   corpora, which is the arms behaving as designed.
   Licenses **ranking**, not generation — hence item 2.
2. **Does the path GENERATE?** E54: delete a load-bearing `use crate::T::{…}`,
   verify the build goes red, have the method supply `T`, and let `cargo check`
   judge. Binary gate, external, no partial credit. This is the experiment that
   decides whether draft-and-fill is a product or a demo.
3. **Build arms B, C, D** of the §3 2×2. Nothing else changes what is known
   about the small-model thesis.
4. **Merge `agent/PH-001` into `main`.** Still the largest single-point-of-
   failure in the repository.
5. **PH-017** — `list` and `replay` disagree about hypothesis status; the ledger
   is the differentiator and it currently contradicts itself.
6. **PH-015** — the content-search flake poisons every full gate, and PH-009
   (790 s suite) cannot even be measured until the suite finishes.
7. **Contradiction recall 0.50.** One of two known pairs missed, unchanged by
   the embedder, so it is a map defect rather than a retrieval one.
8. **403 unreferenced public items** in physis_pro, 112 in physis-core
   (`docs/wiki/UNREFERENCED-2026-09-12.txt`). A maintenance-surface measurement,
   not a defect count.

---

## 7. Correction: the context compiler is the commodity

Written after §0–§6, in response to the right question: *there are already a lot
of context compilers — what makes physis special?*

### 7.1 Concede the compiler

`github.com/Emmimal/context-compiler` (MIT, pure Python, ~12 commits) does
static import-graph reachability plus AST skeletonization: trace symbols, strip
function bodies to signatures, sort files into full / skeletonized / excluded.
Reported: **69–74% token reduction in 43–73 ms**.

Physis, measured this week: **24–30% reduction**, ~12 s per query warm, 583 s
cold. On Python source, a static reachability prior beats embedding retrieval on
physis's own metric by a factor of roughly three, about four orders of magnitude
faster, with no model, no weights and no cache.

That is not a comparison physis wins by arguing. Two honest observations rather
than a defence:

- Its token count is a `chars ÷ 4` heuristic, and it reports reduction but never
  whether the answer survived — the same hole as `benchmarks/ground-truth`
  before `benchmarks/retrieval` existed. A physis contribution here is the
  harness, not the compressor.
- The two priors are **complementary, not competing**. Reachability answers
  "what code does this touch" and is blind to prose, decisions, and history.
  Embedding retrieval answers "what is this about" and is blind to call graphs.
  A compiler that takes the static graph as a prior and re-ranks semantically
  should beat both.

**So: yes, copy it.** MIT, small, and the mechanism is orthogonal to what is
here. That is Ring 0 work and it is the correct call. It is also not a moat, for
physis or for them — it is 12 commits of AST walking, and anyone can write it.

### 7.2 What is not copyable, demonstrated live

While testing whether the epistemic modules were real machinery or another
`OnnxEmbedder` — exported, never called — the graph answered the question by
itself.

A hypothesis registered **2026-09-05**, seven days before this session:

> *Retrieval, not the write path, is the binding constraint on this loop:
> random-projection relevance is too weak to surface the right prior claim, so
> rediscovery will keep happening even once claims are registered.*

Supporting evidence attached to it, same day: an unrelated query about *"stripe
payment webhooks in the praxis backend"* scored 0.830 cosine against a claim
about `OntologyDeltaReport` and ranked it **first of two**.

This session, with no knowledge of that record, independently found: the CLI had
never used a semantic embedder; random projection returns the wrong file at
*higher* confidence than the right one (0.923 wrong vs 0.764 right);
`hit@5` 0.14 → 0.57 when fixed.

The same finding, seven days apart, rediscovered from scratch — which is
precisely what the 2026-09-05 hypothesis predicted would keep happening.

What the ledger then permitted, and what no context compiler can do:

| action | result |
|---|---|
| Attach this session's measurement to the 7-day-old claim | fitness 0.587 → 0.603, empirical 0.85 → 0.91 |
| Register a new hypothesis, then refute it with a construction-matched control | `Contradicted`, fitness 0.281 |
| `replay --at 2026-09-12T16:13:30Z` | `Candidate` — the belief that held before the refutation |
| `replay` now | `Contradicted` |
| `hypothesis open` | a prediction from 2026-09-05 still pending: *"Across the first 20 tasks, rediscovery will be recorded true more often than plan_changed."* |

A context compiler answers *what should the model read now*. This answers *what
did we believe, when, on what evidence, what did it predict, and did the
prediction hold* — and can be asked as of any past instant.

### 7.3 Do not overclaim it either

Truth-maintenance systems are 1980s AI — de Kleer's ATMS, Doyle's TMS. Temporal
knowledge graphs ship today (Graphiti, Zep). The machinery is not new and the
repository already knows this: `BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md` maps T1–T18.

What is not shipped elsewhere, as far as this pass found: a TMS whose subject is
**an agent's own development loop**, with predictions that get *resolved*, in a
repository that runs itself against it. Every agent framework has RAG memory.
None of them record a prediction and come back to score it. External evidence
that RAG memory alone is insufficient: `opencode-orchestrator` retired its own
hybrid-RAG + decay memory in ADR-0019.

And the ledger has a real defect, found in the same hour: `list` reports a status
derived from fitness while `replay` reconstructs it from the event log, and they
**disagree on the live graph** — `Supported` versus `Candidate`, same hypothesis,
same second. `FitnessShifted` and `OutcomeObserved` are declared in the event
enum and emitted by nothing. Filed as `packets/PH-017`. A record that omits the
transitions it did not personally perform returns a belief the system never held,
which is the one failure a ledger may not have.

### 7.4 The re-scope, re-scoped

| ring | before | after |
|---|---|---|
| 0 | context compiler | context compiler — **absorb the static prior; stop treating it as the moat** |
| 1 | the compiled/naive × small/large 2×2 | unchanged, and now instrumented by the ledger rather than by hand |
| 2 | a model-tier router | **an agent memory that can be wrong and know it** |

The maximal expression is not "physis imitates a big model." It is: *an agent
that does not rediscover what it established last week, and does not repeat what
it refuted.* The §3 2×2 stays — it is how you prove the compiler half is not
losing answers — but it stops being the headline.

The measurement changes accordingly. Not `hit@5` alone, but **rediscovery rate**:
across N packets, how often does the agent re-derive a finding already in the
ledger? The 2026-09-05 prediction already named the metric and is still open.
This session is one data point for it, and the data point is: **rediscovered.**

---

## 8. Correction: I compressed the system twice, and both were wrong

§0 first said "Physis is a context compiler." §7 said "no — an epistemic
ledger." Both were reductions of a **190-module, ~113k-line system**, made by
reading part of it. The full inventory, taken after the second correction:

### 8.1 physis-core — 46 modules, 22.7k lines, Apache-2.0

| group | modules |
|---|---|
| Classification & ontology | `classify` `ontology` `models` `semiotic_grid` |
| **Discovery** | `discovery` `coverage` `linkage` `becoming` |
| **Proposal** | `propose` |
| **Invariance & structure** | `transform` `transplant` `structsim` `map` |
| Epistemics | `hypothesis` `contradiction` `epistemic` `provenance` `temporal` `relation` `explanation` `coherence_dimensions` `coherence_query` `delta_engine` `dream` |
| Retrieval & models | `rag` `embed` `embed_onnx` `embed_ngram` `model_provider` `ngram_table` `tokenizer` |
| Surfaces | `studio` `studio_lab` `studio_communities` `bench` `oracle` `quality` `store` `config_run` |
| Importers | `vault` `history` `praxis` `process` |

### 8.2 physis_pro — 140+ modules, ~90k lines, proprietary

22 **philosophical machines** (`chronos` `topos` `techne` `euclid` `elenchus`
`apodeixis` `empedocles` `krisis` `plutos` `polis` `lex` `pathos` `kinesis`
`soma` `harmony` `chroma` `erosa` `erosp` …) · **industrial** (`oee`
`maintenance` `industrial` `moonraker` `shifts` `report` `secom` benchmark) ·
**backoffice** (`reconcile` `inventory` `batch_csv` `review_desk`
`management_pack`) · **knowledge** (`wiki_gen` `corpus` `doctree` `communities`
`decisions` `sessions` `grounding`) · **agent** (`mcp` `ai/provider`
`ai/harness` `ai/agent` `ai/tools` `ai/memory` `routes` `extract`) ·
**space·time·cause** (`address` `ledger` `mails` `audit` `operational`) ·
**linguistic lenses** (`wenyan` `piraha` `sanskrit`) · **personal** (`life_os`
`study` `interest` `monitor` `praxis`) · `pack` `content` `trie` `graph`
`salience` `quantize` `planner` `axes` `context` `brain` `notebook`.

### 8.3 The specific error

I wrote "stop treating the semiotic grid as a discovery mechanism — E1–E18
settled that." **E1–E18 refuted certification, not discovery**, and `propose.rs`
had already written down why the two do not generalise to each other, in the
module header, before this session started:

> All of those asked the machine to **judge**: *does this entry belong here?*
> This module asks it to **propose**: *which few cells should a person look at
> first?* Same geometry, different question, and the second one works.

Measured against the same blind re-filing, top-k containment of the cell a human
actually chose, centroids built only from decisions already made (E21):

| at 436 decisions seen | top-1 | top-3 | top-5 |
|---|---|---|---|
| **`propose`** | 0.447 | **0.712** | 0.791 |
| `old_cell`→`new_cell` table | 0.184 | 0.368 | 0.474 |
| majority cell | 0.089 | 0.255 | 0.359 |
| **permuted labels (null)** | 0.036 | 0.136 | 0.233 |
| entry length alone | 0.010 | 0.036 | 0.071 |

+0.531 top-3 over a construction-matched null, +0.299 over a lookup table, curve
still climbing when the data ran out. And the sharpest line in the module:
**top-1 ≈ 0.45 reproduces an accuracy this project had already recorded as a
failure. The geometry did not improve. Returning three candidates instead of one
is the entire difference.**

That is the intellectual move the whole research track turns on, and I repeated
a conclusion the codebase had already superseded.

### 8.4 What else I under-weighted

- **Invariance finding.** `transform.rs` is a symbolic token-level homomorphism
  engine — deterministic, **no embeddings**, patterns matched by exact token
  equality with variable binding, transforms gated by a coherence profile. It
  does not inherit the embedding negatives at all. `transplant.rs` ports the
  *shape* of one project onto another; `structsim.rs` compares JSON, DB schemas
  and class hierarchies. This is a second, independent epistemic mechanism that
  §0–§7 never mentioned.
- **Discovery proper.** `discovery` proposes entries for uncovered texts;
  `coverage` says which proposals change what the ontology can *place* (Welch
  *t* = +12.11, 2000 → 141); `linkage` reports which cells real data bridges;
  `becoming` separates "a term's meaning changed" from "one name covers two
  things" by looking at *when* each occurrence happened — order-aware where
  every other similarity method here is order-blind.
- **Token savings on debugging.** Reported from real use, not measured here.
  It belongs in `benchmarks/retrieval/` as a case class, not in a document as an
  anecdote — and it is the concrete lead worth instrumenting next.

### 8.5 The standing lesson

A system this size cannot be summarised into one noun, and every attempt above
produced a confident wrong answer. The honest description is structural, not
sloganeering: **physis is a set of epistemic primitives over a semiotic
coordinate system, plus the applications built on them, plus a research
discipline that refuses each primitive until a construction-matched control
survives.** The context compiler is one primitive. The ledger is another. So are
proposal, coverage, becoming, transform and transplant.

The re-scope question was "what is its maximal expression". The answer is not a
smaller physis with a sharper slogan. It is: **which of these primitives already
beat a construction-matched null, and which have never been asked?** Three
already do — `propose` (+0.531 top-3), `coverage` (*t* = +12.11), discovery
ranking (P@25 0.480 vs 0.132). `transform` and `transplant` have never been put
in front of a null at all. That is the gap worth closing before any re-scope.
