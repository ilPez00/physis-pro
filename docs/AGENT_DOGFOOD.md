# Physis develops physis

The research case: physis is a context compiler, and the hardest available test
of a context compiler is an agent doing real engineering work on a real
repository. This repository is the corpus, `packets/` is the task suite, and
each packet's `gate:` line is an objective pass/fail that no participant can
argue with.

## Why this repository is the right corpus

A retrieval benchmark is only worth running if it can fail. `physis-core`'s
`benchmarks/ground-truth` cannot: run it under a random-projection lexical hash
and under a real ONNX model and every quality metric is identical — repeat
100%, anomaly 100%, contradiction 50%, compression ~24% either way. Its repeat
families are lexically near-identical, so string overlap recovers them
perfectly.

Source code with questions asked in a developer's words is the opposite. The
question says "how does the engine refuse a model that loads but produces
nonsense"; the answer says `semantic_self_test`, `is_available`, `MARGIN`.
Almost no tokens are shared. That gap is what a lexical hash cannot cross.

Measured, same query, same 657 candidate chunks under `physis-core/src`:

| | top result | correct? |
|---|---|---|
| random-projection | `delta_engine.rs:401-440` (score 0.923) | no |
| ONNX MiniLM | `embed.rs:321-360` (score 0.764) | **yes** |

Note the scores. The wrong answer was returned with *higher* confidence. That is
the failure shape this repository already named once (PLAN.md H2, the semantic
self-test packet PH-004): more confident, no more correct.

## Setup

```bash
# 1. Build the binary once. The MCP entry used to be `cargo run`, which
#    recompiled on every agent start.
cargo build --release --features mcp,cli --bin physis-pro

# 2. Mint a dev licence for THIS build. The gate verifies signatures even in a
#    dev build, so a licence signed by a different key is rejected.
mkdir -p .physis-bench
(cd physis-dev-license && cargo run -q) > .physis-bench/license.key

# 3. Point an agent at .mcp.json. Claude Code and opencode both read it.
```

`.mcp.json` sets `PHYSIS_ALLOW_DEV_LICENCE=1`. That is correct for local
development and wrong for anything shipped: a dev build's signing key is public,
so its licences can be forged by anyone. Ship with the vendor key stamped in
(`scripts/package.sh`).

### Embedder

By default the server resolves the ONNX cascade and the first startup pays for
it (measured: 4.5 s to select the embedder, 929 ontology seeds served from
`EmbedCache`). Set `PHYSIS_EMBEDDER=random-projection` for a fast, offline,
**non-semantic** session — useful for exercising plumbing, useless for anything
that depends on retrieval being correct.

## The 30 tools

`physis_pack` is the one that matters for this experiment: it packs the
highest-value context for a query from a directory into a hard token ceiling and
reports the path and line range of every chunk it chose. The rest cover
classification (`physis_classify`, `physis_route`), memory (`physis_remember`,
`physis_recall`, `physis_themes`), causal provenance (`physis_why`,
`physis_causes`, `physis_serve`), goals and PDCA, discovery, and health.

`physis-pro pack <dir> <query>` is the same retrieval from the shell, which is
what the benchmark below drives.

## The measurement

```bash
python3 benchmarks/retrieval/run.py --arms rp,semantic --at 5
```

Ten questions about this repository, each with the answering file established by
reading the code. `hit@k` asks only whether an expected file appears in the
first k retrieved paths — deliberately generous, because if even that cannot
separate a semantic model from a string hash then nothing will. The runner
prints `DISCRIMINATION: semantic - rp` and says so explicitly when the
difference is zero or negative.

Cases whose answer does not live under `--root` are skipped and named, not
scored as misses: blaming the retriever for a corpus that never contained the
answer is how a benchmark starts lying.

Measured 2026-09-12, seven cases in scope under `physis-core/src`, hit@5:

| arm | hit@5 | wall clock |
|---|---|---|
| random-projection | 0.14 (1/7) | 0.5 s |
| ONNX MiniLM | **0.57** (4/7) | 657 s cold, 9.7–15.5 s per query warm |

`DISCRIMINATION: semantic − rp = +0.43`. The instrument works. 0.57 is also not
good — three of seven questions are missed by a real model — and that is the
point of having a number that can move.

`pack` keeps a persistent content-addressed `EmbedCache` in `PHYSIS_DATA_DIR`,
so the first pack over a tree pays for the whole corpus (583 s for 657 chunks)
and later queries pay ~12 s. Warm it once per corpus before running any arm.

## The arms worth running, and what each one would settle

All four now have a harness — `benchmarks/retrieval/answer.py` — and none has a
reported result yet. Built is not measured, and the harness says so itself: any
arm without an endpoint prints `not_configured` and is not scored, any arm with
a failed case prints `INCOMPLETE` and reports no rate at all.

| arm | context | model | what it establishes |
|---|---|---|---|
| A | physis-packed | big (agent) | the current path — does compression lose the answer? |
| B | naive (whole files) | big (agent) | the ceiling. **Without this, arm A has no denominator.** |
| C | physis-packed | small (local 4B) | the actual thesis |
| D | naive | small | the control that says whether physis helped or the task was easy |

The thesis — "a small model plus a compiled context imitates a big model" — is
the claim C ≈ B. It is **not supported by anything in this repository today**:
the harness exists, the arms have not been reported. Treat it as the hypothesis
under test, not as a result.

## Running the arms

```bash
# small tier (arms C, D) — defaults to a local ollama
python3 benchmarks/retrieval/answer.py --arms C,D

# large tier (arms A, B) — any OpenAI-compatible endpoint, same env names oracle.rs reads
PHYSIS_ORACLE_URL=http://<host>:11434/v1 \
PHYSIS_ORACLE_MODEL=qwen3.8-27b-q4:latest \
PHYSIS_ORACLE_KEY=local \
  python3 benchmarks/retrieval/answer.py --arms A,B
```

**The gate.** Each case in `cases.json` carries `must_mention`: the decisive
identifiers a correct answer has to name, established by reading the code and
checked literally, case-insensitively. Same rule as `oracle::cites_evidence`.
No grading model, no partial credit. Seven of the ten cases are gated — the
three whose answers live outside `physis-core/src` are skipped, not scored.

**Equal-size comparison.** Both context arms are built to the same character
budget per case, so what is compared is context *quality* at equal size. The
naive arm is round-robin file heads across the whole tree — the shallow dump a
developer actually pastes — not an alphabetical prefix, which would be a
strawman that loses by construction.

**On calling a local model the large tier.** A 27B beside a 4B is a real tier
gap and a legitimate large-vs-small contrast. It is *not* a frontier oracle, and
results from it must be labelled as a local peer tier. Pointing
`PHYSIS_ORACLE_*` at a 4B and calling the output arm B is the exact dishonesty
the gate exists to prevent.

**Cost, measured.** qwen3:4b through ollama's OpenAI endpoint: 93 s on a
one-word prompt, and its thinking cannot be turned off there — neither `think`
nor `chat_template_kwargs` has any effect (verified). Harmless for the gate,
since ollama returns reasoning in a separate field and only the answer is
scored; not harmless for the clock, hence the 1800 s per-call timeout.

## Running the agent arms

The task suite is `packets/`. Take one packet, run its `gate:` line, and record
tokens spent and whether the gate passed. `packets/PROTOCOL.md` governs the rest
(claim, gate, report, commit; one packet per agent; the gate is the spec).

The standing rule of this repository's research track, earned seven times over
in `research/RESEARCH_STATUS.md`: **treat any positive as an artifact until a
construction-matched control says otherwise.**
