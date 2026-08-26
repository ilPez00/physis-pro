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
//! CREATE TABLE WikiEdge (
//!   source  STRING(64) NOT NULL,
//!   target  STRING(64) NOT NULL,
//!   weight  FLOAT64,
//! ) PRIMARY KEY (source, target);
//! -- optionally define a property graph over WikiNode/WikiEdge for GQL.
//! ```
//!
//! ## Status
//! - ✅ node export + chunked `insertOrUpdate` upsert (≤500 rows/commit)
//! - ✅ `fetch_nodes` read-back (executeSql) — durable rows round-trip
//! - ✅ re-embed: `hydrate_into` regenerates vectors from labels under the live
//!   embedder (stamping the current id); `stale_nodes` flags model-swap targets;
//!   `migrate` does fetch → re-embed → write-back. Pure parts unit-tested.
//! - ✅ edge export via the id bridge (`export_edges`); `WikiEdge` upsert is next
//! - ✅ REST round-trip VERIFIED against the official Spanner emulator in CI
//!   (`spanner-emulator` job: gcloud provisions, `spanner_emulator_roundtrip`
//!   upserts → fetches → asserts nodes AND edges). Host via `SPANNER_REST_ENDPOINT`.
//! - ✅ `WikiEdge` upsert/fetch (edge half persisted + emulator-verified)
//! - ✅ service-account JWT auth — RS256 JWT-bearer → access_token, cached +
//!   self-refreshing (`GOOGLE_APPLICATION_CREDENTIALS` / `GOOGLE_SA_KEY`); the
//!   static `GOOGLE_OAUTH_TOKEN` path still works (emulator/dev). Signing
//!   unit-tested; the token *exchange* is network (not unit-run).
//! - ✅ deletes — hard `delete` mutations (`delete_nodes`/`delete_edges`) +
//!   `sync_full` mirror sync (upsert all, then delete remote orphans; edges
//!   before nodes). Hard deletes chosen over tombstones: rows are re-derivable
//!   from the local core, so soft-delete state buys nothing.
//!
//! Library-only; not yet wired into the server hot path.

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
            (Some(source), Some(target)) => Some(EdgeRow {
                source,
                target,
                weight: w,
            }),
            _ => None,
        })
        .collect()
}

const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const SPANNER_SCOPE: &str = "https://www.googleapis.com/auth/spanner.data";

/// Spanner auth: a static bearer token (emulator / pre-minted), or a service
/// account that mints + caches short-lived access tokens via the OAuth
/// JWT-bearer flow (RS256-signed assertion → access_token). The static path
/// expired after ~1h with no refresh; the service-account path self-refreshes.
enum Auth {
    Static(String),
    ServiceAccount {
        client_email: String,
        private_key: String, // PEM (PKCS#8 or PKCS#1)
        cache: std::sync::Mutex<Option<(String, std::time::Instant)>>,
    },
}

#[derive(serde::Serialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

/// Sign the OAuth JWT-bearer assertion (RS256). Pure — no network.
fn build_assertion(client_email: &str, pem: &str) -> anyhow::Result<String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let claims = JwtClaims {
        iss: client_email,
        scope: SPANNER_SCOPE,
        aud: TOKEN_ENDPOINT,
        iat: now,
        exp: now + 3600,
    };
    let key = EncodingKey::from_rsa_pem(pem.as_bytes())?;
    Ok(encode(&Header::new(Algorithm::RS256), &claims, &key)?)
}

impl Auth {
    /// Parse a service-account key JSON (`{client_email, private_key, ...}`).
    fn from_sa_json(json: &str) -> anyhow::Result<Self> {
        let v: serde_json::Value = serde_json::from_str(json)?;
        let client_email = v
            .get("client_email")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("SA key: no client_email"))?
            .to_string();
        let private_key = v
            .get("private_key")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("SA key: no private_key"))?
            .to_string();
        Ok(Auth::ServiceAccount {
            client_email,
            private_key,
            cache: std::sync::Mutex::new(None),
        })
    }

    /// Current bearer token; mints + caches for service accounts (60s safety margin).
    async fn token(&self, http: &reqwest::Client) -> anyhow::Result<String> {
        match self {
            Auth::Static(t) => Ok(t.clone()),
            Auth::ServiceAccount {
                client_email,
                private_key,
                cache,
            } => {
                if let Some((tok, exp)) = cache.lock().unwrap().as_ref() {
                    if *exp > std::time::Instant::now() {
                        return Ok(tok.clone());
                    }
                }
                let assertion = build_assertion(client_email, private_key)?;
                let resp: serde_json::Value = http
                    .post(TOKEN_ENDPOINT)
                    .form(&[
                        ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                        ("assertion", assertion.as_str()),
                    ])
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                let tok = resp
                    .get("access_token")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| anyhow::anyhow!("token endpoint: no access_token"))?
                    .to_string();
                let ttl = resp
                    .get("expires_in")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(3600);
                let exp = std::time::Instant::now()
                    + std::time::Duration::from_secs(ttl.saturating_sub(60));
                *cache.lock().unwrap() = Some((tok.clone(), exp));
                Ok(tok)
            }
        }
    }
}

/// Config for the Spanner REST endpoint. `database` is the full resource name:
/// `projects/<p>/instances/<i>/databases/<d>`. `host` is the REST origin — the
/// real API by default, or the emulator's REST gateway when overridden.
pub struct SpannerSink {
    /// REST origin, e.g. `https://spanner.googleapis.com` or `http://localhost:9020`.
    host: String,
    database: String,
    table: String,
    edge_table: String,
    auth: Auth,
    http: reqwest::Client,
}

impl SpannerSink {
    /// Build from env: SPANNER_PROJECT, SPANNER_INSTANCE, SPANNER_DATABASE,
    /// SPANNER_TABLE (default `WikiNode`), GOOGLE_OAUTH_TOKEN. Set
    /// SPANNER_REST_ENDPOINT (e.g. `http://localhost:9020`) to target the Spanner
    /// emulator's REST gateway instead of the live API (token is then ignored).
    pub fn from_env() -> anyhow::Result<Self> {
        let project = std::env::var("SPANNER_PROJECT")?;
        let instance = std::env::var("SPANNER_INSTANCE")?;
        let database = std::env::var("SPANNER_DATABASE")?;
        let table = std::env::var("SPANNER_TABLE").unwrap_or_else(|_| "WikiNode".into());
        let edge_table = std::env::var("SPANNER_EDGE_TABLE").unwrap_or_else(|_| "WikiEdge".into());
        // Auth precedence: service-account key (self-refreshing) → static token →
        // "emulator" (the emulator ignores it).
        let auth = if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            Auth::from_sa_json(&std::fs::read_to_string(path)?)?
        } else if let Ok(json) = std::env::var("GOOGLE_SA_KEY") {
            Auth::from_sa_json(&json)?
        } else {
            Auth::Static(std::env::var("GOOGLE_OAUTH_TOKEN").unwrap_or_else(|_| "emulator".into()))
        };
        let host = std::env::var("SPANNER_REST_ENDPOINT")
            .unwrap_or_else(|_| "https://spanner.googleapis.com".into());
        Ok(Self {
            host,
            database: format!("projects/{project}/instances/{instance}/databases/{database}"),
            table,
            edge_table,
            auth,
            http: reqwest::Client::new(),
        })
    }

    /// Build a REST URL: `{host}/v1/{suffix}`.
    fn url(&self, suffix: &str) -> String {
        format!("{}/v1/{}", self.host, suffix)
    }

    fn base(&self) -> String {
        self.url(&self.database)
    }

    /// Create a short-lived session (required before a commit).
    async fn create_session(&self) -> anyhow::Result<String> {
        let url = format!("{}/sessions", self.base());
        let resp: serde_json::Value = self
            .http
            .post(url)
            .bearer_auth(self.auth.token(&self.http).await?)
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
        let commit_url = self.url(&format!("{session}:commit"));
        let columns = serde_json::json!(["id", "label", "coherence", "dim", "embedder"]);

        let mut total = 0usize;
        for chunk in rows.chunks(UPSERT_CHUNK) {
            // Spanner JSON value encoding: INT64 + bytes are strings; FLOAT64 is a
            // JSON number; STRING is a string. So coherence (FLOAT64) is a number,
            // dim (INT64) a string.
            let values: Vec<serde_json::Value> = chunk
                .iter()
                .map(|r| {
                    serde_json::json!([
                        r.id,
                        r.label.clone().unwrap_or_default(),
                        r.coherence,
                        r.dim.to_string(),
                        r.embedder.clone().unwrap_or_default(),
                    ])
                })
                .collect();
            let mutation = serde_json::json!({
                "insertOrUpdate": { "table": self.table, "columns": columns, "values": values }
            });
            self.post_commit(&commit_url, mutation).await?;
            total += chunk.len();
        }
        Ok(total)
    }

    /// Read durable rows back (id/label/coherence/dim/embedder). Embeddings aren't
    /// stored — they're regenerated from `label` on hydrate. Spanner returns every
    /// value as a string; we parse.
    pub async fn fetch_nodes(&self) -> anyhow::Result<Vec<NodeRow>> {
        let session = self.create_session().await?;
        let url = self.url(&format!("{session}:executeSql"));
        let sql = format!(
            "SELECT id, label, coherence, dim, embedder FROM {}",
            self.table
        );
        let resp: serde_json::Value = self
            .http
            .post(url)
            .bearer_auth(self.auth.token(&self.http).await?)
            .json(&serde_json::json!({ "sql": sql }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let opt =
            |s: &serde_json::Value| s.as_str().filter(|x| !x.is_empty()).map(|x| x.to_string());
        let rows = resp
            .get("rows")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|row| {
                let c = row.as_array()?;
                Some(NodeRow {
                    id: c.first()?.as_str()?.to_string(),
                    label: c.get(1).and_then(opt),
                    // FLOAT64 comes back as a JSON number; tolerate a string too.
                    coherence: c
                        .get(2)
                        .and_then(|v| {
                            v.as_f64()
                                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                        })
                        .map(|f| f as f32)
                        .unwrap_or(0.0),
                    dim: c
                        .get(3)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                    embedder: c.get(4).and_then(opt),
                })
            })
            .collect())
    }

    /// Upsert edges into the `WikiEdge` table (chunked). PK = (source, target);
    /// weight is FLOAT64 (JSON number).
    pub async fn upsert_edges(&self, edges: &[EdgeRow]) -> anyhow::Result<usize> {
        if edges.is_empty() {
            return Ok(0);
        }
        let session = self.create_session().await?;
        let commit_url = self.url(&format!("{session}:commit"));
        let columns = serde_json::json!(["source", "target", "weight"]);
        let mut total = 0usize;
        for chunk in edges.chunks(UPSERT_CHUNK) {
            let values: Vec<serde_json::Value> = chunk
                .iter()
                .map(|e| serde_json::json!([e.source, e.target, e.weight]))
                .collect();
            let mutation = serde_json::json!({
                "insertOrUpdate": { "table": self.edge_table, "columns": columns, "values": values }
            });
            self.post_commit(&commit_url, mutation).await?;
            total += chunk.len();
        }
        Ok(total)
    }

    /// POST one commit with a single mutation (shared by upserts and deletes).
    async fn post_commit(
        &self,
        commit_url: &str,
        mutation: serde_json::Value,
    ) -> anyhow::Result<()> {
        let body = serde_json::json!({
            "mutations": [mutation],
            "singleUseTransaction": { "readWrite": {} }
        });
        let resp = self
            .http
            .post(commit_url)
            .bearer_auth(self.auth.token(&self.http).await?)
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("Spanner commit {st}: {txt}");
        }
        Ok(())
    }

    /// Hard-delete nodes by id (chunked `delete` mutations with a keySet).
    /// Chosen over tombstone columns: the wiki is fully re-derivable from the
    /// local core, so a deleted row carries no information worth keeping, and
    /// hard deletes keep the schema and every reader unchanged (PREMORTEM P7).
    pub async fn delete_nodes(&self, ids: &[String]) -> anyhow::Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let session = self.create_session().await?;
        let commit_url = self.url(&format!("{session}:commit"));
        let mut total = 0usize;
        for chunk in ids.chunks(UPSERT_CHUNK) {
            let keys: Vec<serde_json::Value> =
                chunk.iter().map(|id| serde_json::json!([id])).collect();
            let mutation = serde_json::json!({
                "delete": { "table": self.table, "keySet": { "keys": keys } }
            });
            self.post_commit(&commit_url, mutation).await?;
            total += chunk.len();
        }
        Ok(total)
    }

    /// Hard-delete edges by (source, target) PK (chunked).
    pub async fn delete_edges(&self, keys: &[(String, String)]) -> anyhow::Result<usize> {
        if keys.is_empty() {
            return Ok(0);
        }
        let session = self.create_session().await?;
        let commit_url = self.url(&format!("{session}:commit"));
        let mut total = 0usize;
        for chunk in keys.chunks(UPSERT_CHUNK) {
            let keys: Vec<serde_json::Value> = chunk
                .iter()
                .map(|(s, t)| serde_json::json!([s, t]))
                .collect();
            let mutation = serde_json::json!({
                "delete": { "table": self.edge_table, "keySet": { "keys": keys } }
            });
            self.post_commit(&commit_url, mutation).await?;
            total += chunk.len();
        }
        Ok(total)
    }

    /// Read edges back from the `WikiEdge` table.
    pub async fn fetch_edges(&self) -> anyhow::Result<Vec<EdgeRow>> {
        let session = self.create_session().await?;
        let url = self.url(&format!("{session}:executeSql"));
        let sql = format!("SELECT source, target, weight FROM {}", self.edge_table);
        let resp: serde_json::Value = self
            .http
            .post(url)
            .bearer_auth(self.auth.token(&self.http).await?)
            .json(&serde_json::json!({ "sql": sql }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let rows = resp
            .get("rows")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|row| {
                let c = row.as_array()?;
                Some(EdgeRow {
                    source: c.first()?.as_str()?.to_string(),
                    target: c.get(1)?.as_str()?.to_string(),
                    weight: c
                        .get(2)
                        .and_then(|v| {
                            v.as_f64()
                                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                        })
                        .map(|f| f as f32)
                        .unwrap_or(0.0),
                })
            })
            .collect())
    }
}

/// Convenience: export the core and upsert nodes + edges in one call.
/// Returns (nodes_upserted, edges_upserted).
pub async fn sync(core: &PhysisCore, sink: &SpannerSink) -> anyhow::Result<(usize, usize)> {
    let n = sink.upsert_nodes(&export_nodes(core)).await?;
    let e = sink.upsert_edges(&export_edges(core)).await?;
    Ok((n, e))
}

// ── causal-ledger mirror (Phase 10b) ────────────────────────────────────────
//
// Why-made history joins the durable wiki under the SAME schema (no
// migration): an artifact becomes a node keyed `cause:{path}` with its label,
// a cause event `input --mechanism--> object` becomes a directed edge, and a
// `serves` event becomes artifact→goal. Ids are namespaced `cause:` so the
// coherence sync's orphan sweep never mistakes ledger rows for vanished
// coherence nodes — the two mirrors share tables but not lifecycles.

/// Id namespace for ledger-mirrored rows.
pub const CAUSE_NS: &str = "cause:";

fn cause_id(path: &str) -> String {
    format!("{CAUSE_NS}{path}")
}

/// One node per distinct artifact in the ledger (objects + inputs + served
/// goals). The label IS the path — Spanner readers see why-chains without the
/// local store.
pub fn export_ledger_nodes(ledger: &crate::ledger::CausalLedger) -> Vec<NodeRow> {
    let mut ids: Vec<String> = Vec::new();
    for ev in ledger.events() {
        let mut endpoints: Vec<&str> = vec![&ev.object];
        if let Some(i) = &ev.input {
            endpoints.push(i);
        }
        for endpoint in endpoints {
            if !ids.contains(&endpoint.to_string()) {
                ids.push(endpoint.to_string());
            }
        }
    }
    ids.sort();
    ids.into_iter()
        .map(|p| NodeRow {
            id: cause_id(&p),
            label: Some(p),
            coherence: 1.0,
            dim: 0,
            embedder: None,
        })
        .collect()
}

/// One directed edge per event that links two endpoints. Derivations read
/// `input → object` (what produced what); serves events invert —
/// `object → goal` — so the durable graph reads "this artifact serves that
/// purpose". Events without an endpoint pair are attributes of their object
/// node, not edges.
pub fn export_ledger_edges(ledger: &crate::ledger::CausalLedger) -> Vec<EdgeRow> {
    ledger
        .events()
        .iter()
        .filter_map(|ev| {
            let input = ev.input.as_ref()?;
            let (source, target) = if ev.mechanism == crate::address::Mechanism::Serves {
                (ev.object.as_str(), input.as_str())
            } else {
                (input.as_str(), ev.object.as_str())
            };
            Some(EdgeRow {
                source: cause_id(source),
                target: cause_id(target),
                weight: 1.0,
            })
        })
        .collect()
}

/// Upsert the ledger mirror (nodes then edges). Append-only ledger means
/// there is nothing to orphan-sweep: rows are only ever added.
pub async fn sync_ledger(
    sink: &SpannerSink,
    ledger: &crate::ledger::CausalLedger,
) -> anyhow::Result<(usize, usize)> {
    let n = sink.upsert_nodes(&export_ledger_nodes(ledger)).await?;
    let e = sink.upsert_edges(&export_ledger_edges(ledger)).await?;
    Ok((n, e))
}

/// Counts from a full (mirroring) sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncStats {
    pub nodes_upserted: usize,
    pub edges_upserted: usize,
    pub nodes_deleted: usize,
    pub edges_deleted: usize,
}

/// Remote node ids that no longer exist locally — the delete targets.
/// Ledger-mirrored rows (`cause:` namespace) belong to the separate ledger
/// lifecycle and are never coherence orphans.
pub fn orphan_node_ids(remote: &[NodeRow], local: &[NodeRow]) -> Vec<String> {
    let live: std::collections::HashSet<&str> = local.iter().map(|r| r.id.as_str()).collect();
    remote
        .iter()
        .filter(|r| !live.contains(r.id.as_str()) && !r.id.starts_with(CAUSE_NS))
        .map(|r| r.id.clone())
        .collect()
}

/// Remote edge keys that no longer exist locally — the delete targets.
/// Edges touching the `cause:` namespace are ledger-managed, not coherence.
pub fn orphan_edge_keys(remote: &[EdgeRow], local: &[EdgeRow]) -> Vec<(String, String)> {
    let live: std::collections::HashSet<(&str, &str)> = local
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect();
    remote
        .iter()
        .filter(|e| {
            !live.contains(&(e.source.as_str(), e.target.as_str()))
                && !e.source.starts_with(CAUSE_NS)
                && !e.target.starts_with(CAUSE_NS)
        })
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect()
}

/// Full mirror sync: upsert everything local, then hard-delete remote rows that
/// no longer exist locally (nodes AND edges). This closes the PREMORTEM P7
/// divergence item — locally removed nodes no longer linger in Spanner. Order
/// matters: edges are deleted before nodes so a graph reader never sees an edge
/// whose endpoint row is already gone. Never call on the hot path.
pub async fn sync_full(core: &PhysisCore, sink: &SpannerSink) -> anyhow::Result<SyncStats> {
    let local_nodes = export_nodes(core);
    let local_edges = export_edges(core);
    let nodes_upserted = sink.upsert_nodes(&local_nodes).await?;
    let edges_upserted = sink.upsert_edges(&local_edges).await?;

    let stale_edges = orphan_edge_keys(&sink.fetch_edges().await?, &local_edges);
    let edges_deleted = sink.delete_edges(&stale_edges).await?;
    let stale_nodes = orphan_node_ids(&sink.fetch_nodes().await?, &local_nodes);
    let nodes_deleted = sink.delete_nodes(&stale_nodes).await?;

    Ok(SyncStats {
        nodes_upserted,
        edges_upserted,
        nodes_deleted,
        edges_deleted,
    })
}

/// Rows whose embedder id differs from the live one — the re-embed targets.
pub fn stale_nodes<'a>(rows: &'a [NodeRow], live_id: &str) -> Vec<&'a NodeRow> {
    rows.iter()
        .filter(|r| r.embedder.as_deref() != Some(live_id))
        .collect()
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

    fn ledger_with_chain() -> crate::ledger::CausalLedger {
        use crate::address::Mechanism;
        let mut l = crate::ledger::CausalLedger::new();
        l.record(Mechanism::Received, "vault", "note.md", None);
        l.record(
            Mechanism::Derived,
            "pack",
            "weekly-pack.md",
            Some("note.md".into()),
        );
        l.record(
            Mechanism::Serves,
            "planner",
            "note.md",
            Some("goal-1".into()),
        );
        l
    }

    #[test]
    fn ledger_export_namespaces_artifacts_and_links_events() {
        let l = ledger_with_chain();
        let nodes = export_ledger_nodes(&l);
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        // Distinct artifacts only: note.md, weekly-pack.md, goal-1.
        assert_eq!(ids.len(), 3, "{nodes:?}");
        assert!(ids.iter().all(|i| i.starts_with(CAUSE_NS)), "{ids:?}");
        assert!(
            nodes
                .iter()
                .any(|n| n.label.as_deref() == Some("weekly-pack.md")),
            "the path is the label: {nodes:?}"
        );
        let edges = export_ledger_edges(&l);
        assert_eq!(
            edges.len(),
            2,
            "derived + serves carry edges; bare received does not: {edges:?}"
        );
        assert!(
            edges
                .iter()
                .any(|e| e.source == format!("{CAUSE_NS}note.md")
                    && e.target == format!("{CAUSE_NS}weekly-pack.md")),
            "{edges:?}"
        );
        assert!(
            edges
                .iter()
                .any(|e| e.target == format!("{CAUSE_NS}goal-1")),
            "serves links artifact → goal: {edges:?}"
        );
    }

    #[test]
    fn coherence_orphan_sweep_never_touches_cause_rows() {
        let remote = vec![
            NodeRow {
                id: "gone-coherence-node".into(),
                label: None,
                coherence: 0.5,
                dim: 4,
                embedder: None,
            },
            NodeRow {
                id: format!("{CAUSE_NS}note.md"),
                label: None,
                coherence: 1.0,
                dim: 0,
                embedder: None,
            },
        ];
        let local: Vec<NodeRow> = vec![];
        assert_eq!(
            orphan_node_ids(&remote, &local),
            vec!["gone-coherence-node".to_string()],
            "cause rows survive a coherence-only mirror sync"
        );
        let remote_edges = vec![EdgeRow {
            source: format!("{CAUSE_NS}note.md"),
            target: format!("{CAUSE_NS}goal-1"),
            weight: 1.0,
        }];
        assert!(
            orphan_edge_keys(&remote_edges, &[]).is_empty(),
            "cause edges survive a coherence-only mirror sync"
        );
    }

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
    fn test_host_override_builds_emulator_urls() {
        // Verify SPANNER_REST_ENDPOINT redirects every REST URL (env-isolated).
        // (Uses from_env via process env — kept simple: just check `url`.)
        let sink = SpannerSink {
            host: "http://localhost:9020".into(),
            database: "projects/p/instances/i/databases/d".into(),
            table: "WikiNode".into(),
            edge_table: "WikiEdge".into(),
            auth: Auth::Static("emulator".into()),
            http: reqwest::Client::new(),
        };
        assert_eq!(
            sink.base(),
            "http://localhost:9020/v1/projects/p/instances/i/databases/d"
        );
        assert_eq!(sink.url("x:commit"), "http://localhost:9020/v1/x:commit");
    }

    #[test]
    fn test_sa_json_parse_and_assertion_sign() {
        // Throwaway test RSA key (PKCS#8) — not a real credential.
        let pem = include_str!("../tests/fixtures/sa_test_key.pem");
        // Embed it in a minimal SA-key JSON (private_key needs literal \n escapes).
        let json = serde_json::json!({
            "client_email": "test@proj.iam.gserviceaccount.com",
            "private_key": pem,
        })
        .to_string();
        let auth = Auth::from_sa_json(&json).expect("parse SA json");
        let (email, key) = match &auth {
            Auth::ServiceAccount {
                client_email,
                private_key,
                ..
            } => (client_email.clone(), private_key.clone()),
            _ => panic!("expected ServiceAccount"),
        };
        assert_eq!(email, "test@proj.iam.gserviceaccount.com");
        // Signs a valid RS256 JWT (3 dot-separated parts; header decodes to RS256).
        let jwt = build_assertion(&email, &key).expect("sign assertion");
        assert_eq!(jwt.split('.').count(), 3, "JWT must have 3 parts");
        let header = jsonwebtoken::decode_header(&jwt).expect("decode header");
        assert_eq!(header.alg, jsonwebtoken::Algorithm::RS256);
    }

    /// Round-trip against a running Spanner emulator (CI provisions the table via
    /// gcloud). Skips unless SPANNER_REST_ENDPOINT is set. Run with:
    ///   cargo test --features spanner -- --ignored spanner_emulator_roundtrip
    #[tokio::test]
    #[ignore]
    async fn spanner_emulator_roundtrip() {
        if std::env::var("SPANNER_REST_ENDPOINT").is_err() {
            eprintln!("skipping: no SPANNER_REST_ENDPOINT (emulator)");
            return;
        }
        let sink = SpannerSink::from_env().expect("from_env");
        let rows = vec![
            NodeRow {
                id: "n1".into(),
                label: Some("alpha".into()),
                coherence: 0.5,
                dim: 16,
                embedder: Some("bge".into()),
            },
            NodeRow {
                id: "n2".into(),
                label: Some("beta".into()),
                coherence: -1.0,
                dim: 16,
                embedder: None,
            },
        ];
        let up = sink.upsert_nodes(&rows).await.expect("upsert");
        assert_eq!(up, 2);

        let got = sink.fetch_nodes().await.expect("fetch");
        let n1 = got.iter().find(|r| r.id == "n1").expect("n1 round-tripped");
        assert_eq!(n1.label.as_deref(), Some("alpha"));
        assert_eq!(n1.embedder.as_deref(), Some("bge"));
        assert!(got.iter().any(|r| r.id == "n2"));

        // Edges round-trip too.
        let edges = vec![EdgeRow {
            source: "n1".into(),
            target: "n2".into(),
            weight: 0.7,
        }];
        assert_eq!(sink.upsert_edges(&edges).await.expect("upsert edges"), 1);
        let got_edges = sink.fetch_edges().await.expect("fetch edges");
        let e = got_edges
            .iter()
            .find(|e| e.source == "n1" && e.target == "n2")
            .expect("edge round-tripped");
        assert!((e.weight - 0.7).abs() < 1e-5);

        // Hard deletes: drop the edge then n2; both must be gone on read-back.
        assert_eq!(
            sink.delete_edges(&[("n1".into(), "n2".into())])
                .await
                .expect("delete edge"),
            1
        );
        assert!(!sink
            .fetch_edges()
            .await
            .expect("fetch edges after delete")
            .iter()
            .any(|e| e.source == "n1" && e.target == "n2"));
        assert_eq!(
            sink.delete_nodes(&["n2".into()])
                .await
                .expect("delete node"),
            1
        );
        let after = sink.fetch_nodes().await.expect("fetch after delete");
        assert!(
            !after.iter().any(|r| r.id == "n2"),
            "n2 must be hard-deleted"
        );
        assert!(after.iter().any(|r| r.id == "n1"), "n1 must survive");
    }

    #[test]
    fn test_orphan_node_ids_diffs_remote_against_local() {
        let remote = vec![
            row("a", None, None),
            row("b", None, None),
            row("c", None, None),
        ];
        let local = vec![row("b", None, None)];
        let mut orphans = orphan_node_ids(&remote, &local);
        orphans.sort();
        assert_eq!(orphans, vec!["a".to_string(), "c".to_string()]);
        assert!(orphan_node_ids(&remote, &remote).is_empty());
    }

    #[test]
    fn test_orphan_edge_keys_diffs_on_full_pk() {
        let e = |s: &str, t: &str| EdgeRow {
            source: s.into(),
            target: t.into(),
            weight: 1.0,
        };
        let remote = vec![e("a", "b"), e("a", "c")];
        let local = vec![e("a", "b"), e("c", "a")]; // (a,c) gone; (c,a) is a different key
        let orphans = orphan_edge_keys(&remote, &local);
        assert_eq!(orphans, vec![("a".to_string(), "c".to_string())]);
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
