# E20 floors — measured before the table exists

**Status: the 51GB Qwen3.8-Flash-Next n-gram table is NOT on the fleet.**
Checked 2026-09-09 on omo and pansa: no file over 5GB matches it, and the
largest Qwen3.8 artifact is `Qwen3.8-27B-Q4_K_M.gguf` (17GB, the dense 27B, a
different object). Disk was the blocker and no longer is — omo has 157G free,
pansa 114G.

None of E20's declared controls needs the table, so they were measured first.
Run: `e20venv/bin/python e20_floors.py` (needs `wordfreq`; no inference except
one embedding call per probe).

---

## The result: E20's pre-registered positive is already the floor

| scorer | forced choice | AUC |
|---|---:|---:|
| LEN-WORDS | 0.250 | 0.378 |
| LEN-CHARS | 0.417 | 0.420 |
| FREQ-MEAN | 0.583 | 0.535 |
| FREQ-MIN | 0.333 | 0.521 |
| FREQ-RARE-N | 0.250 | 0.427 |
| **EMB-LOPO** (`nomic-embed-text`) | **0.917** | **0.757** |

Per-clause forced choice — which decoy type each control already splits:

| scorer | C1 (n=3) | C2 (n=1) | C3 (n=4) | C4 (n=3) | C5 (n=1) |
|---|---|---|---|---|---|
| LEN-WORDS | 0.33 | 0.00 | 0.50 | 0.00 | 0.00 |
| LEN-CHARS | 1.00 | 0.00 | 0.25 | 0.00 | 1.00 |
| FREQ-MEAN | 0.67 | 1.00 | 1.00 | 0.00 | 0.00 |
| FREQ-MIN | 0.00 | 1.00 | 0.50 | 0.33 | 0.00 |
| FREQ-RARE-N | 0.67 | 0.00 | 0.25 | 0.00 | 0.00 |
| **EMB-LOPO** | **1.00** | 1.00 | **1.00** | 0.67 | 1.00 |

**Length and frequency are at or below chance**, which is the good news: the
confound that killed ANCHORVOC in E17/E18 is not present in this probe set, and
"attestedness is frequency wearing a hat" would not by itself explain a sieve
result here.

**The embedding is the problem.** E20's pre-registered prediction was:

> the sieve will **not** separate the C1 (common-cause) pairs, because that
> distinction is causal rather than lexical … Separating only the C3
> (identity-destruction) pairs is the expected honest positive, and would still
> be useful.

An off-the-shelf sentence embedding separates **C3 4/4 and C1 3/3.**

- The "expected honest positive" is **already achieved by control (a)**. If the
  sieve delivers it, that is not evidence for the sieve.
- The pre-registered *negative* is **false**: C1 is separable here, so
  "causal rather than lexical" is not the operative distinction. The rationale
  for the prediction does not survive.

**The only headroom left is C4 at 0.67** — differentiation, which is the same
clause E19 found to be the worst column for every arm including a hand-authored
ORACLE. Both experiments converge on it independently.

## What this changes about running E20 at all

E20 is worth running only against a target that beats **0.917 forced choice**,
and specifically beats **1.00 on C1 and C3**, which is not possible — so the
informative question is narrowed to **C4 alone, where the floor is 0.67 on
n = 3 pairs.** Three pairs cannot settle anything. **Before the table is worth
51GB and a week, the probe set needs more C4 and C5 pairs** (present counts:
C3 4, C1 3, C4 3, C2 1, C5 1 — two clauses rest on a single probe each).

That is a cheap, purely authorial task with a pre-flight to honour, and it
should precede the download rather than follow it.

## Caveats, stated

1. **n = 12 pairs.** EMB-LOPO's 0.917 is 11 of 12; exact 95% CI **[0.615,
   0.998]**. Above chance, but the per-clause cells rest on 1–4 pairs each and
   are indicative only. This is the same small-n objection the floor is being
   used to raise against E20, and it applies to the floor.
2. **This is a supervised baseline; E19's arms are not.** EMB-LOPO fits
   centroids on the other 11 labelled pairs. The E19 arms never see a label —
   they get a stimulus and write a schema. So this does **not** show "an
   embedding beats the LLM at E19's task". It is the right comparison for E20,
   which is itself a supervised representation-quality test, and the wrong one
   for E19.
3. **Substitution.** The design says BGE; `nomic-embed-text` is what the fleet
   serves. It is a strong general embedder, so it raises the floor and makes the
   sieve's job harder — conservative in the direction that matters.
4. **A probe-set concern for E19, not only E20.** The declared E19 floor was
   TF-IDF leave-one-pair-out forced choice **0.750**. A dense embedding reaches
   **0.917** on the same holdout. The probe set is more separable by
   surface-semantic means than the pre-flight's TF-IDF check reported, and the
   pre-flight should gain an embedding row so this is caught at authoring time
   rather than discovered later.
