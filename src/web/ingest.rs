use super::*;
use crate::web::{sandbox_existing, SharedState};
use axum::{
    extract::{Json, Query, State},
    http::{HeaderMap, StatusCode},
};

/// Hash-cache path for one watched directory.
///
/// The cache was a single shared file keyed by *basename*, which made every
/// sweep interfere with every other: watching directory B reported all of
/// directory A's files as deleted (they are simply absent from this sweep),
/// which already dropped their coherence nodes and — once the interpretant
/// loop landed — would have recorded a refutation against every entry those
/// files ever matched. Two files sharing a basename across directories
/// collided outright. One cache per directory scopes the diff to what the
/// sweep actually looked at.
pub fn watch_cache_path(data_dir: &std::path::Path, dir: &std::path::Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    dir.hash(&mut h);
    data_dir.join(format!("watch_hash_cache_{:016x}.json", h.finish()))
}

pub async fn scan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ScanRequest>,
) -> Result<Json<ScanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("directory not found: {}", req.dir),
        ));
    }
    // Path-traversal guard: resolve symlinks/`..` and confine the scan to
    // scan_root. Without this, `{"dir":"/etc"}` would expose the host fs.
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);
    let scanner = crate::network::NetworkScanner::new(
        vec![canonical.clone()],
        watch_cache_path(&state.shared.config.data_dir, &canonical),
        0,
    );
    let diffs = scanner.scan_all();
    let files: Vec<crate::scanner::FileInfo> = diffs
        .iter()
        .flat_map(|d| {
            d.new
                .iter()
                .chain(d.changed.iter())
                .chain(d.unchanged.iter())
        })
        .cloned()
        .collect();
    let changed_files: Vec<crate::scanner::FileInfo> = diffs
        .iter()
        .flat_map(|d| d.new.iter().chain(d.changed.iter()))
        .cloned()
        .collect();
    let (goals, changed_labels, removed_paths) = state.with_user(&uid, |s| {
        let (goals, _all_labels) = s.mapper.map_file_infos_labeled(&files);
        // Labels are cheap to derive from the already-scanned FileInfo; only
        // their embeddings are expensive.
        let changed_labels: Vec<String> = changed_files.iter().flat_map(|f| f.labels()).collect();
        s.goals = goals.clone();
        (
            goals,
            changed_labels,
            diffs
                .iter()
                .flat_map(|d| d.deleted.clone())
                .collect::<Vec<_>>(),
        )
    });

    // Register each scanned file as a labeled core node so it's recallable by
    // content in the wiki. Embed with the SHARED embedder (not mapper's internal
    // RandomProjection one) so these vectors are comparable to query vectors.
    // Only new/changed labels reach the shared embedder; unchanged labels are
    // served by the persistent content-addressed cache.
    let (embeddings, embedding_cache_hits, embedding_cache_misses) =
        embed_scan_labels_cached(state.shared.clone(), changed_labels.clone()).await;
    state.with_user(&uid, |s| {
        for file in &changed_files {
            for label in file.labels() {
                let _ = s.core.delete_nodes_by_label_prefix(&label);
            }
        }
        for path in &removed_paths {
            let prefix = path.replace('/', " → ");
            for id in s.core.delete_nodes_by_label_prefix(&prefix) {
                if let Some(db) = state.db() {
                    let _ = db.remove(crate::core::PhysisCore::node_key(&uid, &id));
                }
            }
        }
        for (emb, label) in embeddings.into_iter().zip(changed_labels) {
            let id = s.core.register_node_vec_labeled(emb, Some(label));
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
        // Causal record (Phase 10 D4): a manual /scan is the same producing
        // act watch-mode performs, just driven by hand.
        s.ledger.record(
            crate::address::Mechanism::Generated,
            "scan",
            canonical.display().to_string(),
            None,
        );
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("scan: ledger persist failed: {e}");
            }
        }
    });
    scanner.update_cache(&diffs);

    let summary = diffs.first().map(|d| &d.summary);
    let reused_labels = diffs
        .iter()
        .flat_map(|d| d.unchanged.iter())
        .map(|f| f.labels().len())
        .sum();

    Ok(Json(ScanResponse {
        count: goals.len(),
        goals,
        new: summary.map(|s| s.new).unwrap_or(0),
        changed: summary.map(|s| s.changed).unwrap_or(0),
        deleted: summary.map(|s| s.deleted).unwrap_or(0),
        unchanged: summary.map(|s| s.unchanged).unwrap_or(0),
        reused_labels,
        embedding_cache_hits,
        embedding_cache_misses,
    }))
}

/// POST /api/v1/sql/scan — read a SQLite table in read-only mode and ingest
/// only rows whose stable key is new or whose selected content changed.
pub async fn sql_scan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SqlScanRequest>,
) -> Result<Json<SqlScanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.database);
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve database: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!(
                "database outside scan root ({})",
                state.scan_root().display()
            ),
        ));
    }
    let uid = user_key(&headers);
    let table = req.table.clone();
    let manifest =
        crate::sql::manifest_path(&state.shared.config.data_dir, &uid, &canonical, &table);
    let scan_req = crate::sql::SqlRequest {
        database: canonical.clone(),
        table: req.table,
        key_columns: req.key_columns,
        text_columns: req.text_columns,
        updated_column: req.updated_column,
        reconcile: req.reconcile,
    };
    let diff = tokio::task::spawn_blocking(move || crate::sql::scan(&scan_req, &manifest))
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let new_count = diff.new_rows.len();
    let changed_count = diff.changed_rows.len();
    let deleted_count = diff.deleted_rows.len();
    let mut changed = diff.new_rows;
    changed.extend(diff.changed_rows);
    let texts = changed
        .iter()
        .map(|row| row.text.clone())
        .collect::<Vec<_>>();
    let (embeddings, hits, misses) = embed_texts_cached(state.shared.clone(), texts).await;
    state.with_user(&uid, |s| {
        for row in &changed {
            let _ = s.core.delete_nodes_by_label_prefix(&row.label);
        }
        for row in &diff.deleted_rows {
            let ids = s.core.delete_nodes_by_label_prefix(&row.label);
            if let Some(db) = state.db() {
                for id in ids {
                    let _ = db.remove(crate::core::PhysisCore::node_key(&uid, &id));
                }
            }
        }
        for (row, embedding) in changed.into_iter().zip(embeddings) {
            let id = s.core.register_node_vec_labeled(embedding, Some(row.label));
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
    });
    Ok(Json(SqlScanResponse {
        database: canonical.display().to_string(),
        table,
        total: diff.total,
        new: new_count,
        changed: changed_count,
        deleted: deleted_count,
        unchanged: diff.unchanged,
        embedding_cache_hits: hits,
        embedding_cache_misses: misses,
    }))
}

/// GET /api/v1/sql/schema?database=... — discover SQLite tables and columns
/// without opening the database for writes.
pub async fn sql_schema_handler(
    State(state): State<SharedState>,
    Query(req): Query<SqlSchemaRequest>,
) -> Result<Json<SqlSchemaResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.database);
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve database: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!(
                "database outside scan root ({})",
                state.scan_root().display()
            ),
        ));
    }
    let database = canonical.clone();
    let tables = tokio::task::spawn_blocking(move || crate::sql::schema(&database))
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    Ok(Json(SqlSchemaResponse {
        database: canonical.display().to_string(),
        tables,
    }))
}

/// POST /api/v1/sql/postgres/schema — discover public PostgreSQL tables.
/// Takes the DSN in a **body**, not a query string.
///
/// A PostgreSQL DSN embeds the password (`postgres://user:pass@host/db`).
/// As a query parameter it would be written to access logs, proxy logs,
/// browser history and `Referer` headers — credential disclosure through
/// plumbing nobody thinks of as part of the application.
pub async fn postgres_schema_handler(
    Json(req): Json<PostgresSchemaRequest>,
) -> Result<Json<SqlSchemaResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tables = crate::postgres::schema(&req.dsn)
        .await
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    Ok(Json(SqlSchemaResponse {
        database: "postgres".into(),
        tables,
    }))
}

/// POST /api/v1/sql/postgres/scan — ingest a PostgreSQL public table.
pub async fn postgres_scan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<PostgresScanRequest>,
) -> Result<Json<SqlScanResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.source.trim().is_empty() {
        return Err(err_response(StatusCode::BAD_REQUEST, "source is required"));
    }
    let uid = user_key(&headers);
    let table = req.table.clone();
    let manifest =
        crate::postgres::manifest_path(&state.shared.config.data_dir, &uid, &req.source, &table);
    let scan_req = crate::postgres::Request {
        dsn: req.dsn,
        source: req.source,
        table: req.table,
        key_columns: req.key_columns,
        text_columns: req.text_columns,
    };
    let diff = crate::postgres::scan(&scan_req, &manifest)
        .await
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let new_count = diff.new_rows.len();
    let changed_count = diff.changed_rows.len();
    let deleted_count = diff.deleted_rows.len();
    let mut changed = diff.new_rows;
    changed.extend(diff.changed_rows);
    let texts = changed
        .iter()
        .map(|row| row.text.clone())
        .collect::<Vec<_>>();
    let (embeddings, hits, misses) = embed_texts_cached(state.shared.clone(), texts).await;
    state.with_user(&uid, |s| {
        for row in &changed {
            let _ = s.core.delete_nodes_by_label_prefix(&row.label);
        }
        for row in &diff.deleted_rows {
            let ids = s.core.delete_nodes_by_label_prefix(&row.label);
            if let Some(db) = state.db() {
                for id in ids {
                    let _ = db.remove(crate::core::PhysisCore::node_key(&uid, &id));
                }
            }
        }
        for (row, embedding) in changed.into_iter().zip(embeddings) {
            let id = s.core.register_node_vec_labeled(embedding, Some(row.label));
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
    });
    Ok(Json(SqlScanResponse {
        database: "postgres".into(),
        table,
        total: diff.total,
        new: new_count,
        changed: changed_count,
        deleted: deleted_count,
        unchanged: diff.unchanged,
        embedding_cache_hits: hits,
        embedding_cache_misses: misses,
    }))
}

/// POST /api/v1/watch — one incremental ingest sweep over `dir` (confined to
/// scan_root): diff against the watch hash cache, register new/changed files as
/// labeled core nodes (shared embedder, persisted), drop nodes for removed
/// files, advance the cache. On-demand twin of the PHYSIS_WATCH_DIR background
/// thread — call this to catch up immediately or from a cron, then let the
/// background poller take over.
pub async fn watch_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<WatchRequest>,
) -> Result<Json<WatchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("directory not found: {}", req.dir),
        ));
    }
    // Same confinement as /scan: resolve symlinks/`..` and refuse outside root.
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);

    let cache_path = watch_cache_path(&state.shared.config.data_dir, &canonical);
    let scanner = crate::network::NetworkScanner::new(vec![canonical.clone()], cache_path, 0);
    let diffs = scanner.scan_all();

    let mut new = 0usize;
    let mut changed = 0usize;
    let mut deleted = 0usize;
    let mut registered = 0usize;
    let mut removed = 0usize;

    let any_change = diffs
        .iter()
        .any(|d| d.summary.new + d.summary.changed + d.summary.deleted > 0);
    if any_change {
        let mut labels: Vec<String> = Vec::new();
        for diff in &diffs {
            new += diff.summary.new;
            changed += diff.summary.changed;
            deleted += diff.summary.deleted;
            for f in diff.new.iter().chain(diff.changed.iter()) {
                labels.extend(f.labels());
            }
        }

        // Embed off-thread (CPU-heavy) then register under the lock, same as /scan.
        let (embeddings, _, _) =
            embed_scan_labels_cached(state.shared.clone(), labels.clone()).await;
        state.with_user(&uid, |s| {
            for (emb, label) in embeddings.into_iter().zip(labels) {
                let id = s.core.register_node_vec_labeled(emb, Some(label));
                if let Some(db) = state.db() {
                    let _ = s.core.persist_node(db, &uid, &id);
                }
                registered += 1;
            }
            // Removed files: their nodes live under the structural path prefix.
            for diff in &diffs {
                for p in &diff.deleted {
                    let prefix = p.replace('/', " → ");
                    for id in s.core.delete_nodes_by_label_prefix(&prefix) {
                        if let Some(db) = state.db() {
                            let _ = db.remove(crate::core::PhysisCore::node_key(&uid, &id));
                        }
                        removed += 1;
                    }
                }
            }
        });
    }

    // Interpretant feedback. A file edited after being classified is evidence
    // the frame was useful; a deleted one is evidence it was not. Both come
    // from work the user was doing anyway, which is the point — asking them to
    // grade classifications would yield a trickle of judgements biased toward
    // the moments they felt like grading.
    //
    // `new` files are deliberately not observed: nothing was classified there
    // yet, so there is no prior frame for the event to confirm or refute.
    let observed = record_salience(&state, &uid, &diffs, &canonical).await;

    scanner.update_cache(&diffs);

    let total_nodes = state.with_user(&uid, |s| s.core.nodes.len());
    Ok(Json(WatchResponse {
        new,
        changed,
        deleted,
        registered,
        removed,
        total_nodes,
        observed,
    }))
}

/// POST /api/v1/vault/import — import a knowledge vault (Obsidian/markdown,
/// plain text, git log) as labeled core nodes. Same scan-root confinement as
/// /scan; embedding runs off-thread with the shared embedder so the vault is
/// recallable by content in the wiki.
pub async fn vault_import_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<VaultImportRequest>,
) -> Result<Json<VaultImportResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("directory not found: {}", req.dir),
        ));
    }
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);
    let vault_dir = canonical.display().to_string();

    // Scan notes + optional git log (off-thread; git runs a subprocess).
    let (docs, commits) = tokio::task::spawn_blocking(move || {
        let mut notes = crate::vault::scan_vault(&canonical);
        let commits = if req.git > 0 {
            crate::vault::scan_git_log(&canonical, req.git)
        } else {
            vec![]
        };
        notes.extend(commits.clone());
        (notes, commits.len())
    })
    .await
    .unwrap_or_default();
    let docs_len = docs.len();
    let note_count = docs_len.saturating_sub(commits);

    if docs.is_empty() {
        return Ok(Json(VaultImportResponse {
            docs: 0,
            notes: 0,
            commits: 0,
            registered: 0,
            deduped: 0,
        }));
    }

    // Label + embeddable text per doc; embed off-thread with the shared embedder.
    let pairs = crate::vault::collect_labels(&docs);
    let labels_total = pairs.len();
    let texts: Vec<String> = pairs.iter().map(|(_, body)| body.clone()).collect();
    let embeddings = embed_batch_off_thread(state.shared.clone(), texts).await;

    // Dedupe is by label: count nodes before/after to know how many were new.
    let (registered, deduped) = state.with_user(&uid, |s| {
        let before = s.core.nodes.len();
        for ((label, _), emb) in pairs.into_iter().zip(embeddings) {
            let id = s.core.register_node_vec_labeled(emb, Some(label));
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
        // Causal record (Phase 10 D4): one event per source stream, not per
        // doc — a 500-note vault would flood the ledger; the run is the event.
        if note_count > 0 {
            s.ledger.record(
                crate::address::Mechanism::Received,
                "vault-import",
                &vault_dir,
                None,
            );
        }
        if commits > 0 {
            s.ledger.record(
                crate::address::Mechanism::Derived,
                "git-log",
                format!("{vault_dir} (git ×{commits})"),
                None,
            );
        }
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("vault import: ledger persist failed: {e}");
            }
        }
        let registered = s.core.nodes.len().saturating_sub(before);
        let deduped = labels_total.saturating_sub(registered);
        (registered, deduped)
    });

    Ok(Json(VaultImportResponse {
        docs: docs_len,
        notes: note_count,
        commits,
        registered,
        deduped,
    }))
}

/// POST /api/v1/history/import — import a personal-history export (browser
/// bookmarks/history, OPML feeds, chat JSONL) as labeled core nodes. Same
/// scan-root confinement as /scan; each item is embedded off-thread with the
/// shared embedder so the life-log is recallable by content in the wiki.
pub async fn history_import_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<HistoryImportRequest>,
) -> Result<Json<HistoryImportResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.path);
    if !path.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("file not found: {}", req.path),
        ));
    }
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);
    let export_path = canonical.display().to_string();

    let (docs, name) = tokio::task::spawn_blocking(move || crate::history::import_file(&canonical))
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let items = docs.len();

    if docs.is_empty() {
        return Ok(Json(HistoryImportResponse {
            file: name,
            items: 0,
            registered: 0,
            deduped: 0,
        }));
    }

    let pairs = crate::vault::collect_labels(&docs);
    let labels_total = pairs.len();
    let texts: Vec<String> = pairs.iter().map(|(_, body)| body.clone()).collect();
    let embeddings = embed_batch_off_thread(state.shared.clone(), texts).await;

    let (registered, deduped) = state.with_user(&uid, |s| {
        let before = s.core.nodes.len();
        for ((label, _), emb) in pairs.into_iter().zip(embeddings) {
            let id = s.core.register_node_vec_labeled(emb, Some(label));
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
        // Causal record (Phase 10 D4): the export file is the received
        // artifact; actor names its stream family.
        let stream = history_stream_actor(&name);
        s.ledger.record(
            crate::address::Mechanism::Received,
            stream,
            &export_path,
            None,
        );
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("history import: ledger persist failed: {e}");
            }
        }
        let registered = s.core.nodes.len().saturating_sub(before);
        let deduped = labels_total.saturating_sub(registered);
        (registered, deduped)
    });

    Ok(Json(HistoryImportResponse {
        file: name,
        items,
        registered,
        deduped,
    }))
}

/// Actor name for a personal-history import, from the export file's extension
/// — the ledger reads as `received(bookmarks)` rather than `received(.html)`.
fn history_stream_actor(file_name: &str) -> &'static str {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "bookmarks",
        "json" => "browser-history",
        "opml" | "xml" => "feeds",
        "jsonl" | "txt" => "chat-log",
        _ => "history-export",
    }
}

/// POST /api/v1/praxis/backfill — seed coherence from the real life-log: read a
/// Praxis export (notebook entries + habit trackers), register each item as a
/// labeled behavioral node and assert its verdict (success +1 / inert 0 /
/// failure −1). Same scan-root confinement as the other importers; the verdict
/// becomes a real assertion the dream loop and PDCA can reason over.
pub async fn praxis_backfill_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<PraxisBackfillRequest>,
) -> Result<Json<PraxisBackfillResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.path);
    if !path.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("file not found: {}", req.path),
        ));
    }
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);
    let export_path = canonical.display().to_string();

    let records = tokio::task::spawn_blocking(move || {
        let text = std::fs::read_to_string(&canonical)?;
        Ok::<_, anyhow::Error>(crate::praxis::parse_export(&text))
    })
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
    .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let items = records.len();

    let mut successes = 0usize;
    let mut inert = 0usize;
    let mut failures = 0usize;
    for r in &records {
        match r.status {
            crate::praxis::BehaviourStatus::Success => successes += 1,
            crate::praxis::BehaviourStatus::Inert => inert += 1,
            crate::praxis::BehaviourStatus::Failure => failures += 1,
        }
    }

    if records.is_empty() {
        return Ok(Json(PraxisBackfillResponse {
            items: 0,
            successes,
            inert,
            failures,
            registered: 0,
            deduped: 0,
        }));
    }

    // Embed each record's body, then register labeled + assert the verdict.
    let bodies: Vec<String> = records.iter().map(|r| r.body.clone()).collect();
    let embeddings = embed_batch_off_thread(state.shared.clone(), bodies).await;

    let (registered, deduped) = state.with_user(&uid, |s| {
        let before = s.core.nodes.len();
        for (record, emb) in records.into_iter().zip(embeddings) {
            let label = format!("praxis:{}:{}", record.kind, record.title);
            let id = s.core.register_node_vec_labeled(emb, Some(label));
            s.core.assert_coherence(&id, record.status.as_score());
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
        // Causal record (Phase 10 D4): behavioural nodes derive from the
        // Praxis export.
        s.ledger.record(
            crate::address::Mechanism::Derived,
            "praxis-backfill",
            &export_path,
            None,
        );
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("praxis backfill: ledger persist failed: {e}");
            }
        }
        let registered = s.core.nodes.len().saturating_sub(before);
        let deduped = items.saturating_sub(registered);
        (registered, deduped)
    });

    Ok(Json(PraxisBackfillResponse {
        items,
        successes,
        inert,
        failures,
        registered,
        deduped,
    }))
}

/// GET /api/v1/fs/graph?dir=… — scan a directory into a theme↔file graph.
pub async fn fs_graph_handler(
    State(state): State<SharedState>,
    Query(q): Query<FsGraphQuery>,
) -> Result<Json<FsGraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    let canonical = sandbox_existing(&state, &q.dir)?;
    let root_str = canonical.display().to_string();

    // Theme hubs are the semiotic cells (≤70) and reuse the classifier's
    // *precomputed* centroids — so a scan only embeds the files themselves (a
    // handful), never the ~600 ontology themes. (Re-embedding the ontology per
    // request made BGE scans take minutes; centroids are built once at startup.)
    const TOP_K: usize = 5;
    const MIN_SCORE: f32 = 0.20;
    // file↔file network: each file linked to its top-K nearest neighbours above τ.
    const SIM_TOP_K: usize = 4;
    const SIM_TAU: f32 = 0.55;
    // discovered-mode clustering: corpus carved into emergent groups via spherical
    // k-means (auto-k ≈ √files, or ?k=). Balanced topics + TF-IDF labels.
    const DISCOVER_MIN: usize = 2;
    let discovered = matches!(q.grid.as_deref(), Some("discovered"));
    let want_k = q.k;
    let shared = state.shared.clone();
    let (themes, files, edges, file_edges) = tokio::task::spawn_blocking(move || {
        let root_display = canonical.display().to_string();
        let docs = crate::ingest::ingest_all(&[canonical]);
        // Embed in BATCHES — one ORT session run per chunk amortizes per-call
        // overhead and lets the batch matmul use multiple cores, far cheaper than
        // N separate embed() runs (which serialize on the session Mutex). Order is
        // preserved so file ids stay stable.
        let bodies: Vec<&str> = docs.iter().map(|d| d.body.as_str()).collect();
        let embs: Vec<Vec<f32>> = bodies
            .chunks(32)
            .flat_map(|c| shared.embedder.embed_batch(c))
            .collect();
        let files: Vec<FsFileDto> = docs
            .iter()
            .enumerate()
            .map(|(fid, doc)| {
                // Address at scan time (Phase 10): one root per request, fs
                // metadata read here inside the blocking task.
                let addr = crate::address::FileAddress::derive(
                    &doc.path,
                    std::path::Path::new(&root_display),
                    crate::address::Mechanism::Generated,
                    "scan",
                    chrono::Utc::now(),
                );
                FsFileDto {
                    id: fid,
                    path: doc.path.display().to_string(),
                    title: doc.title.clone(),
                    kind: format!("{:?}", doc.kind),
                    place: Some(addr.space.place),
                    period: Some(addr.time.period),
                    modified: Some(addr.time.modified.to_rfc3339()),
                    mechanism: Some(addr.cause.mechanism.as_str()),
                }
            })
            .collect();
        let texts: Vec<String> = docs.iter().map(|d| d.body.clone()).collect();

        let mut themes: Vec<FsThemeDto> = Vec::new();
        let mut edges: Vec<FsEdgeDto> = Vec::new();

        if discovered {
            // Hubs are the corpus's OWN emergent clusters, labelled by top terms.
            // Clusters below DISCOVER_MIN are lumped into one "Uncategorized" hub so
            // every file appears on the content map.
            let k = want_k
                .unwrap_or_else(|| ((files.len() as f64).sqrt().round() as usize).clamp(2, 16));
            let clusters = crate::axes::cluster_corpus_kmeans(&texts, &embs, k, 8);
            let mut uncategorized: Vec<usize> = Vec::new();
            for c in &clusters {
                if c.members.len() < DISCOVER_MIN {
                    uncategorized.extend(c.members.iter().copied());
                    continue;
                }
                let tid = themes.len();
                themes.push(FsThemeDto {
                    id: tid,
                    name: format!("{} ({})", c.label, c.members.len()),
                    category: Some("discovered".into()),
                    domain: "discovered".into(),
                    mode: c.label.clone(),
                });
                for &fid in &c.members {
                    let score = crate::models::cosine_sim(&embs[fid], &c.centroid);
                    edges.push(FsEdgeDto {
                        theme_id: tid,
                        file_id: fid,
                        score,
                    });
                }
            }
            if !uncategorized.is_empty() {
                let tid = themes.len();
                themes.push(FsThemeDto {
                    id: tid,
                    name: format!("Uncategorized ({})", uncategorized.len()),
                    category: Some("discovered".into()),
                    domain: "discovered".into(),
                    mode: "uncategorized".into(),
                });
                for fid in uncategorized {
                    edges.push(FsEdgeDto {
                        theme_id: tid,
                        file_id: fid,
                        score: 0.5,
                    });
                }
            }
        } else {
            // Fixed 70-cell semiotic grid via precomputed centroids.
            let mut theme_ids: std::collections::HashMap<(String, String), usize> =
                std::collections::HashMap::new();
            for (fid, emb) in embs.iter().enumerate() {
                for cs in shared
                    .classifier
                    .classify(emb)
                    .iter()
                    .take(TOP_K)
                    .filter(|c| c.score > MIN_SCORE)
                {
                    let key = (cs.domain.clone(), cs.mode.clone());
                    let tid = *theme_ids.entry(key).or_insert_with(|| {
                        let id = themes.len();
                        themes.push(FsThemeDto {
                            id,
                            name: format!("{} × {}", cs.domain, cs.mode),
                            category: Some(cs.domain.clone()),
                            domain: cs.domain.clone(),
                            mode: cs.mode.clone(),
                        });
                        id
                    });
                    edges.push(FsEdgeDto {
                        theme_id: tid,
                        file_id: fid,
                        score: cs.score,
                    });
                }
            }
        }

        // file↔file similarity edges: top-K neighbours per file (cosine > τ),
        // deduped to one undirected edge per pair.
        let mut file_edges: Vec<FsSimEdge> = Vec::new();
        let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for i in 0..embs.len() {
            let mut sims: Vec<(usize, f32)> = (0..embs.len())
                .filter(|&j| j != i)
                .map(|j| (j, crate::models::cosine_sim(&embs[i], &embs[j])))
                .filter(|(_, s)| *s > SIM_TAU)
                .collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (j, s) in sims.into_iter().take(SIM_TOP_K) {
                let key = if i < j { (i, j) } else { (j, i) };
                if seen.insert(key) {
                    file_edges.push(FsSimEdge {
                        source: key.0,
                        target: key.1,
                        score: s,
                    });
                }
            }
        }
        (themes, files, edges, file_edges)
    })
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("graph build failed: {e}"),
        )
    })?;

    // Domains present among the cells that attracted a file.
    let mut categories: Vec<String> = themes
        .iter()
        .map(|t| t.domain.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    categories.sort();

    Ok(Json(FsGraphResponse {
        root: root_str,
        categories,
        themes,
        files,
        edges,
        file_edges,
    }))
}

/// POST /api/v1/structsim — flatten two structures into node lists, embed each
/// node, and match them by cosine. Returns matched pairs + missing/extra fields
/// + overall similarity. Both paths are sandboxed to scan_root.
pub async fn structsim_handler(
    State(state): State<SharedState>,
    Json(req): Json<StructSimRequest>,
) -> Result<Json<structsim::StructureGraph>, (StatusCode, Json<ErrorResponse>)> {
    let pa = sandbox_existing(&state, &req.a)?;
    let pb = sandbox_existing(&state, &req.b)?;
    let shared = state.shared.clone();
    let threshold = req.threshold;
    let graph =
        tokio::task::spawn_blocking(move || -> anyhow::Result<structsim::StructureGraph> {
            let ta = std::fs::read_to_string(&pa)?;
            let tb = std::fs::read_to_string(&pb)?;
            let na = flatten_any(&ta);
            let nb = flatten_any(&tb);
            Ok(structsim::compare_node_lists(
                &na,
                &nb,
                shared.embedder.as_ref(),
                threshold,
            ))
        })
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("structsim failed: {e}"),
            )
        })?
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("structsim failed: {e}"),
            )
        })?;
    Ok(Json(graph))
}

/// GET /api/v1/fs/file?path=… — sandboxed text preview (first 16 KB).
pub async fn fs_file_handler(
    State(state): State<SharedState>,
    Query(q): Query<FsFileQuery>,
) -> Result<Json<FsFileResponse>, (StatusCode, Json<ErrorResponse>)> {
    let canonical = sandbox_existing(&state, &q.path)?;
    if canonical.is_dir() {
        return Err(err_response(StatusCode::BAD_REQUEST, "path is a directory"));
    }
    const CAP: usize = 16 * 1024;
    let bytes = std::fs::read(&canonical).map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("read failed: {e}"),
        )
    })?;
    let size = bytes.len() as u64;
    let truncated = bytes.len() > CAP;
    let slice = &bytes[..bytes.len().min(CAP)];
    let content = String::from_utf8_lossy(slice).to_string();
    Ok(Json(FsFileResponse {
        path: canonical.display().to_string(),
        size,
        truncated,
        content,
    }))
}

/// POST /api/v1/fs/move {from,to} — sandboxed rename within scan_root. Refuses
/// to escape the root, overwrite an existing target, or move a directory.
pub async fn fs_move_handler(
    State(state): State<SharedState>,
    Json(req): Json<FsMoveRequest>,
) -> Result<Json<FsMoveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let from = sandbox_existing(&state, &req.from)?;
    if from.is_dir() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "moving directories is not allowed",
        ));
    }
    // The destination does not exist yet, so confine its *parent* to scan_root.
    let to = PathBuf::from(&req.to);
    let parent = to
        .parent()
        .ok_or_else(|| err_response(StatusCode::BAD_REQUEST, "destination has no parent"))?;
    let parent_canon = parent.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve destination dir: {e}"),
        )
    })?;
    if !parent_canon.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "destination outside scan root",
        ));
    }
    let file_name = to
        .file_name()
        .ok_or_else(|| err_response(StatusCode::BAD_REQUEST, "destination has no file name"))?;
    let dest = parent_canon.join(file_name);
    if dest.exists() {
        return Err(err_response(
            StatusCode::CONFLICT,
            "destination already exists",
        ));
    }
    std::fs::rename(&from, &dest).map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("move failed: {e}"),
        )
    })?;
    Ok(Json(FsMoveResponse {
        from: from.display().to_string(),
        to: dest.display().to_string(),
        moved: true,
    }))
}

const FS_SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".cache",
    "dist",
    ".venv",
    "venv",
];
const FS_LIST_CAP: usize = 400;
const FS_RESOLVE_DEPTH: usize = 5;
const FS_RESOLVE_VISIT: usize = 800;

fn skip_fs_name(name: &str) -> bool {
    name.is_empty() || name == "." || name == ".." || FS_SKIP.contains(&name)
}

fn entry_kind_size(path: &std::path::Path) -> Option<(&'static str, Option<u64>)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        let canon = path.canonicalize().ok()?;
        if canon.is_dir() {
            return Some(("dir", None));
        }
        return Some(("file", Some(meta.len())));
    }
    if meta.is_dir() {
        Some(("dir", None))
    } else if meta.is_file() {
        Some(("file", Some(meta.len())))
    } else {
        None
    }
}

fn confined_parent(state: &SharedState, dir: &std::path::Path) -> Option<String> {
    let root = state.scan_root();
    let parent = dir.parent()?;
    if parent.starts_with(&root) {
        Some(parent.display().to_string())
    } else {
        None
    }
}

/// GET /api/v1/fs/list?dir=… — browse one sandboxed directory. No dir → scan root.
/// Hidden names (leading `.`) and bulky build dirs are skipped so the picker
/// stays readable. Paths that canonicalize outside scan_root are omitted.
pub async fn fs_list_handler(
    State(state): State<SharedState>,
    Query(q): Query<FsListQuery>,
) -> Result<Json<FsListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let root = state.scan_root();
    let canonical = match q.dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => sandbox_existing(&state, raw)?,
        None => root.clone(),
    };
    if canonical.is_file() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "path is a file — open its parent folder",
        ));
    }
    let listed = canonical.clone();
    let listed_root = root.clone();
    let (mut entries, truncated) = tokio::task::spawn_blocking(move || {
        let mut entries: Vec<FsEntryDto> = Vec::new();
        let mut truncated = false;
        let rd = match std::fs::read_dir(&listed) {
            Ok(rd) => rd,
            Err(_) => return (entries, truncated),
        };
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if skip_fs_name(&name) || name.starts_with('.') {
                continue;
            }
            let path = ent.path();
            let Ok(canon) = path.canonicalize() else {
                continue;
            };
            if !canon.starts_with(&listed_root) {
                continue;
            }
            let Some((kind, size)) = entry_kind_size(&path) else {
                continue;
            };
            if entries.len() >= FS_LIST_CAP {
                truncated = true;
                break;
            }
            entries.push(FsEntryDto {
                name,
                path: canon.display().to_string(),
                kind: kind.into(),
                size,
            });
        }
        entries.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        // dirs first: "dir" < "file"
        (entries, truncated)
    })
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("list failed: {e}"),
        )
    })?;
    // "dir" sorts before "file" as strings. Keep dirs first even if the
    // blocking sort ran — re-assert here so the UI can rely on it.
    entries.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") => std::cmp::Ordering::Less,
        ("file", "dir") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(Json(FsListResponse {
        root: root.display().to_string(),
        dir: canonical.display().to_string(),
        parent: confined_parent(&state, &canonical),
        entries,
        truncated,
    }))
}

/// GET /api/v1/fs/resolve?name=… — map an OS drop / folder-picker basename onto
/// paths inside scan_root. Depth-limited BFS; exact name, then case-insensitive.
pub async fn fs_resolve_handler(
    State(state): State<SharedState>,
    Query(q): Query<FsResolveQuery>,
) -> Result<Json<FsResolveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let name = q.name.trim().trim_end_matches('/').to_string();
    if name.is_empty() || name.contains('/') || name.contains('\\') || skip_fs_name(&name) {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "name must be a single basename inside the scan root",
        ));
    }
    let root = state.scan_root();
    let needle = name.clone();
    let walk_root = root.clone();
    let matches = tokio::task::spawn_blocking(move || {
        use std::collections::VecDeque;
        let mut q = VecDeque::from([(walk_root.clone(), 0usize)]);
        let mut exact: Vec<FsResolveHit> = Vec::new();
        let mut fuzzy: Vec<FsResolveHit> = Vec::new();
        let mut visits = 0usize;
        let lower = needle.to_lowercase();
        while let Some((dir, depth)) = q.pop_front() {
            if visits >= FS_RESOLVE_VISIT || depth > FS_RESOLVE_DEPTH {
                continue;
            }
            visits += 1;
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for ent in rd.flatten() {
                let n = ent.file_name().to_string_lossy().into_owned();
                if skip_fs_name(&n) {
                    continue;
                }
                let path = ent.path();
                let Ok(canon) = path.canonicalize() else {
                    continue;
                };
                if !canon.starts_with(&walk_root) {
                    continue;
                }
                let Some((kind, _)) = entry_kind_size(&path) else {
                    continue;
                };
                if n == needle {
                    exact.push(FsResolveHit {
                        path: canon.display().to_string(),
                        kind: kind.into(),
                    });
                } else if n.to_lowercase() == lower {
                    fuzzy.push(FsResolveHit {
                        path: canon.display().to_string(),
                        kind: kind.into(),
                    });
                }
                if kind == "dir" && depth < FS_RESOLVE_DEPTH {
                    q.push_back((canon, depth + 1));
                }
            }
        }
        let mut hits = if exact.is_empty() { fuzzy } else { exact };
        hits.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
            ("dir", "file") => std::cmp::Ordering::Less,
            ("file", "dir") => std::cmp::Ordering::Greater,
            _ => a.path.cmp(&b.path),
        });
        hits.truncate(8);
        hits
    })
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("resolve failed: {e}"),
        )
    })?;
    Ok(Json(FsResolveResponse {
        name,
        root: root.display().to_string(),
        matches,
    }))
}

// ── Space·Time·Cause queries (Phase 10 D5) ─────────────────────────────────

#[derive(Deserialize)]
pub struct FilesQueryRequest {
    /// Free-text filter over the event's object and input paths.
    #[serde(default)]
    query: String,
    /// Space facet: events whose object path starts with this root.
    #[serde(default)]
    place: Option<String>,
    /// Cause facet: mechanism name (`generated`/`received`/`derived`/
    /// `corrected`/`cited`/`serves`) or actor (`watch`, `scan`, `vault-import`…).
    #[serde(default)]
    cause: Option<String>,
    /// Time facet: only events within the last N days.
    #[serde(default)]
    within_days: Option<i64>,
    /// Time facet: exact period bucket, `YYYY-MM`.
    #[serde(default)]
    during: Option<String>,
}

/// POST /api/v1/files/query — facet query over the tenant's causal ledger
/// (PLAN Phase 10 D5). Composes space (place prefix), time (within/during)
/// and cause (mechanism/actor/text) filters; matches come back newest first
/// with per-facet rollups so a dashboard can draw distributions without a
/// second call.
pub async fn files_query_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<FilesQueryRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let now = chrono::Utc::now();
        let mut matches: Vec<serde_json::Value> = Vec::new();
        let mut by_place: std::collections::BTreeMap<String, usize> = Default::default();
        let mut by_period: std::collections::BTreeMap<String, usize> = Default::default();
        let mut by_mechanism: std::collections::BTreeMap<String, usize> = Default::default();

        for ev in s.ledger.events() {
            if !req.query.is_empty() {
                let hay = format!(
                    "{} {} {}",
                    ev.object,
                    ev.input.as_deref().unwrap_or(""),
                    ev.actor
                );
                if !hay.to_lowercase().contains(&req.query.to_lowercase()) {
                    continue;
                }
            }
            if let Some(p) = &req.place {
                if !ev.object.starts_with(p.as_str()) {
                    continue;
                }
            }
            if let Some(c) = &req.cause {
                let c = c.to_ascii_lowercase();
                let mech_hit = ev.mechanism.as_str() == c;
                let actor_hit = ev.actor.to_lowercase() == c;
                if !mech_hit && !actor_hit {
                    continue;
                }
            }
            if let Some(d) = req.within_days {
                if (now - ev.at).num_days() > d {
                    continue;
                }
            }
            if let Some(period) = &req.during {
                if crate::address::period_of(ev.at) != *period {
                    continue;
                }
            }

            // Rollups. Place = the artifact's directory (the finest stable
            // grouping available without re-resolving ontology ids).
            let place = std::path::Path::new(&ev.object)
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "/".into());
            *by_place.entry(place).or_default() += 1;
            *by_period
                .entry(crate::address::period_of(ev.at))
                .or_default() += 1;
            *by_mechanism
                .entry(ev.mechanism.as_str().to_string())
                .or_default() += 1;

            matches.push(serde_json::json!({
                "seq": ev.seq,
                "at": ev.at.to_rfc3339(),
                "mechanism": ev.mechanism.as_str(),
                "actor": ev.actor,
                "object": ev.object,
                "input": ev.input,
            }));
        }
        // Newest first: dashboards read the recent window.
        matches.reverse();

        Json(serde_json::json!({
            "total": matches.len(),
            "matches": matches,
            "by_place": by_place,
            "by_period": by_period,
            "by_mechanism": by_mechanism,
        }))
    })
}

/// POST /api/v1/files/serve — record a purpose link (Phase 10 D6): this
/// artifact serves that goal. The final cause is ledger history like every
/// other cause — append-only, no parallel truth.
#[derive(Deserialize)]
pub struct FileServeRequest {
    /// Artifact path (as recorded in the ledger's object field).
    object: String,
    /// Goal the artifact serves.
    goal_id: String,
}

pub async fn files_serve_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<FileServeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let goal_label = state.with_user(&uid, |s| {
        s.goals
            .iter()
            .find(|g| g.id == req.goal_id)
            .map(|g| g.label.clone().unwrap_or_else(|| g.domain_name.clone()))
    });
    let Some(goal_label) = goal_label else {
        return Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("goal {} not found for tenant", req.goal_id),
        ));
    };
    if req.object.trim().is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "object path required",
        ));
    }

    let event = state.with_user(&uid, |s| {
        let ev = s.ledger.record(
            crate::address::Mechanism::Serves,
            "planner",
            req.object.clone(),
            Some(req.goal_id.clone()),
        );
        let view = serde_json::json!({
            "seq": ev.seq,
            "at": ev.at.to_rfc3339(),
            "mechanism": ev.mechanism.as_str(),
            "actor": ev.actor,
            "object": ev.object,
            "input": ev.input,
        });
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("serve: ledger persist failed: {e}");
            }
        }
        view
    });

    Ok(Json(serde_json::json!({
        "recorded": event,
        "goal": {"id": req.goal_id, "label": goal_label},
    })))
}

/// GET /api/v1/files/lineage?path=… — the why-made chain for one artifact:
/// upstream lineage (what produced it, transitively), descendants (what it
/// fed), and purposes (which goals it serves).
#[derive(Deserialize)]
pub struct FileLineageQuery {
    path: String,
}

fn event_view(ev: &crate::ledger::CauseEvent) -> serde_json::Value {
    serde_json::json!({
        "seq": ev.seq,
        "at": ev.at.to_rfc3339(),
        "mechanism": ev.mechanism.as_str(),
        "actor": ev.actor,
        "object": ev.object,
        "input": ev.input,
    })
}

pub async fn files_lineage_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<FileLineageQuery>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let lineage: Vec<serde_json::Value> = s
            .ledger
            .lineage(&q.path)
            .iter()
            .map(|e| event_view(e))
            .collect();
        let descendants: Vec<serde_json::Value> = s
            .ledger
            .descendants(&q.path)
            .iter()
            .map(|e| event_view(e))
            .collect();
        let purposes: Vec<serde_json::Value> = s
            .ledger
            .purposes(&q.path)
            .iter()
            .map(|e| event_view(e))
            .collect();
        Json(serde_json::json!({
            "path": q.path,
            "lineage": lineage,
            "descendants": descendants,
            "purposes": purposes,
        }))
    })
}

/// Newest parseable message Date per source file (path before the `#N`
/// mbox index). Files with no parseable dates are absent → callers fall back
/// to the import moment.
fn source_mail_dates(
    docs: &[crate::mails::MailDoc],
) -> std::collections::HashMap<String, Option<chrono::DateTime<chrono::Utc>>> {
    let mut out: std::collections::HashMap<String, Option<chrono::DateTime<chrono::Utc>>> =
        Default::default();
    for d in docs {
        let src = d.source.split('#').next().unwrap_or(&d.source).to_string();
        let entry = out.entry(src).or_insert(None);
        if let Some(date) = d.date {
            if entry.is_none_or(|cur| date > cur) {
                *entry = Some(date);
            }
        }
    }
    out
}

/// POST /api/v1/mail/import — mailbox ingestion (Phase 10 D7): walk an export
/// dir of `.eml` files and `.mbox` bundles, register one labeled node per
/// message (`mail:<subject> — <from>`), and record `received(mailbox)` cause
/// events per source file. Same scan-root confinement as every importer.
#[derive(Deserialize)]
pub struct MailImportRequest {
    /// Absolute path to a directory of mail exports. Confined to scan_root.
    dir: String,
}

#[derive(serde::Serialize)]
pub struct MailImportResponse {
    files: usize,
    messages: usize,
    registered: usize,
    deduped: usize,
}

pub async fn mail_import_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<MailImportRequest>,
) -> Result<Json<MailImportResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("directory not found: {}", req.dir),
        ));
    }
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    if !canonical.is_dir() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "mail import needs a directory",
        ));
    }
    let uid = user_key(&headers);

    let docs = tokio::task::spawn_blocking(move || crate::mails::scan_mail_dir(&canonical))
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let messages = docs.len();
    // Distinct source files → causal events (one per stream, not per message).
    let sources: Vec<String> = docs
        .iter()
        .map(|d| d.source.split('#').next().unwrap_or(&d.source).to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let files = sources.len();

    if docs.is_empty() {
        return Ok(Json(MailImportResponse {
            files: 0,
            messages: 0,
            registered: 0,
            deduped: 0,
        }));
    }

    let labels: Vec<String> = docs.iter().map(|d| d.label.clone()).collect();
    let bodies: Vec<String> = docs
        .iter()
        .map(|d| format!("{}\n{}", d.label, d.body))
        .collect();
    let embeddings = embed_batch_off_thread(state.shared.clone(), bodies).await;
    // Truthful clock per source file: newest message Date inside it.
    let mail_dates = source_mail_dates(&docs);

    let (registered, deduped) = state.with_user(&uid, |s| {
        let before = s.core.nodes.len();
        for (label, emb) in labels.into_iter().zip(embeddings) {
            let id = s.core.register_node_vec_labeled(emb, Some(label));
            if let Some(db) = state.db() {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
        // Causal record (D7): the mailbox export is received mail; one event
        // per source file keeps lineage at file granularity without flooding.
        // The message's own Date header is the truthful clock — a month-old
        // letter imported today was received a month ago (TimeRef).
        for src in &sources {
            let at = mail_dates
                .get(src)
                .copied()
                .flatten()
                .unwrap_or_else(chrono::Utc::now);
            s.ledger.record_at(
                crate::address::Mechanism::Received,
                "mailbox",
                src.clone(),
                None,
                at,
            );
        }
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("mail import: ledger persist failed: {e}");
            }
        }
        let registered = s.core.nodes.len().saturating_sub(before);
        let deduped = messages.saturating_sub(registered);
        (registered, deduped)
    });

    Ok(Json(MailImportResponse {
        files,
        messages,
        registered,
        deduped,
    }))
}
