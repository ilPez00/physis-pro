//! Attention (plan S8): the terms that decide what the system looks at, and a
//! readout that reads the ranking's own store rather than restating it.
//!
//! `importance(x)` is one function with two consumers — the UI's ranking and
//! compute routing — so the two cannot drift into disagreeing about what
//! matters. The per-term readout exists so a ranking decision is inspectable:
//! every term shows its raw value, the weight applied, and the contribution, and
//! a term the score does not carry is reported as absent rather than as zero
//! with a weight, which would read as "considered and rejected".

use crate::nav::{NavWeights, RetrievalScore};

/// The seven attention terms, in the order the readout reports them.
///
/// Five come from the nav ranking's own score (`semantic`, `lexical`,
/// `recency`, `context`, `pin`), one from the relation graph, and `intent` from
/// the workspace's editable intent — what the work is for is a reason to look at
/// something, not decoration.
pub const ATTENTION_TERMS: [&str; 7] = [
    "intent", "pin", "context", "relation", "vector", "lexical", "recency",
];

/// One term's contribution to a score.
#[derive(Debug, Clone, PartialEq)]
pub struct TermReadout {
    pub term: &'static str,
    /// The raw value the store holds, or `None` when the term is absent.
    pub raw: Option<f32>,
    /// The weight applied, or `None` when the term is absent (a weight with no
    /// value is not a decision the system made).
    pub weight: Option<f32>,
    /// `weight * raw`, or 0 when absent.
    pub contribution: f32,
}

impl TermReadout {
    pub fn present(&self) -> bool {
        self.raw.is_some()
    }

    pub fn render(&self) -> String {
        match (self.raw, self.weight) {
            (Some(r), Some(w)) => format!(
                "{:<9} raw {r:>7.3}  x {w:.2}  = {:>7.3}",
                self.term, self.contribution
            ),
            _ => format!("{:<9} (absent)", self.term),
        }
    }
}

/// The weight the ranking applies to a term. A term the weights do not name has
/// no weight — it is not silently weighted zero.
pub fn weight_of(weights: &NavWeights, term: &str) -> Option<f32> {
    match term {
        // The component is named for the computation. Whether the vectors mean
        // meaning is `nav::is_semantic_kind`'s question, not this table's.
        "vector" => Some(weights.vector),
        "lexical" => Some(weights.lexical),
        "recency" => Some(weights.recency),
        "pin" => Some(weights.pin),
        "context" => Some(weights.context),
        // `relation` and `intent` are carried in the score when they apply; the
        // nav weights do not name them, so their weight is whatever the score
        // already folded in.
        _ => None,
    }
}

/// Read every attention term out of the score's own components.
///
/// The raw value comes from `score.components` — the same store the ranking
/// wrote — so the readout cannot disagree with what was scored. `intent` and
/// `relation` are reported from the score when present.
pub fn readout(score: &RetrievalScore, weights: &NavWeights) -> Vec<TermReadout> {
    ATTENTION_TERMS
        .iter()
        .map(|&term| {
            let raw = score.components.get(term).copied();
            let weight = if raw.is_some() {
                weight_of(weights, term).or(Some(1.0))
            } else {
                None
            };
            TermReadout {
                term,
                raw,
                weight,
                contribution: match (raw, weight) {
                    (Some(r), Some(w)) => r * w,
                    _ => 0.0,
                },
            }
        })
        .collect()
}

/// The importance of a scored object. One function, two consumers: the UI ranks
/// with it and compute routing spends against it, so they agree by construction.
pub fn importance(score: &RetrievalScore) -> f32 {
    score.total
}

/// A whole readout as text, for the HUD and the CLI.
pub fn render(score: &RetrievalScore, weights: &NavWeights) -> String {
    let mut out = String::new();
    out.push_str(&format!("importance {:.3}\n", importance(score)));
    for t in readout(score, weights) {
        out.push_str("  ");
        out.push_str(&t.render());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score_with(pairs: &[(&str, f32)]) -> RetrievalScore {
        let mut s = RetrievalScore::default();
        for (k, v) in pairs {
            s.components.insert((*k).to_string(), *v);
            s.total += *v; // total is set by the ranking; the readout must not recompute it
        }
        s
    }

    #[test]
    fn the_readout_matches_the_score_it_reads() {
        let s = score_with(&[("lexical", 0.4), ("vector", 0.9), ("pin", 1.0)]);
        let w = NavWeights::default();
        let r = readout(&s, &w);
        // Every present term reports exactly the raw value the store holds.
        for t in &r {
            assert_eq!(t.raw, s.components.get(t.term).copied(), "{}", t.term);
        }
        let sem = r.iter().find(|t| t.term == "vector").unwrap();
        assert_eq!(sem.raw, Some(0.9));
        assert_eq!(sem.weight, Some(w.vector));
        assert_eq!(sem.contribution, 0.9 * w.vector);
    }

    #[test]
    fn an_absent_term_is_absent_not_zero_with_a_weight() {
        let s = score_with(&[("lexical", 0.5)]);
        let r = readout(&s, &NavWeights::default());
        let sem = r.iter().find(|t| t.term == "vector").unwrap();
        assert!(!sem.present());
        assert_eq!(sem.raw, None);
        assert_eq!(
            sem.weight, None,
            "a weight with no value would read as considered-and-rejected"
        );
        assert_eq!(sem.contribution, 0.0);
        assert!(sem.render().contains("absent"));
    }

    #[test]
    fn intent_and_relation_are_reported_when_the_score_carries_them() {
        let s = score_with(&[("intent", 0.8), ("relation", 1.0)]);
        let r = readout(&s, &NavWeights::default());
        assert_eq!(
            r.iter().find(|t| t.term == "intent").unwrap().raw,
            Some(0.8)
        );
        assert_eq!(
            r.iter().find(|t| t.term == "relation").unwrap().raw,
            Some(1.0)
        );
    }

    #[test]
    fn importance_is_the_scores_own_total_not_a_recomputation() {
        let mut s = score_with(&[("lexical", 0.5)]);
        s.total = 42.0; // deliberately not the sum of components
        assert_eq!(importance(&s), 42.0);
    }

    #[test]
    fn all_seven_terms_are_reported_in_order() {
        let r = readout(&RetrievalScore::default(), &NavWeights::default());
        assert_eq!(r.len(), 7);
        assert_eq!(
            r.iter().map(|t| t.term).collect::<Vec<_>>(),
            ATTENTION_TERMS.to_vec()
        );
    }
}
