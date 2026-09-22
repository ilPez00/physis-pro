# PH-1xx proposals — second discovery round (PH-100, 2026-09-22, opencode, pansa)

Sources: `packets/reports/*` unknowns sections (PH-015 §1, PH-020 §1, PH-022
§1–2), `mesh mem traps` (no new physis traps since 2026-09-19 — nothing to
promote), PLAN.md hardening backlog H5 (H6 noted as a future source; H10
residue is PH-006's blocked human input), last 20 git subjects (PH-101 cortex
landed, MS-003 corrections applied). H11 stands: no new surface below —
hardening, measurement, removal, enablement only. Numbering starts at PH-102:
PH-101 is taken (`PH-101-cortex-hardware-semantic.md`), and the PH-030…PH-036
series was delivered on branches `agent/PH-100` + `agent/PH-030-pre`
(`759817f`), not on this tree — a merger renumbers or keeps both, never
silently overwrites.

## PH-102-generic-extract-guard

---
id: PH-102
track: physis
phase: 1
status: todo
rung: 2
depends: [PH-000, PH-015]
gate: "cargo test --lib ingest:: && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/PH-102; guard + tests commit"
---

# Generic misclassification guard for `extract_doc`

## Why

PH-015 fixed one arm: a text file classified `VoiceTranscript` without the
`audio` feature now falls back to its bytes (`src/ingest.rs:262-277`, test
`misclassified_text_file_keeps_its_body_ph015`). The report's §1 names the
rest of the class, still open: `Invoice`/`Email`/`OcrDocument`/`Spreadsheet`
with their features off (`src/ingest.rs:241,248,255,284`), `Image` without
`ocr` (`:295`), and `Video` (`:302`) all return empty unconditionally — while
the header comment (`:225-229`) claims a real UTF-8 text file is never an
empty body "whatever the kind claims". The comment over-claims; the guard is
partial.

## Steps

1. Write one failing test per arm: a UTF-8 text file forced into each kind
   above must yield a non-empty body. Run to confirm all fail.
2. Extend the PH-015 fallback (extension check + `read_to_string`) to every
   arm that can return empty for text, keeping genuine binaries empty.
3. Update the header comment to describe the NEW coverage — never delete the
   PH-015 history.
4. Full gate green.

## Tests you must add

- `misclassified_text_body_survives_{invoice,email,ocr,spreadsheet,image,video}_ph102`
  (each fails before, passes after).

## Gate

```bash
cargo test --lib ingest::
cargo clippy --lib --tests -- -D warnings
```

## Never

- Change `classify_file` itself — extraction fallback only.

## PH-103-env-off-hot-paths

---
id: PH-103
track: physis
phase: 1
status: todo
rung: 2
depends: [PH-000, PH-020]
gate: "cargo test --lib && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-103; snapshot + test commit"
---

# Environment reads off per-call paths

## Why

PH-020's §1: a direction-affecting `env::var` was found on a hot path by
accident, and `grep -rn 'std::env::var' src/` answers whether siblings exist.
They do: `src/ai/provider.rs` (`:482,496,504,559,561,887,892,1295-1298`),
`src/mvp/repl.rs`, `src/mvp/deps.rs`, `src/settings.rs`. Any read inside a
per-call function is the same defect class — behaviour that changes with
process environment mid-run, untestable by construction.

## Steps

1. Audit every hit: startup/config-load (keep, justify in report) vs
   per-call (move).
2. Snapshot per-call reads into config at startup; call sites read the
   snapshot. No default values change.
3. Add a test with the deterministic embedder: mutate the env var mid-process,
   assert the handler output is byte-identical.
4. Full gate green.

## Tests you must add

- `env_mutation_does_not_move_handler_output_ph103` (fails before, passes
  after).

## Gate

```bash
cargo test --lib
cargo clippy --all-targets -- -D warnings
```

Report must list every residual `env::var` with its keep justification.

## Never

- Change any default value — snapshot the read, not a new policy.

## PH-104-secom-wire-or-decide

---
id: PH-104
track: physis
phase: 1
status: todo
rung: 2
depends: [PH-000]
gate: "cargo test --features industrial_benchmark 2>&1 | tail -1"
commit: "branch agent/PH-104; numbers-or-decites commit"
---

# H5: wire the SECOM benchmark to full data or stop citing it

## Why

PLAN.md H5, uncovered by any packet: the embedded sample is 21 observations /
10 features (real UCI SECOM is 1567 × 590); the default run splits to a
10-row test set, scoring 60% (random embedder) / 70% (ONNX) against a 61.9%
majority baseline — a number that carries no information, while the module is
still cited (`docs/ARCHITECTURAL_REPORT.md:156`, the industrial module list).
`src/industrial_benchmark/secom.rs` already has the seams: "embedded sample +
optional download" (`:4`), `parse_secom_csv` with `max_rows` (`:311`), a
`max_observations` config (`:455`).

## Steps

1. Attempt the full-data run first (download or local UCI CSV through the
   existing seams). Record accuracy, precision/recall on the rare FAIL class,
   and the majority-class baseline.
2. If full data runs: commit the numbers with the command that reproduces
   them, and update every citation to quote them.
3. If full data cannot run: remove SECOM from customer-facing claims, mark
   the module research-only in place, and say so in the report.
4. Gate green either way.

## Tests you must add

- A test pinning whichever outcome landed: full-data floor (numbers from step
  1, no tighter than measured) or the absence of customer-facing citations
  (`! grep -rin secom` over the customer pages named in the report).

## Gate

```bash
cargo test --features industrial_benchmark
```

Plus the committed numbers or the de-citation grep above.

## Never

- Invent data or quote a number not measured on the full set.

## PH-105-human-rerank-run

---
id: PH-105
track: physis
phase: 2
status: todo
rung: 3
depends: [PH-000]
gate: "test -f research/mapper/rerank_scores.json && python3 scripts/score_rerank.py --help"
commit: "branch agent/PH-105; scores + labels commit (docs/data only)"
---

# Run the built human-reranker worklist

## Why

Research-track open item 5 (`research/RESEARCH_STATUS.md`): the machine ranks
a worklist but cannot certify it (0.410 AUC on real misfilings, Stage 9), so
the last filter is a person.
`physis-core/examples/human_rerank_export.rs` emits a 75-item worklist
(machine head-50 + 25-item uniform tail, injected misfilings blind as gold,
machine scores hidden, labels held separately) and `scripts/score_rerank.py`
joins them back. Built, never run — the native filings this track has never
had labelled are the deliverable.

## Steps

1. Export the worklist; record the exact command and seed.
2. A human labels all 75 items blind (scores hidden, held-file discipline —
   the machine never sees labels pre-score).
3. Run the join; commit `research/mapper/rerank_scores.json`: exact recall on
   the injected, human-vs-machine AUC on the same items, precision lift from
   human filtering, anchoring check on revealed items, repair check on named
   destinations, and the flagged native filings.
4. Report which natives were flagged; propose their re-filings or a packet
   that does.

## Tests you must add

- None (measurement). The gate is the scores file existing with all six
  measures present (a schema assertion script counts; evaluator eyeballs).

## Gate

```bash
test -f research/mapper/rerank_scores.json
python3 scripts/score_rerank.py --help
```

## Never

- Let the machine see labels before scoring; break the held-file discipline
  and the AUC is void.
- Build new surface — data and scores only.

## What a human must decide (rung 3)

Who labels the 75 (domain competence required), the dispute rule on
disagreements, sign-off on the label schema before step 2, and whether the
flagged natives get re-filed (that is a follow-up packet, not this one).
