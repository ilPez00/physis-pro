# llm-coherence-scan

A standalone CLI in the `physis-pro` workspace that scores an LLM run
(prompt + RAG files → assistant text + edited files) and runs a
refinement loop over the ontology.

## What it produces

- **Coherence Risk** — 0–1 scalar + 3-dim breakdown (semantic, structural, topical).
- **Model Drift** — 0–1 scalar + vector over `[local, reference, cross-model]`.
- **Workflow map** — Mermaid diagram of the run.
- **Refinement loop** — every run updates the ontology: salience from
  file edits/deletions (lazy half-life decay), `dream` pass replays
  asserted failures, `discover` proposes new cells for outlier files,
  context overlays catch "right cell, wrong frame".

## CLI

```text
physis-scan run --in <run.json> [--out ./out] [--ref <corpus.json>]
physis-scan baseline add <name> <run.json>... --to <corpus.json>
physis-scan baseline list --from <corpus.json>
physis-scan refine                              # list pending proposals
physis-scan refine --accept <id> | --reject <id>
```

## Build + test

```text
cargo test  -p llm-coherence-scan
cargo build -p llm-coherence-scan
```

## v1 notes

- Embedder is deterministic-hash so the build is portable. The
  physis `VectorEmbed` seam is `engine::score`; swapping in a real
  embedder is a one-file change.
- `physis_pro` is the future target for `classify` / `structsim` /
  `salience`; the refinement module's surface (`SalienceStore`,
  `DiscoveryProposal`, etc.) matches the physis API, so the swap is
  mechanical.
- Salience bias is bounded by `physis_pro::salience::BIAS_STRENGTH`
  when wired to physis; v1's salience is a free scalar.
