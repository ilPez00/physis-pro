# Small encoders: where BERT/DeBERTa replaces an embedder or an LLM — 2026-09-19

The question is not "can a small model do this". It is "which of our many models
is doing a job a 20–180 M parameter encoder does better, faster and measurably".
This note inventories what runs today, names the candidates, and says how each
one would be judged — because every replacement here is a measurement, not a
preference.

## 1. What runs today

| model | size | job | where |
|---|---|---|---|
| `bge-base-en-v1.5` (ONNX) | 110 M, 768-d | semantic embeddings | the default embedder |
| `bge-m3` (ONNX) | 568 M, 1024-d | multilingual embeddings | when its weights are present |
| `nomic-embed-text` (ollama) | 137 M, 768-d | semantic embeddings | the ollama leg, tried first |
| Jina (quant) | — | embeddings | known to fail its self-test; kept as a documented casualty |
| random projection | 0 | deterministic fallback | offline mode |
| `oczen/nemotron-3.5-lightning-free` | hosted | agent text, judge, brainstorm | the Zen transport |
| the decision endpoint | hosted | typed decisions (noul/choice/score) | optional, off by default |

**Five embedders, one LLM, one optional decision server.** The embedders are not
interchangeable in name only: each has its own dimension, its own self-test
outcome, and its own cost. The measured consequence is in this repo's history —
a cold `pack` over the whole tree ran **10+ minutes at ~680% CPU and did not
finish**, and the retrieval layer's own self-recovery is 0.585 at best. That is
what an encoder that is too large for CPU costs.

## 2. Where an encoder replaces something

### E1. Embeddings: `bge-base` → a distilled encoder (highest immediate value)

`all-MiniLM-L6-v2` (22 M, 384-d) or `bge-small-en-v1.5` (33 M, 384-d) are the
candidates. The trade is explicit: 3–5× fewer parameters, roughly 5–10× faster
on CPU, at some cost in retrieval quality.

**Measured 2026-09-20 (first run).** `bge-small-en-v1.5` was fetched and wired
as a selectable embedder (`PHYSIS_EMBEDDER=bge-small`, self-test gated like every
other candidate). Same corpus, same bounded pack (60 files, 661/685 chunks), on
pansa, CPU-only:

| encoder | dim | cold chunks | wall-clock | **s per cold chunk** |
|---|---|---|---|---|
| bge-small-en-v1.5 (33 M) | 384 | 661 | 124.8 s | **0.189** |
| bge-base-en-v1.5 (110 M) | 768 | 364 (warm cache) | 114.9 s | **0.316** |

**1.67× faster per cold chunk**, on the axis that matters when the cache is
cold. Two honest caveats: the bge-base run had 321 cache hits so its wall-clock
is not comparable (hence the per-chunk figure), and *both* remain minutes for a
60-file pack because the ONNX runtime is CPU-bound at ~520% CPU. **The encoder
swap helps; it does not solve the cold-pack problem** — the bound (`--max-files`)
and the saved pack remain the real fix, and this measurement does not change
that.

Still missing for E1's full verdict: `hit@5` on the same corpus with both
encoders (`benchmarks/retrieval/`). A faster encoder that retrieves worse is not
an improvement, and the number is not in yet.

**How to judge it, not argue it:** the pack has a timing harness and the
retrieval layer has `hit@5` (currently 0.57 semantic vs 0.14 lexical,
`benchmarks/retrieval/`). Run both encoders over the same corpus: pack wall-clock
and CPU seconds, then `hit@5` and the grid's self-recovery. If `hit@5` holds
within noise and the pack drops from minutes to seconds, the small encoder wins
on the only axis that matters — the one the user feels.

**Note the dimension change is cheap here**: `EmbedCache` is keyed by embedder
kind and dimension, so a new encoder simply starts a new cache. Nothing else
needs to know.

### E2. Grid classification: cosine-to-anchors → a classifier head

Today an entry's cell is `argmax` cosine against anchor phrases. That is an
encoder used as a nearest-neighbour lookup. A **frozen encoder plus a logistic
head** over the 731 entries — or a lightly fine-tuned DeBERTa-v3-small — is a
direct classifier, and it produces calibrated probabilities per cell instead of
a cosine.

This is the natural implementation of plan item **C1** (measure the reworked
grid): the permutation-null harness already exists
(`physis-core/src/grid_fitness.rs`, `research/E66_AXIS_FITNESS.md`), so a
classifier can be scored on exactly the same 731 entries and the same null as
the current method. Whichever wins, the grid finally gets a number that belongs
to the grid that exists.

**And the same head answers routing.** "Which agent for this request" is a
classification over a fixed option set — the same shape as "which cell", and the
same shape as the `choice` decision the decision endpoint already exposes.

### E3. Verification: `noul` via an LLM → an NLI model

"Does the evidence support this claim?" is textual entailment. A DeBERTa-v3 NLI
model (e.g. a `DeBERTa-v3-base-mnli` derivative, ~180 M) answers it in tens of
milliseconds on CPU with a probability, no generation and no parsing.

This is the S4 acceptance gate's missing verifier. Today the gate demands a
verifier record with `ok=true` and nothing in the system can produce one without
an LLM. An NLI leg gives a deterministic, cheap, auditable verifier — and
`similar.rs` already has the vocabulary to say *which* leg decided, so an NLI
verdict would be labelled, not asserted.

### E4. Ranking: hand-weighted attention → a cross-encoder reranker

The seven attention terms are hand-weighted (`semantic 0.60, lexical 0.30,
pin 0.25, context 0.15, recency 0.08`). A cross-encoder reranker
(DeBERTa-v3-small, ~44 M) scores a (query, passage) pair directly and reliably
beats bi-encoder cosine on reranking.

The right move is **not** to delete the terms but to add the reranker as an
eighth term with its own weight, so the readout stays complete and the
comparison stays auditable — the attention module's whole design is that a term
shows its raw value, its weight and its contribution.

### E5. The judge loop: LLM prose → a scored encoder

`jev_review` scores metrics; the agent judges. A small encoder cannot replace the
judgement, but it *can* replace the parts that are classification: "does this
diff add an abstraction with one implementation", "does this test assert
behaviour or implementation". Those are classifier heads, and they would make
the review loop cheap enough to run on every commit rather than on request.

## 3. Where an encoder does NOT replace the LLM

- **Open-ended synthesis.** The agent's answer to "what does this module do" is
  generation. No encoder does it. The measured cost (~43 s floor, 3 m 10 s for a
  repo question) is the price of that job, and the honest move is to keep it off
  the interactive path — which the worker thread already does.
- **The decision contract at scale.** `noul`/`choice`/`score` with probabilities
  and confidence is a served contract; a local NLI model can *back* one endpoint,
  but the contract itself is the interface, not a model choice.
- **Crystallization.** Turning repeated agent output into deterministic
  operators is a judgement about sameness, not a classification — and the plan
  already gates it on a person.

## 4. The consolidation argument

The inventory has five embedders because each arrived for its own reason:
multilingual need, offline need, a quant that failed, a fallback. The small-
encoder candidates collapse most of that:

| today | after | note |
|---|---|---|
| bge-base + bge-m3 + nomic + jina + random projection | one distilled encoder + random projection | one dimension, one cache, one self-test |
| cosine-to-anchors classification | the same encoder with a head | measured on the same null |
| LLM verifier (absent) | an NLI encoder | the S4 gate becomes producible |
| hand-weighted semantic leg | the same encoder as a cross-encoder term | auditable, not replaced |
| LLM for "which agent/cell" | the classifier head | classification, not generation |

That is **one encoder family + one LLM** instead of five embedders and an LLM
doing classification it is bad at. The LLM keeps the two jobs only it can do:
synthesis and open judgement.

## 5. What to do first

1. **E1, because it is measurable today.** Same corpus, two encoders, pack
   wall-clock and `hit@5`. No new concept, no new contract.
2. **E2 together with C1.** The grid needs measuring; a classifier head is both
   the measurement and the replacement. One harness, two answers.
3. **E3 before S9/S12**, because the acceptance gate currently cannot be
   satisfied without an LLM and a cheap NLI leg unblocks it.
4. **E4 as an added term, never as a deletion.** The readout is the reason the
   ranking is trustworthy.

Every step above is a measurement with a named harness, not a rewrite. Where a
measurement does not exist yet (E1's timing, E2's null over the new axis), build
the measurement first — that is the same rule the grid rework followed, and the
reason it could be defended.
