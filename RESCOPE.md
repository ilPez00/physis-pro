# Physis, re-scoped

Written 2026-09-12, against the code as measured that day, not as documented.
Every number here was produced by a command in this repository during the pass
that wrote it. Claims without a run behind them are labelled as open.

---

## 0. The one-paragraph version

Physis is a **context compiler**: corpus plus query in, the minimal token set
that answers the query out, under a hard ceiling, deterministically. Everything
else in this repository — the semiotic grid, the holarchy, the PDCA loop, the
dream generator, the epistemic revision machinery — is either an application of
that compiler or an experiment about it. The compiler is the asset. The thesis
worth betting the next year on is that a *small* model reading a compiled
context can match a *large* model reading everything, on the class of tasks
where difficulty is knowledge-location rather than reasoning-depth. That thesis
is currently **untested**, and this document says exactly what it would take to
test it.

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

Work: pay the padding tax on the physis_pro side; make `benchmarks/retrieval/`
the gate that must discriminate before any context claim ships; keep the
`NOT MEASURED` label mandatory.

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

- Treating the semiotic grid as a discovery mechanism. E1–E18 settled that.
  It is a useful *filing* structure; it is not an epistemic instrument.
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

1. **Build arms B, C, D.** Nothing else changes what is known.
2. **Pay the physis_pro padding tax.** 376 s per `pack` makes every agent arm
   impractical; the fix is the one already applied in core.
3. **Merge `agent/PH-001` into `main`.** 181 commits of work on one branch.
4. **PH-015** — the content-search flake poisons every full gate, and PH-009
   (790 s suite) cannot even be measured until the suite finishes.
5. **Contradiction recall 0.50.** One of two known pairs missed, unchanged by
   the embedder, so it is a map defect rather than a retrieval one.
