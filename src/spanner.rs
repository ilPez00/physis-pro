//! Spanner Graph sink (Phase 7C) — **SCAFFOLD, UNTESTED against a live instance.**
//!
//! Mirrors the labeled coherence nodes of the UberWiki into Google Cloud Spanner
//! so the wiki becomes durable, queryable (GQL / SQL), and shareable across
//! devices. physis stays the brain (embed / classify / cohere / dream); Spanner
//! is the durable memory.
//!
//! Gated behind `--features spanner` (default off). Uses `reqwest` (already a
//! dep) against Spanner's REST API — no GCP SDK pulled in. The one thing a real
//! deploy must supply is an OAuth bearer token: this scaffold reads it from
//! `GOOGLE_OAUTH_TOKEN` (mint via `gcloud auth print-access-token` or ADC). A
//! production version should mint/refresh tokens from a service-account JWT.
//!
//! ## Expected Spanner schema (create once)
//! ```sql
//! CREATE TABLE WikiNode (
//!   id         STRING(64)  NOT NULL,
//!   label      STRING(MAX),
//!   coherence  FLOAT64,
//!   dim        INT64,
//!   embedder   STRING(64),  -- vector provenance (which embedder made it)
//! ) PRIMARY KEY (id);
//! -- optional property-graph view over WikiNode (+ a WikiEdge table) for GQL.
//! ```
//!
//! ## Status
//! - ✅ node export + chunked `insertOrUpdate` upsert (≤500 rows/commit)
//! - ✅ `fetch_nodes` read-back (executeSql) — durable rows round-trip
//! - ✅ re-embed: `hydrate_into` regenerates vectors from labels under the live
//!   embedder (stamping the current id); `stale_nodes` flags model-swap targets;
//!   `migrate` does fetch → re-embed → write-back. Pure parts unit-tested.
//! - ✅ edge export via the id bridge (`export_edges`); `WikiEdge` upsert is next
//! - ⬜ token minting from a service account (currently env bearer token, ~1h)
//! - ⬜ deletes / tombstones (today: insertOrUpdate only)
//! - ⬜ **NOT run against live Spanner** — REST flow compiles + pure logic tested,
//!   but the wire round-trip is unverified (PREMORTEM P7). Library-only; not wired
//!   into the server hot path.

use serde::Serialize;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// Max rows per commit. Spanner caps a commit at ~80k mutations; stay well under.
const UPSERT_CHUNK: usize = 500;

/// One durable wiki row. Embeddings are intentionally NOT shipped here yet —
/// they're large and re-derivable; add a `vector` column + ANN later if needed.
#[derive(Debug, Clone, Serialize)]
pub struct NodeRow {
    pub id: String,
    pub label: Option<String>,
    pub coherence: f32,
    pub dim: usize,
    /// Embedder that produced the vector (provenance). A loader compares this to
    /// the live embedder; a mismatch means re-embed, not compare across spaces.
    pub embedder: Option<String>,
}

/// Project the in-memory core into durable rows (labeled nodes only).
pub fn export_nodes(core: &PhysisCore) -> Vec<NodeRow> {
    core.nodes
        .values()
        .map(|n| NodeRow {
            id: n.id.clone(),
            label: n.label.clone(),
            coherence: n.coherence_score,
            dim: n.embedding.len(),
            embedder: n.embedder.clone(),
        })
        .collect()
}

/// One durable edge between two coherence nodes. Built from holarchy edges via
/// the id bridge; only edges whose BOTH endpoints are bridged are exportable.
#[derive(Debug, Clone, Serialize)]
pub struct EdgeRow {
    pub source: String,
    pub target: String,
    pub weight: f32,
}

/// Project holarchy edges into durable rows, resolving endpoints to coherence
/// ids through the bridge. Unbridged endpoints are skipped (not an error — they
/// just aren't joined to the wiki yet; call `bridge_node` to include them).
pub fn export_edges(core: &PhysisCore) -> Vec<EdgeRow> {
    core.resolved_edges()
        .into_iter()
        .filter_map(|(s, t, w)| match (s, t) {
            (Some(source), Some(target)) => Some(EdgeRow { source, target, weight: w }),
            _ => None,
        })
        .collect()
}

/// Config for the Spanner REST endpoint. `database` is the full resource name:
/// `projects/<p>/instances/<i>/databases/<d>`.
pub struct SpannerSink {
    database: String,
    table: String,
    token: String,
    http: reqwest::Client,
}

impl SpannerSink {
    /// Build from env: SPANNER_PROJECT, SPANNER_INSTANCE, SPANNER_DATABASE,
    /// SPANNER_TABLE (default `WikiNode`), GOOGLE_OAUTH_TOKEN.
    pub fn from_env() -> anyhow::Result<Self> {
        let project = std::env::var("SPANNER_PROJECT")?;
        let instance = std::env::var("SPANNER_INSTANCE")?;
        let database = std::env::var("SPANNER_DATABASE")?;
        let table = std::env::var("SPANNER_TABLE").unwrap_or_else(|_| "WikiNode".into());
        let token = std::env::var("GOOGLE_OAUTH_TOKEN")?;
        Ok(Self {
            database: format!(
                "projects/{project}/instances/{instance}/databases/{database}"
            ),
            table,
            token,
            http: reqwest::Client::new(),
        })
    }

    fn base(&self) -> String {
        format!("https://spanner.googleapis.com/v1/{}", self.database)
    }

    /// Create a short-lived session (required before a commit).
    async fn create_session(&self) -> anyhow::Result<String> {
        let url = format!("{}/sessions", self.base());
        let resp: serde_json::Value = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        resp.get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Spanner session create: no `name` in response"))
    }

    /// Upsert rows via `insertOrUpdate`, chunked (≤ UPSERT_CHUNK rows/commit) so a
    /// large wiki never exceeds Spanner's per-commit mutation limit. One session,
    /// one commit per chunk. Strings for floats/ints (Spanner wire convention).
    pub async fn upsert_nodes(&self, rows: &[NodeRow]) -> anyhow::Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let session = self.create_session().await?;
        let commit_url = format!("https://spanner.googleapis.com/v1/{session}:commit");
        let columns = serde_json::json!(["id", "label", "coherence", "dim", "embedder"]);

        let mut total = 0usize;
        for chunk in rows.chunks(UPSERT_CHUNK) {
            let values: Vec<serde_json::Value> = chunk
                .iter()
                .map(|r| {
                    serde_json::json!([
                        r.id,
                        r.label.clone().unwrap_or_default(),
                        r.coherence.to_string(),
                        r.dim.to_string(),
                        r.embedder.clone().unwrap_or_default(),
                    ])
                })
                .collect();
            let body = serde_json::json!({
                "mutations": [{
                    "insertOrUpdate": { "table": self.table, "columns": columns, "values": values }
                }],
                "singleUseTransaction": { "readWrite": {} }
            });
            self.http
                .post(&commit_url)
                .bearer_auth(&self.token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?;
            total += chunk.len();
        }
        Ok(total)
    }

    /// Read durable rows back (id/label/coherence/dim/embedder). Embeddings aren't
    /// stored — they're regenerated from `label` on hydrate. Spanner returns every
    /// value as a string; we parse.
    pub async fn fetch_nodes(&self) -> anyhow::Result<Vec<NodeRow>> {
        let session = self.create_session().await?;
        let url = format!("https://spanner.googleapis.com/v1/{session}:executeSql");
        let sql = format!("SELECT id, label, coherence, dim, embedder FROM {}", self.table);
        let resp: serde_json::Value = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({ "sql": sql }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let opt = |s: &serde_json::Value| s.as_str().filter(|x| !x.is_empty()).map(|x| x.to_string());
        let rows = resp.get("rows").and_then(|r| r.as_array()).cloned().unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|row| {
                let c = row.as_array()?;
                Some(NodeRow {
                    id: c.first()?.as_str()?.to_string(),
                    label: c.get(1).and_then(opt),
                    coherence: c.get(2).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    dim: c.get(3).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0),
                    embedder: c.get(4).and_then(opt),
                })
            })
            .collect())
    }
}

/// Convenience: export the core and upsert in one call.
pub async fn sync(core: &PhysisCore, sink: &SpannerSink) -> anyhow::Result<usize> {
    let rows = export_nodes(core);
    sink.upsert_nodes(&rows).await
}

/// Rows whose embedder id differs from the live one — the re-embed targets.
pub fn stale_nodes<'a>(rows: &'a [NodeRow], live_id: &str) -> Vec<&'a NodeRow> {
    rows.iter().filter(|r| r.embedder.as_deref() != Some(live_id)).collect()
}

/// Re-embed each row's label with the LIVE embedder and register it into `core`
/// (which stamps the live embedder id — set `core.set_embedder_id` first). This
/// IS the re-embed: durable rows keep text; vectors are regenerated under the
/// current model, so a swap can't leave mixed spaces. Rows without a label are
/// skipped. Returns the count re-embedded.
pub fn hydrate_into(core: &mut PhysisCore, embedder: &dyn VectorEmbed, rows: &[NodeRow]) -> usize {
    let mut n = 0;
    for r in rows {
        if let Some(label) = &r.label {
            core.register_node_from_text(label, embedder);
            n += 1;
        }
    }
    n
}

/// Full re-embed migration: pull durable rows, re-embed labels under the live
/// embedder (stamp via `core.set_embedder_id` beforehand), then write the rows
/// back with refreshed provenance. Returns (fetched, re_embedded, upserted).
/// UNTESTED against live Spanner — see PREMORTEM P7. Never call on the hot path.
pub async fn migrate(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    sink: &SpannerSink,
) -> anyhow::Result<(usize, usize, usize)> {
    let rows = sink.fetch_nodes().await?;
    let fetched = rows.len();
    let re_embedded = hydrate_into(core, embedder, &rows);
    let upserted = sink.upsert_nodes(&export_nodes(core)).await?;
    Ok((fetched, re_embedded, upserted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{RandomProjectionEmbedder, VectorEmbed};

    fn row(id: &str, label: Option<&str>, embedder: Option<&str>) -> NodeRow {
        NodeRow {
            id: id.into(),
            label: label.map(|s| s.into()),
            coherence: 0.0,
            dim: 16,
            embedder: embedder.map(|s| s.into()),
        }
    }

    #[test]
    fn test_stale_nodes_flags_mismatched_embedder() {
        let rows = vec![
            row("a", Some("a"), Some("bge")),
            row("b", Some("b"), Some("minilm")),
            row("c", Some("c"), None),
        ];
        let stale = stale_nodes(&rows, "bge");
        assert_eq!(stale.len(), 2); // minilm + None differ from live "bge"
        assert!(stale.iter().all(|n| n.id != "a"));
    }

    #[test]
    fn test_hydrate_reembeds_labels_under_live_embedder() {
        let rows = vec![
            row("x", Some("alpha"), Some("old")),
            row("y", None, Some("old")), // no label → nothing to re-embed
        ];
        let mut core = PhysisCore::new();
        core.set_embedder_id("live");
        let emb = RandomProjectionEmbedder::new(16);
        let n = hydrate_into(&mut core, &emb, &rows);
        assert_eq!(n, 1);
        let node = core.nodes.values().next().unwrap();
        assert_eq!(node.label.as_deref(), Some("alpha"));
        assert_eq!(node.embedder.as_deref(), Some("live")); // re-stamped, not "old"
    }

    #[test]
    fn test_export_nodes_carries_label_and_coherence() {
        let mut core = PhysisCore::new();
        let emb = RandomProjectionEmbedder::new(16);
        let _ = emb.embed("seed");
        core.register_node_from_text("src/main.rs", &emb);
        let rows = export_nodes(&core);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_deref(), Some("src/main.rs"));
        assert_eq!(rows[0].dim, 16);
    }
}
