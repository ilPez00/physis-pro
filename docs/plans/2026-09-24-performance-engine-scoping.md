# Performance & capability plan — scoped (2026-09-24)

Source: `docs/ENGINE_PERFORMANCE_PLAN.md`. Two targets:
- **physis-pro** (this repo) — production line, ships behind feature gates.
- **physis-next** (`~/dev/physis-next`) — canonical kernel line; each item
  lands in the layer that owns the concern, behind the quarry boundary.

Scope decision rule (**revised 2026-09-24: physis-next has priority**).
physis-next is the canonical line — contracts and kernels land there first;
physis-pro gets the same capability as a transplant once the contract is
proven. Items that are pure contract (provenance, deltas) and anything the
kernel layer owns go next-first; only pro-specific production plumbing
(ONNX cascade, web tenants) stays pro-only.

---

## 1. Direct SQL ingestion (SQLite first) — lead: **next**

| | physis-pro | physis-next |
|---|---|---|
| Where | new `src/connectors/sqlite.rs` behind `connectors` feature; `MachineAdapter`-style trait `RowSource` | `adapters/` (out of workspace, per `main_builds_without_optional_adapters`) feeding `physis observe` |
| Contract | `RowSource { open(cursor) -> Rows }`, stable PK + source cursor; rows flatten to `IngestedDoc` labeled `sql:<db>:<table>:<pk>` | rows become kernel `Observation`s with provenance `(source, table, row_key, cursor)` |
| Perf target | 10k unchanged rows = DB read only, no vector allocation (item 3 prerequisite) | same, enforced as a test in `tests/invariants/` |
| Accept | round-trip test on a fixture DB; re-scan unchanged = 0 embeds (assert via EmbedCache hit count) | observation idempotency: same row key → same revision, no delta |
| Est | 1–2 d | 1 d after pro contract exists |

## 2. CDC / watermarks — lead: **next** (near-free on event sourcing)

| | physis-pro | physis-next |
|---|---|---|
| Where | `RowSource` extension: `changed_since(watermark)`; watermark persisted per (source, table) in sled | kernel already event-sourced — watermark is a `Revision` cursor, zero new state |
| Scope | v1: `updated_at` column compare + full-scan fallback. WAL/logical replication deferred (ponytail: column watermark first; CDC pipes are a 10x lift for a problem 1% of sources have) | v1 free via replay; deletion/drift detect = periodic reconcile pass over PK set |
| Accept | 1% changed rows → ~1% embedding work measured on fixture | delta emitted only for changed rows, test with mutated fixture |
| Est | 1 d (after 1) | 0.5 d |

## 3. Content-addressed work cache — lead: **next** (stamp contract), pro follows

| | physis-pro | physis-next |
|---|---|---|
| Where | generalize `pack::EmbedCache` into `ContentCache` keyed `(layer, content_hash, model_stamp)`; layers: parse, normalized text, embed, classify, projection. Persisted per-root like nav index (JSON, not sled) | `physis-world` already content-hashes; add per-layer stamp so model/ontology change invalidates only its layer |
| Key invariant | embedder-stamp refusal (PH-028 pattern) reused per layer | same |
| Accept | model swap → only embed layer recomputes; ontology swap → only classify+projection; measured on 1k-file fixture | invalidation matrix test |
| Est | 2–3 d (touches ingest, watch, nav warm paths) | 1 d |

## 4. Batch execution / back-pressure — lead: **deferred both sides** (contract only in next)

| | physis-pro | physis-next |
|---|---|---|
| Where | ingestion queue: bounded channel + adaptive batch sizing for ONNX embed; ONNX session reuse is already the cascade's job — verify, don't rebuild | physis-next has no embedding hot path yet (`quarry` link) — scope only the queue contract in `physis-world` |
| Ponytail note | no full async runtime in pro; `std::sync::mpsc` + bounded channel suffices until measured otherwise | skip until a real embed workload exists — YAGNI |
| Accept | 10k-row import holds < N MB, resumable after kill (checkpoint per batch) | contract only: `IngestQueue::push` never blocks caller |
| Est | 2 d | 0.5 d contract |

## 5. Durable provenance — lead: **next** (kernel struct)

| | physis-pro | physis-next |
|---|---|---|
| Where | ledger `CauseEvent` already carries actor/mechanism — extend with `(source, table, row_key, watermark, model_stamp, ontology_version)` | kernel provenance field is the canonical home; this is **next-first**: define the struct in `physis-kernel`, pro follows |
| Accept | every node answers "which row, which model version"; round-trip serde; ledger replay stable | already replay-stable by design — test the new fields |
| Est | 1 d | 0.5 d (**do first here**) |

## Capability upgrades (deferred, ranked)

Ranked by value/cost; none block the five above.

1. **FK joins as relation edges** — next: kernel `Relation` native (first); pro consumes via RowSource metadata. 1–2 d. High value.
2. **Active learning correction queue** — pro quality tracker + review queue; next physis-reasoning. 3–4 d. High value, later.
3. **Row-level permissions / tenant manifests** — pro web has tenant map; extend labels. 2 d. Only when multi-tenant demand is real.
4. **Confidence calibration / abstention** — research-shaped, park behind `research/`. Defer.
5. **Materialized operational views** (inventory/quality/maintenance) — compose existing trackers; ship per-workflow on demand, not speculatively.
6. **Connector health metrics** — piggyback on 1–4 (lag = watermark age, cache hit = EmbedCache stats). ~0.5 d after 1+3.

## Sequencing (physis-next priority)

```
Week 1: next #5 (provenance struct in kernel) → next #1 (RowSource contract
        + adapters/sqlite) → next #2 (watermark, ~free on event sourcing)
Week 2: next #3 (per-layer cache stamp in physis-world) + invalidation tests
        pro #5 (adopt provenance struct) → pro #1 (sqlite connector on the
        now-proven contract)
Week 3: pro #2 (watermark) → pro #3 (content cache)
        next #4 queue contract only; pro #4 bounded queue
        capability #1 (FK edges) next kernel Relation first, then pro
```

Contract-first rule: every pro item above consumes a next-proven contract,
so transplants are receipts, not rewrites.

Gate for every item: `cargo check --all-targets` fast; `clippy -D warnings` +
`cargo test` done-gate; next also `physis-check all`.

## Perf targets (measured, from source plan)

- unchanged rescan: zero model inference
- 10k unchanged rows: DB read only, no vector allocation
- 1% changed: ~1% embedding work
- bounded resumable imports
- p50/p95 ingest latency + cost per changed row printed by the fixture benchmark
