//! Ontology discovery mode (PLAN 1D): scan a new environment's texts and
//! propose ontology entries for what the current grid covers poorly.
//!
//! The pipeline is pure and LLM-free: embed every text, keep the ones whose
//! best single-entry cosine falls under a coverage threshold ("unmapped"),
//! greedy-cluster those by cosine, and emit one *draft* entry per cluster —
//! suggested cell (the nearest existing one, even if weak), sample texts, and
//! hint candidates from term frequency. The output is a ready-to-edit ontology
//! JSON: an operator reviews names/cells, drops the file in `config/`, adds
//! one `EXTRA_ONTOLOGIES` row. This is the onboarding-workshop tool — it turns
//! a customer's raw corpus into their custom ontology pack.

use serde::{Deserialize, Serialize};

use crate::classify::CellClassifier;
use crate::embed::VectorEmbed;
use crate::models::cosine_sim;

/// Tunables for a discovery pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Texts whose best entry cosine is below this are "unmapped".
    pub coverage_threshold: f32,
    /// Two unmapped texts join the same cluster above this cosine.
    pub cluster_sim: f32,
    /// Clusters smaller than this are dropped (singletons are usually noise).
    pub min_cluster: usize,
    /// Hint candidates emitted per proposal.
    pub max_hints: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        // 0.55: BGE-base short-text cosine to a *related* entry runs ~0.6-0.8
        // and to unrelated ~0.3-0.5, so 0.55 sits in the gap. But no single
        // default fits every corpus — the report surfaces the real sim
        // distribution so the operator re-runs with a tuned threshold.
        Self { coverage_threshold: 0.55, cluster_sim: 0.70, min_cluster: 2, max_hints: 8 }
    }
}

/// One proposed ontology entry, drafted from a cluster of unmapped texts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedDomain {
    /// Placeholder name from the cluster's most frequent terms — rename it.
    pub name: String,
    /// Nearest existing cell (weak by construction; review it).
    pub domain: String,
    pub mode: String,
    /// Hint candidates by term frequency across the cluster.
    pub hints: Vec<String>,
    /// Up to 3 representative member texts.
    pub samples: Vec<String>,
    /// Cluster size.
    pub count: usize,
    /// Mean best-entry cosine of members — how badly the grid misses them.
    pub coverage: f32,
}

/// Result of a discovery pass over a corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub total: usize,
    pub covered: usize,
    pub unmapped: usize,
    pub proposals: Vec<ProposedDomain>,
    /// Best-entry cosine distribution across the whole corpus — the numbers an
    /// operator needs to pick `coverage_threshold`. Discovery is a tuning tool:
    /// a single magic threshold can't fit every corpus, so we surface the
    /// distribution and let the operator re-run with a value between `mean` and
    /// `max` to target genuinely uncovered text.
    pub sim_min: f32,
    pub sim_mean: f32,
    pub sim_max: f32,
}

/// Tiny stopword list — just enough to keep hints meaningful. Bilingual
/// because the corpora this runs on are Italian shop floors and English logs.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "is",
    "are", "was", "be", "this", "that", "it", "at", "by", "from", "as", "we",
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "di", "del", "della",
    "e", "o", "che", "per", "con", "su", "da", "dal", "dalla", "al", "alla",
    "in", "non", "si", "ha", "sono", "è",
];

fn terms(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.trim_matches('\'').to_lowercase())
        .filter(|w| w.len() > 2 && !STOPWORDS.contains(&w.as_str()))
}

/// Most frequent terms across a cluster, ties broken alphabetically (determinism).
fn top_terms(texts: &[&str], max: usize) -> Vec<String> {
    let mut freq: std::collections::BTreeMap<String, usize> = Default::default();
    for t in texts {
        for w in terms(t) {
            *freq.entry(w).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(max).map(|(w, _)| w).collect()
}

/// Run discovery: which of `texts` does the current ontology fail to cover,
/// and what entries would cover them?
pub fn discover(
    texts: &[String],
    clf: &CellClassifier,
    embedder: &dyn VectorEmbed,
    cfg: &DiscoveryConfig,
) -> DiscoveryReport {
    // Chunk the batch: one giant [N, max_length] pass allocates the whole
    // hidden-state tensor at once (600 texts × 512 tokens × 768 dims ≈ 1GB)
    // and thrashes memory. Small chunks keep the working set flat.
    const EMBED_CHUNK: usize = 16;
    let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(refs.len());
    for chunk in refs.chunks(EMBED_CHUNK) {
        embeddings.extend(embedder.embed_batch(chunk));
    }

    // Split covered / unmapped by best-entry cosine.
    struct Unmapped<'a> {
        text: &'a str,
        embedding: &'a [f32],
        best: f32,
        domain: String,
        mode: String,
    }
    let mut covered = 0usize;
    let mut unmapped: Vec<Unmapped> = Vec::new();
    let (mut sim_min, mut sim_max, mut sim_sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f32);
    for (text, emb) in texts.iter().zip(&embeddings) {
        let (best, d, m) = clf.best_entry_sim(emb).unwrap_or((0.0, "", ""));
        sim_min = sim_min.min(best);
        sim_max = sim_max.max(best);
        sim_sum += best;
        if best >= cfg.coverage_threshold {
            covered += 1;
        } else {
            unmapped.push(Unmapped {
                text,
                embedding: emb,
                best,
                domain: d.to_string(),
                mode: m.to_string(),
            });
        }
    }
    let n = texts.len().max(1) as f32;
    let (sim_min, sim_max) = if texts.is_empty() { (0.0, 0.0) } else { (sim_min, sim_max) };

    // Greedy cosine clustering over the unmapped set (same scheme as
    // certify_branches): first unassigned seeds a cluster, pulls everything
    // above cluster_sim, repeat.
    let mut assigned = vec![false; unmapped.len()];
    let mut proposals: Vec<ProposedDomain> = Vec::new();
    for i in 0..unmapped.len() {
        if assigned[i] {
            continue;
        }
        assigned[i] = true;
        let mut members = vec![i];
        for j in (i + 1)..unmapped.len() {
            if !assigned[j]
                && cosine_sim(unmapped[i].embedding, unmapped[j].embedding) > cfg.cluster_sim
            {
                assigned[j] = true;
                members.push(j);
            }
        }
        if members.len() < cfg.min_cluster {
            continue;
        }
        let member_texts: Vec<&str> = members.iter().map(|&k| unmapped[k].text).collect();
        let hints = top_terms(&member_texts, cfg.max_hints);
        let name = {
            let mut words: Vec<String> = hints.iter().take(2).cloned().collect();
            for w in &mut words {
                if let Some(c) = w.get_mut(0..1) {
                    let up = c.to_uppercase();
                    w.replace_range(0..1, &up);
                }
            }
            if words.is_empty() { "Unnamed Cluster".to_string() } else { words.join(" & ") }
        };
        let coverage =
            members.iter().map(|&k| unmapped[k].best).sum::<f32>() / members.len() as f32;
        proposals.push(ProposedDomain {
            name,
            domain: unmapped[members[0]].domain.clone(),
            mode: unmapped[members[0]].mode.clone(),
            hints,
            samples: member_texts.iter().take(3).map(|s| s.to_string()).collect(),
            count: members.len(),
            coverage,
        });
    }
    proposals.sort_by_key(|p| std::cmp::Reverse(p.count));

    DiscoveryReport {
        total: texts.len(),
        covered,
        unmapped: unmapped.len(),
        proposals,
        sim_min,
        sim_mean: sim_sum / n,
        sim_max,
    }
}

/// Render proposals as a ready-to-edit ontology JSON file (the same shape as
/// every file in `config/`). The operator renames entries, fixes cells, then
/// registers the file in `EXTRA_ONTOLOGIES`.
pub fn to_ontology_json(report: &DiscoveryReport, kind: &str) -> String {
    let domains: Vec<serde_json::Value> = report
        .proposals
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "category": "Discovered",
                "domain": if p.domain.is_empty() { "STUDY" } else { p.domain.as_str() },
                "mode": if p.mode.is_empty() { "WORK" } else { p.mode.as_str() },
                "axis_kind": kind,
                "axis_name": "discovered",
                "unit": "items",
                "hints": p.hints,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "kind": kind, "domains": domains }))
        .expect("ontology json serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OntologyLoader, PhysisConfig};
    use crate::embed::RandomProjectionEmbedder;

    fn clf_and_embedder() -> (CellClassifier, RandomProjectionEmbedder) {
        let ontology = OntologyLoader::load_all(&PhysisConfig::default());
        let embedder = RandomProjectionEmbedder::new(64);
        let clf = CellClassifier::build(&ontology, &embedder);
        (clf, embedder)
    }

    #[test]
    fn test_identical_unmapped_texts_cluster_into_one_proposal() {
        let (clf, embedder) = clf_and_embedder();
        // RP embeddings of identical strings are identical (cosine 1.0), so
        // with an impossible coverage bar everything is unmapped and the
        // duplicates must form one cluster.
        let texts = vec![
            "zorgon flux capacitor recalibration".to_string(),
            "zorgon flux capacitor recalibration".to_string(),
            "zorgon flux capacitor recalibration".to_string(),
        ];
        let cfg = DiscoveryConfig { coverage_threshold: 1.1, ..Default::default() };
        let report = discover(&texts, &clf, &embedder, &cfg);
        assert_eq!(report.unmapped, 3);
        assert_eq!(report.proposals.len(), 1);
        let p = &report.proposals[0];
        assert_eq!(p.count, 3);
        assert!(p.hints.contains(&"zorgon".to_string()), "hints: {:?}", p.hints);
        assert!(!p.name.is_empty());
    }

    #[test]
    fn test_zero_threshold_covers_everything() {
        let (clf, embedder) = clf_and_embedder();
        let texts = vec!["anything at all".to_string(), "another text".to_string()];
        let cfg = DiscoveryConfig { coverage_threshold: -1.0, ..Default::default() };
        let report = discover(&texts, &clf, &embedder, &cfg);
        assert_eq!(report.covered, 2);
        assert_eq!(report.unmapped, 0);
        assert!(report.proposals.is_empty());
    }

    #[test]
    fn test_singletons_dropped_below_min_cluster() {
        let (clf, embedder) = clf_and_embedder();
        // Two very different strings → two singleton clusters → both dropped.
        let texts = vec![
            "wibble grommet anodizing".to_string(),
            "completely unrelated quasar telemetry".to_string(),
        ];
        let cfg = DiscoveryConfig { coverage_threshold: 1.1, ..Default::default() };
        let report = discover(&texts, &clf, &embedder, &cfg);
        assert_eq!(report.unmapped, 2);
        assert!(report.proposals.is_empty(), "singletons must not become proposals");
    }

    #[test]
    fn test_ontology_json_round_trips_through_loader() {
        let report = DiscoveryReport {
            total: 3,
            covered: 0,
            unmapped: 3,
            proposals: vec![ProposedDomain {
                name: "Zorgon & Flux".into(),
                domain: "STUDY".into(),
                mode: "WORK".into(),
                hints: vec!["zorgon".into(), "flux".into()],
                samples: vec!["zorgon flux capacitor".into()],
                count: 3,
                coverage: 0.12,
            }],
            sim_min: 0.1,
            sim_mean: 0.12,
            sim_max: 0.15,
        };
        let json = to_ontology_json(&report, "discovered_test");
        let map = OntologyLoader::load_from_str(&json).expect("draft loads via the real loader");
        assert_eq!(map.len(), 1);
        let def = map.get("Zorgon & Flux").unwrap();
        assert_eq!(def.domain.as_deref(), Some("STUDY"));
        assert!(def.hints.contains(&"zorgon".to_string()));
    }

    #[test]
    fn test_top_terms_skips_stopwords_bilingual() {
        let t = ["la manutenzione del cuscinetto", "the maintenance of the cuscinetto"];
        let terms = top_terms(&t, 4);
        assert!(terms.contains(&"cuscinetto".to_string()));
        assert!(!terms.contains(&"the".to_string()));
        assert!(!terms.contains(&"della".to_string()));
    }
}
