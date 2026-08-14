# Industrial Vertical — Execution Plan

> Decision (2026-06-12): per PREMORTEM P2 (one thread of value), the active
> vertical is **industrial backoffice intelligence** — the only one with a named
> prospect (`docs/business/proposta-generalmecannica.md`). LifeOS and
> agent-memory continue as background arcs; they ride the same engine.

Sequence: **1. multilingual embedder → 2. shift report → 3. proposal/meeting.**
Edge adapters (MQTT/Modbus) deliberately wait for a customer to name their
machines — no adapter guessing.

---

## Step 1 — BGE-M3 multilingual embedder — ❌ MEASURED, NOT ADOPTED (2026-06-12)

**Hypothesis:** BGE-base-en-v1.5 is English-only; BGE-M3 (XLM-R, 1024-d,
multilingual) should lift Italian classification.

**Result — M3 lost on our fixture set** (nearest-entry scoring, English
ontology hints):

| | CORE dom/cell | ITALIAN dom/cell | runtime |
|---|---|---|---|
| bge-base + mean (prod) | **1.000 / 0.949** | 0.733 / 0.600 | 583s |
| bge-m3 + mean | 0.923 / 0.846 | 0.667 / 0.600 | 1573s |
| bge-m3 + CLS (official) | 0.923 / 0.769 | 0.733 / 0.667 | 1397s |

Italian domain accuracy only *ties* the English model; CORE regresses hard;
2.5× slower (fp32 XLM-R-large on CPU). M3's misses are domain-level
("manutenzione del macchinario" → HEAL). **bge-base stays production.**

**Kept (useful regardless of model choice):**
- [x] `OnnxEmbedder` feeds `token_type_ids` only when the session's input
      signature declares it (XLM-R exports don't take it; previously errored
      → silent RP fallback).
- [x] `PoolingStrategy` config knob (`mean` | `cls`) wired through embed +
      embed_batch.
- [x] `EmbedderKindConfig::BgeM3` opt-in (registry/cli/web arms); auto-cascade
      ranks it BELOW bge-base per the measurement.
- [x] Italian fixture tier (15 cases) in `tests/classification.rs` — reported
      per-model; the multilingual gap is now a measured number, not a guess.
- [x] Local export pipeline: `~/.venvs/onnx-export` venv,
      `optimum-cli export onnx -m BAAI/bge-m3` → `models/bge-m3/` (2.1GB fp32,
      external-data format, loads under pinned ort rc12).

**Why M3 likely lost & next candidates (deferred, do after Step 2):**
- Centroids come from short English ontology hint phrases — M3 is tuned for
  retrieval over longer passages; bge-base is a strong short-text STS model.
- Candidates: `multilingual-e5-base` (560MB, instruction-prefixed),
  `paraphrase-multilingual-MiniLM-L12-v2` (470MB, STS-tuned — likely best fit
  for short hint matching), or translating the ontology hints to Italian and
  keeping bge-base per-locale.
- Gate any candidate on the same three tiers; adopt only if ITALIAN improves
  materially with CORE ≥ floor.

## Step 2 — Shift report (the sellable artifact)

One readable report per shift, generated from data already in the engine:
OEE (availability × performance × quality), top stops, anomalies with
sigma scores, RUL warnings, and dream-proposed countermeasures.

- [x] `report.rs`: `ShiftReportInput` → Italian Markdown — plant rollup,
      per-machine KPI + six-big-losses Pareto, anomalies (σ), RUL projections,
      dream countermeasures. Pure/stateless, unit-tested.
- [x] CLI: `physis-pro report --shift <json>`; HTTP: `POST /api/v1/report/shift`
      (stateless, full input) + `GET /api/v1/report/shift` (live OEE board +
      graded dream countermeasures).
- [x] Render path: self-contained HTML (inline CSS, print stylesheet) —
      `render_shift_report_html`, `?format=html` on both endpoints, CLI
      `--html`, sample at `docs/business/sample-shift-report.html`. Email-able
      and browser print-to-PDF; a server-side PDF writer (typst/pandoc binary
      in the container) is deferred until a customer needs the file generated.
- [x] Demo dataset end-to-end: `examples/shift_report_demo.rs` →
      `docs/business/demo-shift.json` + `docs/business/sample-shift-report.md`
      (real engine output: OEE 67.8%/90.0%, 4.4σ bearing anomaly, RUL ~2h).

## Step 3 — Proposal out

- [x] Refresh `proposta-generalmecannica.md`: OEE/anomalie/RUL + shift-report
      section with real demo excerpt (attachment:
      `sample-shift-report.html`); corrected the stale accuracy claim (the
      97.4/92.3 figures were the English fixture set — Italian is measured at
      73.3% domain and now stated honestly with the onboarding mitigation).
- [ ] Send; meeting decides the adapter roadmap (which machines, which
      protocol — MQTT, Modbus, OPC-UA, serial). ← human step

## Step 4 — Grid expansion: Italian-language ontology anchors

The measured Italian gap (0.733 domain / 0.600 cell) comes from cross-lingual
embedding noise: Italian queries matched against English-only anchors under an
English model. Fix without changing models: **add Italian-language ontology
entries** — an Italian query against an Italian anchor is *same-language*
matching, where bge-base is strong. Nearest-entry scoring picks the best anchor
per cell, so Italian entries help Italian input without touching English paths.

- [x] `config/italian_industrial_ontology.json` — 26 entries of generic Italian
      vocabulary per cell (industrial + office + daily life), registered in
      `EXTRA_ONTOLOGIES`. Anti-overfit rule held: entries carry vocabulary,
      never fixture sentences.
- [x] Measured (2026-06-12): **ITALIAN 0.733/0.600 → 0.867/0.867** (cell
      accuracy +44% relative), CORE unchanged at 1.000/0.949 — zero English
      regression. Same-language anchors beat the multilingual-model swap that
      step 1 measured and rejected. ⬜ Gate the Italian tier after it survives
      a few runs at this level.
- [x] **Dogfood bonus** — classifying this session's own work log with the CLI
      exposed two real bugs: the CLI embedder cascade had no semantic self-test
      gate (broken Jina fp16 loaded first → every CLI verdict was ~0.06 noise)
      and `run_classify` still used pre-P3 mean-centroid scoring. Both fixed:
      shared `embed::semantic_self_test` gates web+CLI+tests, CLI uses
      nearest-entry `CellClassifier`, and the CLI prints embedder provenance.

## Future angles (validated by one customer before building)

- **Transizione 5.0 evidence reports** — Italian tax credits require
  *documented, measured* efficiency improvements; the OEE+report pipeline plus
  the Power Draw ontology domains can emit the evidence document. Verify the
  current credit rules before pitching. Sharpest wedge: customers are legally
  incentivized to buy monitoring.
- **ISO 9001 audit trail** — classified, timestamped machine/quality logs as
  audit-ready documentation; reuses the report infra, different buyer.
- **Coherence tripwire for AI outputs** — generalize Agora quality scoring:
  score any LLM/agent output against accumulated ±1 coherence truth
  ("does this claim cohere with what is known to have worked?").
- **Process drift detection** — weekly classification of a team's
  communication stream; drift in the cell distribution = early process-decay
  signal. Fits the "facilitatore di processo" pitch directly.

## Explicitly deferred (until customer pull)

- MQTT/Modbus subscribe loops (PLAN 1C) — after the meeting names protocols.
- Edge agent binary — after the meeting names the network topology.
- Web dashboard (PLAN Phase 2) — the report is the MVP of "dashboard".
- Ontology discovery mode — feeds ontology packs later.
