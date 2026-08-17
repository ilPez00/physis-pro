# Physis performance and capability plan

The goal is not merely to make Physis faster. It should become a dependable
decision engine: cheap to update, explicit about provenance, able to explain
its inferences, and safe to run against operational data.

## Immediate architecture

1. **Direct SQL ingestion** — SQLite is the first read-only connector. Add
   Postgres and MySQL behind the same row-delta contract, using stable primary
   keys and source cursors.
2. **CDC/watermarks** — when a source exposes `updated_at`, WAL, logical
   replication, or a change feed, query only the changed window. Keep periodic
   reconciliation scans for deletion and drift detection.
3. **Content-addressed work** — cache parsing, normalized row text,
   embeddings, classifications, and ontology projections independently. A
   model or ontology change should invalidate only the affected layer.
4. **Batch execution** — bounded queues, adaptive batches, ONNX session reuse,
   and back-pressure. Never hold the user/API worker while embedding a large
   source.
5. **Durable provenance** — every node records source, table, row key, source
   version/watermark, model, ontology version, and timestamp. This makes every
   recommendation auditable.

## Capability upgrades

- typed schemas and units rather than flattening every value to text;
- joins and relationship edges from foreign keys, not only semantic
  similarity;
- row-level permissions and tenant-isolated manifests;
- confidence calibration, abstention, and contradiction detection;
- decision simulations that show which source rows and ontology assumptions
  changed the outcome;
- active learning: let an operator correct one classification and propagate
  the correction to similar rows with an explicit review queue;
- materialized operational views for inventory, quality, maintenance, and
  finance workflows;
- connector health, lag, throughput, cache-hit rate, and embedding cost as
  first-class metrics.

## Performance targets

- repeat scan with no source changes: zero model inference;
- 10,000 unchanged rows: database read only, no vector allocation;
- 1% changed rows: approximately 1% of embedding work plus reconciliation;
- bounded memory during imports, with resumable batches;
- deterministic results for the same source snapshot, model, and ontology;
- measured p50/p95 ingest latency and cost per changed row.

The decisive product advantage is an explainable delta: “these 37 rows
changed, these three ontology relations moved, and this decision changed for
these reasons.” That is more valuable than a generic search box and gives
Physis a defensible operational intelligence position.
