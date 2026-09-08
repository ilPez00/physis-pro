# Qwen3-8B n-gram representation dictionary — definitional audit

**Status: NOT IMPLEMENTED. No experiment has been run on the intended
artifact, and none of the existing "Qwen" results describe it.**

This document does the work the brief requires *before* implementation: say
exactly what the component is, classify every existing artifact that might be
mistaken for it, and state what would have to be built.

## 1. What the component is, and is not

**Is:** a representation dictionary distilled from Qwen3-8B's n-gram structure —
a stable intermediate structural vocabulary between raw text and Physis'
epistemic layer.

```
text → tokenizer / subword structure → Qwen n-gram dictionary
     → structural features → Physis (ontology, hypotheses, coherence,
       provenance, prediction, revision)
```

**Is not:** `text → Qwen inference → generated text`. The component is not a
generative model, and must not be described as one anywhere in the code,
experiments, or thesis.

The hypothesis under test is **not** "Qwen understands language and therefore
gives Physis semantics." It is: *Qwen contains exploitable n-gram structure that
distills into a dictionary, and that dictionary may be a useful intermediate
structural vocabulary.* Whether it is remains open (H5).

## 2. Existing artifacts, classified — none of them is this

The brief lists seven categories that must not be merged. Every Qwen-adjacent
artifact in this repository, audited:

| # | category | present? | what it actually is |
|---|---|---|---|
| 1 | intended Qwen3-8B n-gram dictionary | **NO** | does not exist in any form |
| 2 | generic n-grams | no | — |
| 3 | hashed n-grams | **YES** | `physis-core/src/embed.rs:34` `hash_ngrams` |
| 4 | tokenizer statistics | no | — |
| 5 | full Qwen model | **YES** | `scripts/gen_cell_vocab.py`, `qwen3:4b` via ollama |
| 6 | precomputed Qwen-derived vocabulary | **YES** | `cell_vocab_AB.json` `"generated"` |
| 7 | obsolete cell/schema definitions | **YES** | that same file, authored pre-0.1.21 |

### Category 3 — `hash_ngrams`, and why it is not a bridge

`physis-core/src/embed.rs:34`. It takes **character** unigrams and bigrams of
the lowercased string (`hash_ngrams(&lower, 1)` and `(&lower, 2)` over
`chars()`, space-padded), SHA-256 hashes each gram, and sums a seeded random
unit vector per hash — bigrams at weight 0.5, XOR-masked into a separate basis.

It is the *fallback* embedder for builds without ONNX weights. Its dictionary
carries **no frequency, no embedding, no transition probability, no contextual
statistic, and no learned score** — a hash is an address, and the vector behind
it is deterministic noise. Retrofitting these hashes into Q_n would be relabeling
a random projection as a structural bridge. Explicitly ruled out.

### Categories 5, 6, 7 — the `cell_vocab_AB.json` "generated" arm

This is the artifact scoring **AUC 0.575** that the thesis draft cites. Three
independent reasons it is not evidence for the Qwen component:

1. **Wrong mechanism.** `scripts/gen_cell_vocab.py:150` calls `qwen3:4b` through
   ollama at temperature 0 and asks it, in natural language, for additional
   vocabulary terms in the cell's register. That is generative inference
   producing word lists. Category 5, not category 1.
2. **Wrong model.** `qwen3:4b` (2.5 GB, the only Qwen asset on this machine) is
   not Qwen3-8B.
3. **Wrong schema.** The vocabulary was authored against pre-0.1.21 cell
   composition. Stage 12 records that 77.6% of entries have since moved, so the
   vocabularies no longer describe the cells they are scored on. The drop from
   0.701 to 0.577 is staleness, not a finding — in either direction.

The script's design is nonetheless the right shape and should be preserved: it
pairs the LLM arm (B) against a no-LLM IDF-frequency arm (A) on identical
inputs, with a deterministic train/test split, so the model is falsifiable
rather than merely present. Stage 6e used that control to measure the LLM's
contribution at **zero**.

## 3. What must be defined before anything is built

The brief's ten questions, with what is currently decidable and what is not:

| # | question | state |
|---|---|---|
| 1 | what constitutes a Qwen n-gram | **undecided** — see §4 |
| 2 | extraction level | **undecided** — bytes / BPE subwords / token sequences all plausible |
| 3 | how n-grams map to representations | undefined |
| 4 | what the dictionary carries | undefined; must be *stated*, not inherited |
| 5 | what an entry is | undefined |
| 6 | how input text maps into the dictionary | undefined |
| 7 | what is emitted downstream | undefined |
| 8 | deterministic? | must be — non-determinism has bitten this track three times (Iteration 26) |
| 9 | corpus-dependent? | depends on §4; frequencies are, token IDs are not |
| 10 | behaviour when the corpus changes | must be specified before use — §2 category 7 is exactly this failure |

**A list of n-grams is not a representation dictionary.** What information the
dictionary carries has to be established, not assumed from provenance.

## 4. The ambiguity that has to be resolved first

"Qwen3.8" has two readings and they lead to different experiments:

- **Qwen3-8B's tokenizer / BPE merge structure.** Extractable from an ~11 MB
  `tokenizer.json` — no weights, no inference, cheap, deterministic, and
  corpus-independent. This is the reading consistent with "a representation
  dictionary, not the full model."
- **N-gram statistics distilled from Qwen3-8B's behaviour** (embedding matrix
  rows, or n-gram transition statistics read off the model). Requires the
  weights, is far more expensive, and is closer to distillation than to a
  dictionary.

The first reading is the one the brief's framing implies and the only one
runnable at present. **Neither is available locally**: `ollama list` shows only
`qwen3:4b`, and no Qwen tokenizer or vocabulary file exists in `models/` or the
HF cache. The first concrete step, if this track is authorized, is pulling
`Qwen/Qwen3-8B`'s tokenizer alone.

## 5. Required ablation, when it runs

The dictionary earns its place only if

```
Qwen3-8B n-gram dictionary  >  ordinary n-gram representation
                            >  tokenizer-only representation
```

on the specific downstream task, against the ladder A–F in the brief. Two
baselines from this track must be in that ladder, because both are stronger than
they look:

- **F, existing Physis mechanisms**: plain cosine to the cell centroid,
  **AUC 0.866** with BGE-base (E1) — not the 0.733 the older write-ups compare
  against. Every structural mechanism proposed in this track has lost to it.
- **B, ordinary n-gram statistics**: the IDF-frequency arm of
  `gen_cell_vocab.py`, which already beat the qwen3:4b generative arm.

If the dictionary does not clear these, the negative result gets recorded. It is
not valuable merely because it originates from a powerful model.

## 6. Sequencing

E1 returned **DIVERGENCE HYPOTHESIS NOT SUPPORTED**
(`E1_CROSS_MODEL_DIVERGENCE.md`), which is the brief's FAIL branch: stop the
JEPA/Qwen expansion and reassess around Stage 13. So the Qwen track is **not
started**, and nothing here has been combined with the divergence signal —
correctly, since the brief requires the dictionary to be validated independently
first, and since there is now no divergence signal to combine it with.

What survives from E1 for this track is a warning rather than an opening. Stage
13 found a single embedder reading two authors' hint-formatting conventions as a
semantic stratum; E1 found a second embedder's disagreement does not correct
that. A **format-blind** instrument is still the right idea — and an n-gram
dictionary is one — but the same evidence says the bar is now 0.866, and the one
format-blind instrument already tested (WordNet ancestry) reaches 0.553 within
cosine strata. That is the prior this track starts from.

## 7. Level discipline (to be preserved in code and thesis)

| level | contents |
|---|---|
| **FORM** | tokenizer, subwords, n-grams, the Qwen structural dictionary |
| **REPRESENTATION** | vectors/features produced from the dictionary |
| **INTERPRETATION** | Physis hypotheses, ontology references, competing readings, coherence |
| **GROUNDING** | empirical observation, `src/embed/clip.rs`, `src/embed/jina.rs`, potentially JEPA |

The dictionary is none of: an ontology, a hypothesis engine, a truth
representation, a contradiction detector, a semantic ground truth, a generative
model. Any implementation that silently substitutes full-model inference,
generic embeddings, arbitrary hashes, or an unrelated n-gram representation is a
**different experiment** and must be labeled as one.

## 8. Two corrections to the thesis draft, verified in source

**Multimodal grounding already exists.** `src/embed/clip.rs` (CLIP, 512-d,
separate text and vision ONNX sessions in a shared space) and
`src/embed/jina.rs` (Jina CLIP v2, 1024-d, single model for text + image,
L2-normalized shared space) are both real, and `models/jina-clip-v2` (627 MB) is
present locally. So "JEPA vs nothing" is not the available comparison; the
correct one is CLIP/Jina vs JEPA vs CLIP/Jina + JEPA, and JEPA must show
incremental value over an already-aligned representation.

One caveat that matters for sequencing: **both live in `physis-pro/src/embed/`,
not in `physis-core`**, while the Stage 12 harness is a `physis-core` example.
The multimodal path is not reachable from the certification harness today
without moving code. That is a real cost on any E3/E4, and it should be counted
before either is scheduled.

**The implemented coherence dimensions are six, and "Ontological" is not among
them.** `physis-core/src/coherence_dimensions.rs`: `semantic`, `logical`,
`temporal`, `causal`, `procedural`, `empirical`. Any mapping of new signals onto
Physis primitives uses these names and no others.
