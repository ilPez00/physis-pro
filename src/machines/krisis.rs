//! krisis — the machine of taste / judgement.
//!
//! Grounding: Kant's *Critique of Judgment* and Hume's *Of the Standard of
//! Taste* — judgement as the ordering of the agreeable. The primitive is a
//! **preference relation** (A ≻ B), reduced from pairwise choices into a partial
//! order. The noise is the *intransitive cycle* (A ≻ B ≻ C ≻ A): the incoherent
//! taste that cannot be ranked, rejected exactly as `euclid` rejects the circle.
//! Composing the coherent preferences gives a Copeland ranking; the dream
//! surfaces the contradiction in stated taste.
//!
//! Pure + testable: `detect_preferences` / `ranking` / `cycles` are engine-free;
//! only `register_preferences` touches the core.

use std::collections::{HashMap, HashSet};

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One observed choice: `over` was preferred to `under`.
#[derive(Debug, Clone)]
pub struct Choice {
    pub over: String,
    pub under: String,
}

/// A net preference relation: `over ≻ under`, by `margin` (wins minus losses).
#[derive(Debug, Clone, PartialEq)]
pub struct Preference {
    pub over: String,
    pub under: String,
    pub margin: i32,
    pub hash: String,
}

/// A score for one option in the partial order (Copeland: net options it beats).
#[derive(Debug, Clone, PartialEq)]
pub struct Rank {
    pub option: String,
    pub score: i32,
}

fn net_pairs(choices: &[Choice]) -> HashMap<(String, String), i32> {
    // For each unordered pair, net = (a→b chosen) − (b→a chosen), keyed (a,b) a<b.
    let mut net: HashMap<(String, String), i32> = HashMap::new();
    for c in choices {
        if c.over == c.under {
            continue;
        }
        let (a, b, sign) = if c.over < c.under {
            (c.over.clone(), c.under.clone(), 1)
        } else {
            (c.under.clone(), c.over.clone(), -1)
        };
        *net.entry((a, b)).or_insert(0) += sign;
    }
    net
}

/// Reduce choices to net preference relations (the dominant direction per pair).
/// Ties (net 0) are dropped — an undecided pair is not a preference.
pub fn detect_preferences(choices: &[Choice]) -> Vec<Preference> {
    let net = net_pairs(choices);
    let mut out: Vec<Preference> = net
        .into_iter()
        .filter(|(_, n)| *n != 0)
        .map(|((a, b), n)| {
            let (over, under) = if n > 0 { (a, b) } else { (b, a) };
            let margin = n.abs();
            let hash = format!("krisis_{over}_{under}");
            Preference {
                over,
                under,
                margin,
                hash,
            }
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.margin));
    out
}

/// Copeland ranking: each option scores (preferences it wins) − (it loses).
pub fn ranking(prefs: &[Preference]) -> Vec<Rank> {
    let mut score: HashMap<String, i32> = HashMap::new();
    for p in prefs {
        *score.entry(p.over.clone()).or_insert(0) += 1;
        *score.entry(p.under.clone()).or_insert(0) -= 1;
    }
    let mut out: Vec<Rank> = score
        .into_iter()
        .map(|(option, score)| Rank { option, score })
        .collect();
    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.option.cmp(&b.option)));
    out
}

/// Intransitive 3-cycles in the preference graph (A ≻ B ≻ C ≻ A) — the
/// incoherent taste, the noise. Each cycle returned once as `[a, b, c]`.
pub fn cycles(prefs: &[Preference]) -> Vec<[String; 3]> {
    // adjacency: over → {under}
    let mut adj: HashMap<&str, HashSet<&str>> = HashMap::new();
    for p in prefs {
        adj.entry(&p.over).or_default().insert(&p.under);
    }
    let nodes: Vec<&str> = {
        let mut s: HashSet<&str> = HashSet::new();
        for p in prefs {
            s.insert(&p.over);
            s.insert(&p.under);
        }
        let mut v: Vec<&str> = s.into_iter().collect();
        v.sort_unstable();
        v
    };
    let beats = |x: &str, y: &str| adj.get(x).map(|s| s.contains(y)).unwrap_or(false);
    let mut out = Vec::new();
    for i in 0..nodes.len() {
        for j in 0..nodes.len() {
            for k in 0..nodes.len() {
                let (a, b, c) = (nodes[i], nodes[j], nodes[k]);
                // canonical: a is the smallest of the three, to emit each once
                if a < b && a < c && b != c && beats(a, b) && beats(b, c) && beats(c, a) {
                    out.push([a.to_string(), b.to_string(), c.to_string()]);
                }
            }
        }
    }
    out
}

/// Adapter: register each preference as a directed, bridged relation.
pub fn register_preferences(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    prefs: &[Preference],
) -> Vec<String> {
    let mut ids = Vec::new();
    for p in prefs {
        let over = core.register_node_from_text(&format!("option: {}", p.over), embedder);
        let under = core.register_node_from_text(&format!("option: {}", p.under), embedder);
        core.add_edge_between(&over, &under, p.margin as f32, 1);
        ids.push(over);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(over: &str, under: &str) -> Choice {
        Choice {
            over: over.into(),
            under: under.into(),
        }
    }

    #[test]
    fn net_preferences_and_ranking() {
        let choices = vec![
            c("tea", "coffee"),
            c("tea", "coffee"),
            c("coffee", "tea"),   // net tea≻coffee (1)
            c("tea", "water"),    // tea≻water
            c("coffee", "water"), // coffee≻water
        ];
        let prefs = detect_preferences(&choices);
        assert!(prefs
            .iter()
            .any(|p| p.over == "tea" && p.under == "coffee" && p.margin == 1));
        assert!(prefs.iter().any(|p| p.over == "tea" && p.under == "water"));
        let rank = ranking(&prefs);
        // tea beats 2, coffee net 0, water loses 2
        assert_eq!(rank[0].option, "tea");
        assert_eq!(rank[0].score, 2);
        assert_eq!(rank.last().unwrap().option, "water");
        assert!(cycles(&prefs).is_empty());
    }

    #[test]
    fn intransitive_cycle_is_caught() {
        let choices = vec![c("a", "b"), c("b", "c"), c("c", "a")];
        let prefs = detect_preferences(&choices);
        let cyc = cycles(&prefs);
        assert_eq!(cyc.len(), 1, "one 3-cycle");
        assert_eq!(cyc[0], ["a".to_string(), "b".to_string(), "c".to_string()]);
    }
}
