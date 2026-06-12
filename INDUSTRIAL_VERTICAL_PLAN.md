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

- [ ] `report.rs`: compose `OeeReport` + `MaintenanceMonitor` anomalies +
      `dream` output into one Markdown document (Italian template first —
      the buyer reads Italian).
- [ ] CLI: `physis-pro report --shift <spec>`; HTTP: `GET /api/v1/report/shift`.
- [ ] Render path: Markdown → PDF (typst or headless pandoc) or clean HTML
      email. Samples to mirror: `docs/business/sample-outputs.md`.
- [ ] Wire one demo dataset end-to-end so the proposal shows a real report,
      not a mock.

## Step 3 — Proposal out

- [ ] Refresh `proposta-generalmecannica.md` with the real shift report as an
      attachment and the multilingual classify demo.
- [ ] Send; meeting decides the adapter roadmap (which machines, which
      protocol — MQTT, Modbus, OPC-UA, serial).

## Explicitly deferred (until customer pull)

- MQTT/Modbus subscribe loops (PLAN 1C) — after the meeting names protocols.
- Edge agent binary — after the meeting names the network topology.
- Web dashboard (PLAN Phase 2) — the report is the MVP of "dashboard".
- Ontology discovery mode — feeds ontology packs later.
