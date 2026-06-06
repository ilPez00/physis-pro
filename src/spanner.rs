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
//! ) PRIMARY KEY (id);
//! -- optional property-graph view over WikiNode (+ a WikiEdge table) for GQL.
//! ```
//!
//! ## Status
//! - ✅ node export from core + Spanner `insertOrUpdate` mutation payload shape
//! - ✅ session-create + commit REST calls (compiles; not run against real Spanner)
//! - ⬜ token minting from a service account (currently env bearer token)
//! - ✅ edge export via the id bridge (`export_edges` resolves holarchy edges to
//!   coherence ids; bridge endpoints with `PhysisCore::bridge_node`). A
//!   `WikiEdge(source, target, weight)` table + `upsert_edges` is the next step.
//! - ⬜ incremental sync / deletes (today: full insertOrUpdate upsert)

use serde::Serialize;

use crate::core::PhysisCore;

/// One durable wiki row. Embeddings are intentionally NOT shipped here yet —
/// they're large and re-derivable; add a `vector` column + ANN later if needed.
#[derive(Debug, Clone, Serialize)]
pub struct NodeRow {
    pub id: String,
    pub label: Option<String>,
    pub coherence: f32,
    pub dim: usize,
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

    /// Upsert rows via a single commit of `insertOrUpdate` mutations.
    /// Spanner wants column-major string values; floats/ints are sent as strings.
    pub async fn upsert_nodes(&self, rows: &[NodeRow]) -> anyhow::Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let session = self.create_session().await?;

        let columns = serde_json::json!(["id", "label", "coherence", "dim"]);
        let values: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!([
                    r.id,
                    r.label.clone().unwrap_or_default(),
                    r.coherence.to_string(),
                    r.dim.to_string(),
                ])
            })
            .collect();

        let body = serde_json::json!({
            "mutations": [{
                "insertOrUpdate": {
                    "table": self.table,
                    "columns": columns,
                    "values": values,
                }
            }],
            "singleUseTransaction": { "readWrite": {} }
        });

        let url = format!("https://spanner.googleapis.com/v1/{}:commit", session);
        self.http
            .post(url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(rows.len())
    }
}

/// Convenience: export the core and upsert in one call.
pub async fn sync(core: &PhysisCore, sink: &SpannerSink) -> anyhow::Result<usize> {
    let rows = export_nodes(core);
    sink.upsert_nodes(&rows).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{RandomProjectionEmbedder, VectorEmbed};

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
