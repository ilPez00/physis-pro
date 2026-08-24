//! polis — the social-graph machine.
//!
//! Perceive directed interactions (who reached whom, how much); reduce to **tie**
//! primitives with a *reciprocity* (is it mutual?); elaborate by flagging ties
//! that are strengthening (mutual + warm) vs fading (one-sided or cooling). A tie
//! is coherent when it's reciprocated — affinity that flows both ways.
//!
//! Pure + testable.

use std::collections::HashMap;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One directed interaction with a weight (e.g. messages, time, help given).
#[derive(Debug, Clone)]
pub struct Interaction {
    pub from: String,
    pub to: String,
    pub weight: f32,
}

/// An undirected tie between two people.
#[derive(Debug, Clone, PartialEq)]
pub struct Tie {
    pub a: String,
    pub b: String,
    /// Total interaction weight both directions.
    pub strength: f32,
    /// 0–1: how balanced the flow is (min/max of the two directions). 1 = equal.
    pub reciprocity: f32,
}

fn key(x: &str, y: &str) -> (String, String) {
    if x <= y {
        (x.into(), y.into())
    } else {
        (y.into(), x.into())
    }
}

/// Aggregate directed interactions into undirected ties with reciprocity.
pub fn ties(interactions: &[Interaction]) -> Vec<Tie> {
    // (a,b) ordered → (a→b weight, b→a weight)
    let mut flow: HashMap<(String, String), (f32, f32)> = HashMap::new();
    for i in interactions {
        let (a, b) = key(&i.from, &i.to);
        let e = flow.entry((a.clone(), b.clone())).or_insert((0.0, 0.0));
        if i.from == a {
            e.0 += i.weight;
        } else {
            e.1 += i.weight;
        }
    }
    let mut out: Vec<Tie> = flow
        .into_iter()
        .map(|((a, b), (ab, ba))| {
            let strength = ab + ba;
            let (lo, hi) = if ab <= ba { (ab, ba) } else { (ba, ab) };
            let reciprocity = if hi > 0.0 { lo / hi } else { 0.0 };
            Tie {
                a,
                b,
                strength,
                reciprocity,
            }
        })
        .collect();
    out.sort_by(|x, y| {
        y.strength
            .partial_cmp(&x.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Strengthening ties: strong AND reciprocated.
pub fn strengthening(ties: &[Tie], min_strength: f32, min_reciprocity: f32) -> Vec<&Tie> {
    ties.iter()
        .filter(|t| t.strength >= min_strength && t.reciprocity >= min_reciprocity)
        .collect()
}

/// Fading ties: one-sided (low reciprocity) despite some contact — the
/// relationship to tend before it lapses.
pub fn fading(ties: &[Tie], max_reciprocity: f32) -> Vec<&Tie> {
    ties.iter()
        .filter(|t| t.strength > 0.0 && t.reciprocity < max_reciprocity)
        .collect()
}

/// Adapter: register each tie as a labeled, bridged node + a weighted edge
/// between the two people (also bridged as nodes).
pub fn register_ties(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    ties: &[Tie],
) -> Vec<String> {
    let mut ids = Vec::new();
    for t in ties {
        let pa = core.register_node_from_text(&format!("person: {}", t.a), embedder);
        let pb = core.register_node_from_text(&format!("person: {}", t.b), embedder);
        core.add_edge_between(&pa, &pb, t.strength * t.reciprocity, 1);
        ids.push(pa);
        ids.push(pb);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(from: &str, to: &str, w: f32) -> Interaction {
        Interaction {
            from: from.into(),
            to: to.into(),
            weight: w,
        }
    }

    #[test]
    fn ties_aggregate_with_reciprocity() {
        let interactions = vec![
            i("ann", "bob", 5.0),
            i("bob", "ann", 5.0), // balanced, strong
            i("ann", "cy", 8.0),
            i("cy", "ann", 1.0), // one-sided
        ];
        let t = ties(&interactions);
        let ab = t.iter().find(|x| x.a == "ann" && x.b == "bob").unwrap();
        assert_eq!(ab.strength, 10.0);
        assert!((ab.reciprocity - 1.0).abs() < 1e-6);
        let ac = t
            .iter()
            .find(|x| (x.a == "ann" && x.b == "cy") || (x.a == "cy" && x.b == "ann"))
            .unwrap();
        assert!((ac.reciprocity - 0.125).abs() < 1e-6); // 1/8

        assert_eq!(strengthening(&t, 8.0, 0.8).len(), 1); // ann–bob only
        let fade = fading(&t, 0.5);
        assert_eq!(fade.len(), 1); // ann–cy
        assert!(fade[0].a == "ann" || fade[0].b == "ann");
    }
}
