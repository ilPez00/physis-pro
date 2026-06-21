//! Dynamic axis expansion (ONTOLOGY_EXPANSION_PLAN Phase 6) — the axis tier of
//! discovery.
//!
//! `discovery` proposes new *cells* (a domain×mode intersection of axes that
//! already exist). This proposes a new **axis value** — a whole new domain or
//! mode — when incoming data clusters in a region no existing axis can name.
//! New cell ≠ new axis: a cell is an intersection; an axis is a new dimension of
//! carving.
//!
//! The grid is already an entry list (not a fixed 6×6), so the current axes are
//! *derived*: a domain axis value = the centroid of all entries in that domain, a
//! mode axis value = the centroid of all entries in that mode. An **axis-orphan**
//! is a text far from every value on one axis role; a dense, recurring cluster of
//! orphans is a missing dimension → an `AxisProposal`.
//!
//! Pure + testable. The design invariant (Phase 6): no axis without its cause —
//! every proposal carries the source texts (`samples`) that earned it. Installing
//! a proposal (mutating the live grid), space/time scoping, decay and the
//! promote/demote API are the next tier and are intentionally NOT done here.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::models::cosine_sim;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AxisRole {
    Domain,
    Mode,
}

/// A current axis value, derived from the live ontology entries.
#[derive(Debug, Clone, Serialize)]
pub struct Axis {
    pub role: AxisRole,
    pub name: String,
    /// How many cells contribute to this axis value (a coverage/provenance proxy).
    pub cells: usize,
}

/// A proposed new axis value, drafted from a dense cluster of axis-orphans.
/// `samples` is the lineage — the source texts that earned the axis.
#[derive(Debug, Clone, Serialize)]
pub struct AxisProposal {
    pub role: AxisRole,
    /// Placeholder name from the cluster's top terms — an operator renames it.
    pub name: String,
    pub hints: Vec<String>,
    pub samples: Vec<String>,
    pub count: usize,
    /// Mean best-axis cosine of members — how badly the current axes miss them
    /// (lower = a clearer gap).
    pub coverage: f32,
}

/// Group per-cell centroids (keyed `"DOMAIN\x00MODE"`) into axis anchors for a
/// role: average the cell centroids that share the role's name. Returns
/// `(name, anchor, contributing_cell_count)`.
pub fn axis_anchors(centroids: &HashMap<String, Vec<f32>>, role: AxisRole) -> Vec<(String, Vec<f32>, usize)> {
    let mut groups: BTreeMap<String, (Vec<f32>, usize)> = BTreeMap::new();
    for (key, c) in centroids {
        let mut parts = key.split('\u{0}');
        let domain = parts.next().unwrap_or("");
        let mode = parts.next().unwrap_or("");
        let name = match role {
            AxisRole::Domain => domain,
            AxisRole::Mode => mode,
        };
        if name.is_empty() || c.is_empty() {
            continue;
        }
        let e = groups.entry(name.to_string()).or_insert_with(|| (vec![0.0; c.len()], 0));
        if e.0.len() != c.len() {
            continue; // dimension mismatch guard
        }
        for (i, v) in c.iter().enumerate() {
            e.0[i] += v;
        }
        e.1 += 1;
    }
    groups
        .into_iter()
        .map(|(name, (mut sum, n))| {
            if n > 0 {
                for v in sum.iter_mut() {
                    *v /= n as f32;
                }
            }
            (name, sum, n)
        })
        .collect()
}

/// The current axes (both roles), derived from per-cell centroids.
pub fn derive_axes(centroids: &HashMap<String, Vec<f32>>) -> Vec<Axis> {
    let mut out = Vec::new();
    for role in [AxisRole::Domain, AxisRole::Mode] {
        for (name, _anchor, cells) in axis_anchors(centroids, role) {
            out.push(Axis { role, name, cells });
        }
    }
    out
}

/// Best cosine from `e` to any anchor (the axis "coverage" of an embedding).
fn best_axis_sim(e: &[f32], anchors: &[(String, Vec<f32>, usize)]) -> f32 {
    anchors
        .iter()
        .map(|(_, a, _)| cosine_sim(e, a))
        .fold(f32::MIN, f32::max)
}

/// Indices of embeddings that are axis-orphans on this role: their best cosine to
/// any axis value is below `tau` — the existing axes cannot name them.
pub fn axis_orphans(embeddings: &[Vec<f32>], anchors: &[(String, Vec<f32>, usize)], tau: f32) -> Vec<usize> {
    if anchors.is_empty() {
        return (0..embeddings.len()).collect();
    }
    embeddings
        .iter()
        .enumerate()
        .filter(|(_, e)| best_axis_sim(e, anchors) < tau)
        .map(|(i, _)| i)
        .collect()
}

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "is",
    "are", "was", "be", "this", "that", "it", "at", "by", "from", "as", "we",
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "di", "del", "della",
    "e", "o", "che", "per", "con", "su", "da", "non", "si", "ha", "sono",
];

fn top_terms(texts: &[&str], max: usize) -> Vec<String> {
    let mut freq: BTreeMap<String, usize> = BTreeMap::new();
    for t in texts {
        for w in t
            .split(|c: char| !c.is_alphanumeric())
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() > 2 && !STOPWORDS.contains(&w.as_str()))
        {
            *freq.entry(w).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(max).map(|(w, _)| w).collect()
}

/// Reduce axis-orphans to `AxisProposal`s: greedy-cluster the orphan embeddings
/// by cosine (running-mean centroids), drop clusters below `min_cluster` (a lone
/// weird item is noise; a dense recurring cluster is a missing dimension), and
/// draft one proposal per surviving cluster with its lineage (`samples`).
#[allow(clippy::too_many_arguments)]
pub fn propose_axes(
    role: AxisRole,
    texts: &[String],
    embeddings: &[Vec<f32>],
    anchors: &[(String, Vec<f32>, usize)],
    tau: f32,
    cluster_sim: f32,
    min_cluster: usize,
    max_hints: usize,
) -> Vec<AxisProposal> {
    let orphans = axis_orphans(embeddings, anchors, tau);

    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut centroids: Vec<Vec<f32>> = Vec::new();
    for &i in &orphans {
        let e = &embeddings[i];
        let mut placed = false;
        for (ci, c) in centroids.iter_mut().enumerate() {
            if cosine_sim(e, c) >= cluster_sim {
                let n = clusters[ci].len() as f32;
                for (k, v) in c.iter_mut().enumerate() {
                    *v = (*v * n + e[k]) / (n + 1.0);
                }
                clusters[ci].push(i);
                placed = true;
                break;
            }
        }
        if !placed {
            clusters.push(vec![i]);
            centroids.push(e.clone());
        }
    }

    let mut proposals: Vec<AxisProposal> = clusters
        .into_iter()
        .filter(|c| c.len() >= min_cluster)
        .map(|c| {
            let member_texts: Vec<&str> = c.iter().map(|&i| texts[i].as_str()).collect();
            let hints = top_terms(&member_texts, max_hints);
            let name = hints.first().cloned().unwrap_or_else(|| "unnamed-axis".to_string());
            let coverage =
                c.iter().map(|&i| best_axis_sim(&embeddings[i], anchors)).sum::<f32>() / c.len() as f32;
            AxisProposal {
                role,
                name,
                hints,
                samples: member_texts.iter().take(3).map(|s| s.to_string()).collect(),
                count: c.len(),
                coverage,
            }
        })
        .collect();
    // Densest, least-covered first.
    proposals.sort_by(|a, b| {
        b.count.cmp(&a.count).then(a.coverage.partial_cmp(&b.coverage).unwrap_or(std::cmp::Ordering::Equal))
    });
    proposals
}

/// A data-driven cluster over an arbitrary corpus — **no anchors, keeps every
/// cluster** (incl. singletons). Where `propose_axes` finds only what the fixed
/// axes miss, this carves the corpus into its *own* emergent groups: the basis
/// for a "content map" whose hubs are discovered, not predefined.
#[derive(Debug, Clone, Serialize)]
pub struct CorpusCluster {
    /// Indices into the input `texts`/`embeddings`.
    pub members: Vec<usize>,
    /// Top term — the cluster's display label.
    pub label: String,
    /// Top terms (provenance / richer label).
    pub hints: Vec<String>,
    /// Cluster centroid (mean of member embeddings) for downstream scoring.
    pub centroid: Vec<f32>,
}

/// Greedy single-pass cosine clustering (same kernel as `propose_axes`, but over
/// the whole corpus). Returns clusters sorted largest-first.
pub fn cluster_corpus(
    texts: &[String],
    embeddings: &[Vec<f32>],
    cluster_sim: f32,
    max_hints: usize,
) -> Vec<CorpusCluster> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut centroids: Vec<Vec<f32>> = Vec::new();
    for (i, e) in embeddings.iter().enumerate() {
        let mut placed = false;
        for (ci, c) in centroids.iter_mut().enumerate() {
            if cosine_sim(e, c) >= cluster_sim {
                let n = clusters[ci].len() as f32;
                for (k, v) in c.iter_mut().enumerate() {
                    *v = (*v * n + e[k]) / (n + 1.0);
                }
                clusters[ci].push(i);
                placed = true;
                break;
            }
        }
        if !placed {
            clusters.push(vec![i]);
            centroids.push(e.clone());
        }
    }
    let mut out: Vec<CorpusCluster> = clusters
        .into_iter()
        .zip(centroids)
        .map(|(members, centroid)| {
            let member_texts: Vec<&str> = members.iter().map(|&i| texts[i].as_str()).collect();
            let hints = top_terms(&member_texts, max_hints);
            let label = hints.first().cloned().unwrap_or_else(|| "misc".to_string());
            CorpusCluster { members, label, hints, centroid }
        })
        .collect();
    out.sort_by(|a, b| b.members.len().cmp(&a.members.len()));
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Installed axes — the promote/persist/decay tier. An installed axis is a NEW
// axis value an operator earned (from an AxisProposal) and chose to keep. It is
// persisted, becomes a first-class anchor for the axis layer (so its region stops
// producing orphans), and decays when unused. Installing does NOT mutate the
// cell classifier — the axis layer is a self-extending set on top of the derived
// builtin axes.
// ─────────────────────────────────────────────────────────────────────────────

use chrono::{DateTime, Utc};

const AXES_KEY: &[u8] = b"installed_axes_v1";
/// Activation half-life: an axis untouched for ~30d decays to 0.5, etc.
const ACTIVATION_HALF_LIFE_DAYS: f64 = 30.0;

fn default_scope() -> String {
    "global".to_string()
}
fn default_activation() -> f32 {
    1.0
}

/// A promoted axis value. `operator` + `samples` are the lineage — the cause that
/// earned the axis (Phase 6 invariant: no axis without its cause).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisValue {
    pub id: String,
    pub role: AxisRole,
    pub name: String,
    #[serde(default)]
    pub anchor: Vec<f32>,
    #[serde(default)]
    pub hints: Vec<String>,
    /// Who installed it (review path) — part of the cause.
    pub operator: String,
    /// Source texts the proposal clustered — the lineage.
    pub samples: Vec<String>,
    #[serde(default = "default_scope")]
    pub world_scope: String,
    pub created_at: String,
    #[serde(default)]
    pub last_seen: String,
    #[serde(default = "default_activation")]
    pub activation: f32,
    #[serde(default)]
    pub archived: bool,
}

/// Time-decayed activation: stored `activation` scaled by half-life decay since
/// `last_seen` (falls back to `created_at`). An axis that stops being touched fades.
pub fn decayed_activation(av: &AxisValue, now: DateTime<Utc>) -> f32 {
    let anchor_ts = if av.last_seen.is_empty() { &av.created_at } else { &av.last_seen };
    let parsed = DateTime::parse_from_rfc3339(anchor_ts).ok();
    let days = parsed
        .map(|t| (now - t.with_timezone(&Utc)).num_seconds() as f64 / 86_400.0)
        .unwrap_or(0.0)
        .max(0.0);
    let factor = 0.5f64.powf(days / ACTIVATION_HALF_LIFE_DAYS) as f32;
    (av.activation * factor).clamp(0.0, 1.0)
}

/// Installed axes as anchors for a role (active only) — merged with the derived
/// builtin anchors so a promoted axis's region is no longer orphaned.
pub fn installed_anchors(installed: &[AxisValue], role: AxisRole) -> Vec<(String, Vec<f32>, usize)> {
    installed
        .iter()
        .filter(|a| !a.archived && a.role == role && !a.anchor.is_empty())
        .map(|a| (a.name.clone(), a.anchor.clone(), 1usize))
        .collect()
}

/// Durable set of installed axes (JSON under one sled key on the data volume).
#[derive(Default)]
pub struct AxisStore {
    pub axes: Vec<AxisValue>,
}

impl AxisStore {
    pub fn load(db: &sled::Db) -> Self {
        let axes = db
            .get(AXES_KEY)
            .ok()
            .flatten()
            .and_then(|b| serde_json::from_slice::<Vec<AxisValue>>(&b).ok())
            .unwrap_or_default();
        AxisStore { axes }
    }

    pub fn save(&self, db: &sled::Db) {
        if let Ok(bytes) = serde_json::to_vec(&self.axes) {
            let _ = db.insert(AXES_KEY, bytes);
            let _ = db.flush();
        }
    }

    pub fn active(&self) -> Vec<&AxisValue> {
        self.axes.iter().filter(|a| !a.archived).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build per-cell centroids over an orthonormal-ish basis so cosines are clean.
    fn unit(dim: usize, hot: usize) -> Vec<f32> {
        let mut v = vec![0.0; dim];
        v[hot] = 1.0;
        v
    }

    #[test]
    fn derive_axes_groups_domains_and_modes() {
        let mut c = HashMap::new();
        c.insert("HEAL\u{0}REST".to_string(), unit(8, 0));
        c.insert("HEAL\u{0}WORK".to_string(), unit(8, 1));
        c.insert("STUDY\u{0}LEARN".to_string(), unit(8, 2));
        let axes = derive_axes(&c);
        let domains: Vec<_> = axes.iter().filter(|a| a.role == AxisRole::Domain).map(|a| a.name.clone()).collect();
        let modes: Vec<_> = axes.iter().filter(|a| a.role == AxisRole::Mode).map(|a| a.name.clone()).collect();
        assert!(domains.contains(&"HEAL".to_string()) && domains.contains(&"STUDY".to_string()));
        assert!(modes.contains(&"REST".to_string()) && modes.contains(&"WORK".to_string()) && modes.contains(&"LEARN".to_string()));
        // HEAL spans 2 cells.
        assert_eq!(axes.iter().find(|a| a.role == AxisRole::Domain && a.name == "HEAL").unwrap().cells, 2);
    }

    #[test]
    fn proposes_a_new_axis_from_a_dense_orphan_cluster() {
        let dim = 8;
        // Two existing domain anchors on dims 0 and 1.
        let anchors = vec![
            ("HEAL".to_string(), unit(dim, 0), 1usize),
            ("STUDY".to_string(), unit(dim, 1), 1usize),
        ];
        // Four orphan texts clustered on dim 5 (cosine ~0 to both anchors), plus
        // one lone orphan on dim 6 (should be dropped as noise).
        let texts: Vec<String> = vec![
            "logistics route delivery".into(),
            "logistics delivery dispatch".into(),
            "delivery route logistics".into(),
            "logistics dispatch route".into(),
            "one off weird item".into(),
        ];
        let embeddings = vec![unit(dim, 5), unit(dim, 5), unit(dim, 5), unit(dim, 5), unit(dim, 6)];
        let props = propose_axes(AxisRole::Domain, &texts, &embeddings, &anchors, 0.5, 0.9, 2, 5);
        assert_eq!(props.len(), 1, "one dense cluster proposed, singleton dropped");
        let p = &props[0];
        assert_eq!(p.count, 4);
        assert_eq!(p.role, AxisRole::Domain);
        assert!(!p.samples.is_empty(), "lineage present");
        assert!(p.hints.contains(&"logistics".to_string()));
    }
}
