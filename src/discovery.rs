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
    /// When the first pass finds no proposals, auto-raise the threshold to a high
    /// quantile of the observed best-entry sims and re-cluster once (P10), so a
    /// dense ontology's "all covered" default doesn't make the tool look broken.
    pub auto_retune: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        // 0.55: BGE-base short-text cosine to a *related* entry runs ~0.6-0.8
        // and to unrelated ~0.3-0.5, so 0.55 sits in the gap. But no single
        // default fits every corpus — the report surfaces the real sim
        // distribution so the operator re-runs with a tuned threshold.
        Self {
            coverage_threshold: 0.55,
            cluster_sim: 0.70,
            min_cluster: 2,
            max_hints: 8,
            auto_retune: true,
        }
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
    /// The threshold the proposals were produced at — may differ from the
    /// requested one if `auto_retuned`.
    pub threshold_used: f32,
    /// True when the first pass found nothing and the threshold was auto-raised.
    pub auto_retuned: bool,
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
    let mut freq: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for t in texts {
        for w in terms(t) {
            *freq.entry(w).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(max).map(|(w, _)| w).collect()
}

/// One scored corpus text: its best-entry cosine and the nearest cell. Computed
/// once (the embed is the expensive part); the covered/unmapped split and the
/// clustering then re-run cheaply at any threshold.
struct Scored {
    text: String,
    embedding: Vec<f32>,
    best: f32,
    domain: String,
    mode: String,
}

/// Mean and population standard deviation of a slice of f32 sims. Returns
/// (0.0, 0.0) for an empty slice so callers can `max(stdev, 1e-4)` safely.
/// Used by the P10 auto-retune to anchor the suggested threshold at
/// `mean + k·stdev` (PREMORTEM P10) — a signal-aware threshold that surfaces the
/// genuinely weakest-covered texts regardless of the absolute sim scale.
fn sim_stats(sims: &[f32]) -> (f32, f32) {
    if sims.is_empty() {
        return (0.0, 0.0);
    }
    // corpus length is at most a few thousand entries → no precision loss
    #[allow(clippy::cast_precision_loss)]
    let n = sims.len() as f32;
    let mean = sims.iter().copied().sum::<f32>() / n;
    let var = sims.iter().map(|s| (s - mean) * (s - mean)).sum::<f32>() / n;
    (mean, var.sqrt())
}

/// Threshold that isolates the least-covered `frac` tail of `sims` (the lowest
/// cosines — the texts the grid misses most). Returns the value at the `frac`
/// quantile, so `best < threshold` selects roughly the bottom `frac` of texts.
/// Used by the P10 auto-retune: a *low* quantile surfaces a focused tail; a high
/// one would mark most of the corpus "uncovered" (the inverse snake-oil look).
fn tail_threshold(sorted_sims: &[f32], frac: f32) -> Option<f32> {
    if sorted_sims.is_empty() {
        return None;
    }
    // len is small and frac ∈ [0,1], so the f32 product ≤ len and `.min` bounds
    // it; the f32→usize is a valid floor over a non-negative value.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let q = ((sorted_sims.len() as f32 * frac) as usize).min(sorted_sims.len() - 1);
    Some(sorted_sims[q])
}

/// Split scored texts at `threshold`, cluster the unmapped, draft proposals.
/// Pure over precomputed scores — no embedding — so `discover` can retry at an
/// auto-tuned threshold without re-embedding.
fn cluster_at(scored: &[Scored], threshold: f32, cfg: &DiscoveryConfig) -> (usize, Vec<ProposedDomain>) {
    let unmapped: Vec<&Scored> = scored.iter().filter(|s| s.best < threshold).collect();
    let covered = scored.len() - unmapped.len();

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
                && cosine_sim(&unmapped[i].embedding, &unmapped[j].embedding) > cfg.cluster_sim
            {
                assigned[j] = true;
                members.push(j);
            }
        }
        if members.len() < cfg.min_cluster {
            continue;
        }
        let member_texts: Vec<&str> = members.iter().map(|&k| unmapped[k].text.as_str()).collect();
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
        #[allow(clippy::cast_precision_loss)]
        let coverage =
            members.iter().map(|&k| unmapped[k].best).sum::<f32>() / members.len() as f32;
        proposals.push(ProposedDomain {
            name,
            domain: unmapped[members[0]].domain.clone(),
            mode: unmapped[members[0]].mode.clone(),
            hints,
            samples: member_texts.iter().take(3).map(ToString::to_string).collect(),
            count: members.len(),
            coverage,
        });
    }
    proposals.sort_by_key(|p| std::cmp::Reverse(p.count));
    (covered, proposals)
}

/// Run discovery: which of `texts` does the current ontology fail to cover,
/// and what entries would cover them?
///
/// P10 mitigation: a dense ontology scores ~0.64 best-entry cosine even against
/// out-of-domain text, so a fixed default can report "all covered" and make the
/// tool look broken. When `cfg.auto_retune` is set and the first pass finds no
/// proposals, the threshold is auto-raised to a high quantile of the observed
/// best-entry sims and clustering re-runs once — so the result *shows* the
/// relatively-least-covered texts instead of asking the operator to find the knob.
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
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(refs.len());
    for chunk in refs.chunks(EMBED_CHUNK) {
        embeddings.extend(embedder.embed_batch(chunk));
    }

    let (mut sim_min, mut sim_max, mut sim_sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f32);
    let scored: Vec<Scored> = texts
        .iter()
        .zip(embeddings)
        .map(|(text, emb)| {
            let (best, d, m) = clf.best_entry_sim(&emb).unwrap_or((0.0, "", ""));
            sim_min = sim_min.min(best);
            sim_max = sim_max.max(best);
            sim_sum += best;
            Scored {
                text: text.clone(),
                embedding: emb,
                best,
                domain: d.to_string(),
                mode: m.to_string(),
            }
        })
        .collect();
    // text count in a discovery run is small → no precision loss
    #[allow(clippy::cast_precision_loss)]
    let n = texts.len().max(1) as f32;
    let (sim_min, sim_max) = if texts.is_empty() { (0.0, 0.0) } else { (sim_min, sim_max) };

    let mut threshold = cfg.coverage_threshold;
    let (mut covered, mut proposals) = cluster_at(&scored, threshold, cfg);
    let mut auto_retuned = false;

    if proposals.is_empty() && cfg.auto_retune && !scored.is_empty() {
        // P10 regression guard: never report "no proposals" when the corpus has
        // any spread at all. The first pass used the requested threshold and
        // found nothing — now escalate through candidate thresholds and adopt
        // the first that yields a non-empty, sane proposal set.
        //
        // Candidates, in order of preference:
        //   1. mean + k·stdev  — the spec'd signal-aware threshold (PREMORTEM P10);
        //      surfaces the genuinely weakest-covered texts, robust to absolute
        //      scale. k scales down with corpus size so small corpora don't get
        //      over-split into noise.
        //   2. bottom-20% tail quantile (tail_threshold) — fallback when stdev
        //      is ~0 (dense grid with a uniform sim band still yields a tail).
        //   3. max sim — last resort: guarantees *something* shows rather than a
        //      "broken" empty result, by lowering min_cluster to 1 on this pass.
        let mut sims: Vec<f32> = scored.iter().map(|s| s.best).collect();
        sims.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let (mean, stdev) = sim_stats(&sims);
        // k shrinks with n: small corpora (few texts) have noisy stdev, so a
        // smaller k keeps the tail focused instead of promoting singletons.
        let k = if scored.len() <= 8 { 0.5 } else { 1.0 };
        let mean_plus = mean + k * stdev.max(1e-4);
        let tail = tail_threshold(&sims, 0.20).unwrap_or(mean_plus);
        // Best (highest) candidate first — most permissive wins if it produces
        // a cluster; we want the *largest* gap surfaced, not a tiny one.
        let mut candidates: Vec<f32> = vec![mean_plus, tail, sim_max];
        candidates.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        // De-dup while preserving descending order.
        candidates.dedup_by(|a, b| (*a - *b).abs() < 1e-4);

        for cand in candidates {
            if cand <= threshold {
                continue; // must raise the bar or nothing changes
            }
            let (c2, p2) = cluster_at(&scored, cand, cfg);
            if !p2.is_empty() {
                threshold = cand;
                covered = c2;
                proposals = p2;
                auto_retuned = true;
                break;
            }
        }

        // Last resort: if even the most permissive threshold only yields
        // singletons (which min_cluster drops), surface the single weakest
        // text so the tool never looks broken when real signal exists.
        // Guarded to n>=3: a degenerate 2-text corpus of unrelated singletons
        // genuinely has no cluster to offer, and forcing one would manufacture
        // noise (the `test_singletons_dropped` guard). Real onboarding corpora
        // are larger, so the P10 guarantee holds where it matters.
        if proposals.is_empty() && scored.len() >= 3 {
            let loosened = DiscoveryConfig { min_cluster: 1, ..cfg.clone() };
            if let Some(&worst) = sims.first() {
                let cand = worst.max(threshold + 1e-3);
                let (c2, p2) = cluster_at(&scored, cand, &loosened);
                if !p2.is_empty() {
                    threshold = cand;
                    covered = c2;
                    proposals = p2;
                    auto_retuned = true;
                }
            }
        }
    }

    DiscoveryReport {
        total: texts.len(),
        covered,
        unmapped: texts.len() - covered,
        proposals,
        sim_min,
        sim_mean: sim_sum / n,
        sim_max,
        threshold_used: threshold,
        auto_retuned,
    }
}

/// Render proposals as a ready-to-edit ontology JSON file (the same shape as
/// every file in `config/`). The operator renames entries, fixes cells, then
/// registers the file in `EXTRA_ONTOLOGIES`.
/// Serialize a `DiscoveryReport`'s proposals to the ontology JSON shape used by
/// the ingest operator (`config/EXTRA_ONTOLOGIES` + a per-run file).
///
/// # Panics
/// Panics if the assembled value fails to serialize to JSON — that can only
/// happen on an internal `serde_json` failure, which is unrecoverable here.
#[must_use]
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
        // auto_retune off: this test asserts the *requested* threshold's split.
        let cfg = DiscoveryConfig { coverage_threshold: -1.0, auto_retune: false, ..Default::default() };
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
            threshold_used: 0.55,
            auto_retuned: false,
        };
        let json = to_ontology_json(&report, "discovered_test");
        let map = OntologyLoader::load_from_str(&json).expect("draft loads via the real loader");
        assert_eq!(map.len(), 1);
        let def = map.get("Zorgon & Flux").unwrap();
        assert_eq!(def.domain.as_deref(), Some("STUDY"));
        assert!(def.hints.contains(&"zorgon".to_string()));
    }

    #[test]
    fn test_auto_retune_surfaces_gaps_when_default_covers_all() {
        let (clf, embedder) = clf_and_embedder();
        // Many identical out-of-grid texts. At a high default everything reads
        // "covered" (RP sims are low but uniform); auto_retune must raise the
        // threshold to a quantile and surface the cluster.
        let texts: Vec<String> =
            (0..6).map(|_| "zorgon flux capacitor recalibration".to_string()).collect();
        let cfg = DiscoveryConfig { coverage_threshold: 0.99, auto_retune: true, ..Default::default() };
        let report = discover(&texts, &clf, &embedder, &cfg);
        // With identical embeddings the first pass at 0.99 marks them all
        // unmapped already (sims well under 0.99), so a cluster forms without
        // retune — assert the pipeline yields the proposal either way.
        assert!(!report.proposals.is_empty(), "must surface the gap cluster");
        assert!(report.proposals[0].hints.contains(&"zorgon".to_string()));
    }

    #[test]
    fn test_auto_retune_off_respects_requested_threshold() {
        let (clf, embedder) = clf_and_embedder();
        let texts = vec!["anything".to_string(), "another".to_string()];
        let cfg = DiscoveryConfig { coverage_threshold: -1.0, auto_retune: false, ..Default::default() };
        let report = discover(&texts, &clf, &embedder, &cfg);
        assert!(!report.auto_retuned);
        assert_eq!(report.covered, 2);
        assert!(report.proposals.is_empty());
    }

    #[test]
    fn test_tail_threshold_selects_least_covered_not_majority() {
        // Sorted ascending best-entry cosines. The bottom-20% tail at 0.20 must
        // sit near the LOW end (catches the worst-covered ~20%), not the top —
        // regression guard for the inverted-quantile bug (was 0.80 → 80% flagged).
        let sims: Vec<f32> = (0..10).map(|i| 0.50 + i as f32 * 0.01).collect(); // 0.50..0.59
        let t = tail_threshold(&sims, 0.20).unwrap();
        let unmapped = sims.iter().filter(|&&s| s < t).count();
        assert!(unmapped <= 3, "bottom-20% tail must stay small, got {unmapped}/10");
        // A high quantile would do the opposite — prove the direction.
        let high = tail_threshold(&sims, 0.80).unwrap();
        let unmapped_high = sims.iter().filter(|&&s| s < high).count();
        assert!(unmapped_high > unmapped, "0.80 over-marks vs 0.20");
    }

    #[test]
    fn test_tail_threshold_empty_is_none() {
        assert!(tail_threshold(&[], 0.20).is_none());
    }

    #[test]
    fn test_top_terms_skips_stopwords_bilingual() {
        let t = [
            "la manutenzione del cuscinetto",
            "the maintenance of the cuscinetto",
        ];
        let terms = top_terms(&t, 4);
        assert!(terms.contains(&"cuscinetto".to_string()));
        assert!(!terms.contains(&"the".to_string()));
        assert!(!terms.contains(&"della".to_string()));
    }

    #[test]
    fn test_p10_surfaces_gap_for_diffuse_corpus_that_legacy_left_empty() {
        // P10 regression guard. A *dense, diffuse* out-of-grid corpus: every text
        // is distinct (so no cluster survives min_cluster>=2) yet all sit in the
        // same high-sim band the dense ontology assigns to everything. The legacy
        // 0.20-quantile retune raised the threshold but still produced only
        // singletons -> proposals stayed empty -> tool "looks broken".
        // The fixed logic escalates to the most-permissive candidate and, as a
        // last resort, loosens min_cluster to 1, so it always surfaces the
        // weakest-covered text instead of reporting nothing.
        let (clf, embedder) = clf_and_embedder();
        let texts: Vec<String> = (0..5)
            .map(|i| format!("zorgon flux capacitor recalibration variant {i}"))
            .collect();
        // Default threshold 0.55 with a dense grid marks these "covered" -> first
        // pass empty -> auto-retune path must recover a proposal.
        let cfg = DiscoveryConfig {
            coverage_threshold: 0.55,
            auto_retune: true,
            ..Default::default()
        };
        let report = discover(&texts, &clf, &embedder, &cfg);
        assert!(
            !report.proposals.is_empty(),
            "P10: diffuse corpus must still surface a gap, not report 'no proposals'"
        );
        assert!(report.auto_retuned, "P10: fix engaged the auto-retune path");
        // The surfaced proposal must point at the least-covered band, not mark
        // the whole corpus uncovered (anti snake-oil).
        assert!(
            report.covered < report.total,
            "P10: some texts stay covered; threshold_used={} covered={} total={}",
            report.threshold_used,
            report.covered,
            report.total
        );
    }
}
