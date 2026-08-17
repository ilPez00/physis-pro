# Incremental database and filesystem scanning

`POST /api/v1/scan` and the existing `POST /api/v1/watch` now share a
per-directory SHA-256 manifest and the persistent content-addressed embedding
cache.

Each sweep does four things:

1. Hashes the current inventory and compares it with the previous manifest.
2. Keeps the complete structural inventory for the mapper.
3. Re-embeds only new or changed labels.
4. Removes deleted labels and advances the manifest after processing.

The scan response reports `new`, `changed`, `deleted`, `unchanged`,
`reused_labels`, `embedding_cache_hits`, and `embedding_cache_misses`. An
unchanged database/table export therefore avoids model inference entirely;
changed rows or files are the only work sent through the embedder.

The cache is namespaced by embedder kind and vector dimension. Switching from
random projection to BGE, or changing model dimensions, cannot reuse
incompatible vectors. A missing or corrupt cache is treated as a performance
loss, not as a correctness failure: the next scan rebuilds it.

The first scan is necessarily full. Later scans are incremental. A restart can
still rebuild the cheap structural mapper from the current inventory while
reusing persisted coherence nodes and embeddings.

## Direct SQL connector

GET /api/v1/sql/schema?database=... discovers tables and columns. Then
POST /api/v1/sql/scan imports a selected table. The connector currently
supports SQLite in read-only mode:

```json
{
  "database": "/data/orders.sqlite",
  "table": "orders",
  "key_columns": ["id"],
  "text_columns": ["status", "customer", "amount"],
  "updated_column": "updated_at"
}
```

The connector validates identifiers, discovers the table schema, derives a
stable row key, hashes the selected content, and persists a per-user/table
manifest. It emits only new, changed, and deleted rows to the vector store.
Row content—not only its label—is the embedding-cache key, so an edited row
cannot accidentally reuse its previous vector.

When `updated_column` is supplied, the first scan records its maximum value.
Later scans reconcile only keys for deletion detection and fetch full payloads
from the watermark window. Set `reconcile: true` for a full payload scan when
you need to detect edits made without advancing the source timestamp.

## Database connector boundary

The current public scanner operates on directories, so a database is
incremental when it is presented as an export or mounted data directory. It
does not yet issue database-specific `updated_at`/CDC queries or track row
identity. The next connector layer should persist a source cursor plus a
stable row key, hash only changed rows, and emit the same new/changed/deleted
contract before embedding. That will avoid re-reading unchanged database
rows as well as avoiding their inference.
