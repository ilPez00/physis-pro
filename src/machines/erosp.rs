//! erosp — the relational machine, **Platonic** genealogy (Symposium 203b–e).
//!
//! Eros is the child of **Penia** (poverty, lack) and **Poros** (resource,
//! plenty): a daimon forever *in between*, never possessing yet never wholly
//! without, always reaching toward what it lacks. So the primitive is an
//! ascending **Longing** — a directed reach from a subject toward an object that
//! holds what the subject lacks. It is *eros* only while the lack persists: a
//! longing fully returned is sated, and is no longer eros but a settled bond
//! (handed to `polis`). Its dream is the longing with drive but no resource —
//! Penia without Poros — the hopeless pursuit that leaves no trace.
//!
//! Twin of `erosa` (the Ares×Aphrodite genealogy): same perception (`Reach`),
//! inverse reduction. Pure + testable: `detect_longings` is engine-free; only
//! `register_longings` touches the core.

use std::collections::HashMap;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One directed reaching: a subject reaching toward an object, with intensity
/// (messages, time, initiative). The shared perception of both eros machines.
#[derive(Debug, Clone)]
pub struct Reach {
    pub subject: String,
    pub object: String,
    pub weight: f32,
}

/// A Platonic erotic vector: a directed longing born of lack and resource.
#[derive(Debug, Clone, PartialEq)]
pub struct Longing {
    pub subject: String,
    pub object: String,
    /// Penia — the lack the subject expresses by reaching out.
    pub penia: f32,
    /// Poros — the resource that returns from the object.
    pub poros: f32,
    /// The unsatisfied longing that *is* eros (Penia beyond what returns).
    pub intensity: f32,
    /// 0–1: ascent potential — drive (Penia) with enough resource (Poros) to
    /// climb Diotima's ladder. Drive with no resource is hopeless, not ascending.
    pub ascent: f32,
    pub hash: String,
}

/// Sum directed reaches into a `(subject, object) → weight` flow.
fn directed_flow(reaches: &[Reach]) -> HashMap<(String, String), f32> {
    let mut flow: HashMap<(String, String), f32> = HashMap::new();
    for r in reaches {
        *flow.entry((r.subject.clone(), r.object.clone())).or_insert(0.0) += r.weight;
    }
    flow
}

/// Reduce directed reaches to Platonic longings. A `subject → object` is a
/// longing while the subject reaches *more* than returns (the lack persists);
/// the sated relation (return ≥ reach) is rejected — it is no longer eros.
pub fn detect_longings(reaches: &[Reach]) -> Vec<Longing> {
    let flow = directed_flow(reaches);
    let mut out = Vec::new();
    for ((subject, object), penia) in flow.iter().map(|(k, v)| (k.clone(), *v)) {
        if penia <= 0.0 {
            continue;
        }
        let poros = *flow.get(&(object.clone(), subject.clone())).unwrap_or(&0.0);
        // Eros lives in the gap: only while lack exceeds what returns.
        if poros >= penia {
            continue; // sated → a settled bond (polis), not eros
        }
        let intensity = penia - poros;
        // Child of BOTH parents: drive (Penia) tempered by some resource (Poros).
        let ascent = (poros / penia).clamp(0.0, 1.0);
        let hash = format!("erosp_{subject}_{object}");
        out.push(Longing { subject, object, penia, poros, intensity, ascent, hash });
    }
    out.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Ascending longings: real drive AND some resource to climb with — the eros
/// that can beget in beauty (Diotima's ladder).
pub fn ascending(longings: &[Longing], min_intensity: f32, min_ascent: f32) -> Vec<&Longing> {
    longings
        .iter()
        .filter(|l| l.intensity >= min_intensity && l.ascent >= min_ascent)
        .collect()
}

/// Hopeless longings: drive with (almost) no resource — Penia without Poros, the
/// inert pursuit that leaves no trace. The dream surfaces these to be released.
pub fn hopeless(longings: &[Longing], max_ascent: f32) -> Vec<&Longing> {
    longings.iter().filter(|l| l.ascent <= max_ascent).collect()
}

/// Adapter: register each longing as a directed, bridged relation in the graph.
pub fn register_longings(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    longings: &[Longing],
) -> Vec<String> {
    let mut ids = Vec::new();
    for l in longings {
        let subject = core.register_node_from_text(&format!("person: {}", l.subject), embedder);
        let longing = core.register_node_from_text(
            &format!("longing: {} -> {}", l.subject, l.object),
            embedder,
        );
        core.bridge_node(&longing);
        core.add_edge_between(&subject, &longing, l.intensity, 1);
        ids.push(longing);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(subject: &str, object: &str, weight: f32) -> Reach {
        Reach { subject: subject.into(), object: object.into(), weight }
    }

    #[test]
    fn longing_persists_while_lack_exceeds_return() {
        let reaches = vec![
            r("ann", "bob", 10.0), r("bob", "ann", 2.0), // ann longs (reaches more)
            r("cy", "dee", 5.0), r("dee", "cy", 5.0),    // sated, balanced
        ];
        let longings = detect_longings(&reaches);
        let ab = longings.iter().find(|x| x.subject == "ann" && x.object == "bob").unwrap();
        assert_eq!(ab.penia, 10.0);
        assert_eq!(ab.poros, 2.0);
        assert_eq!(ab.intensity, 8.0);
        assert!((ab.ascent - 0.2).abs() < 1e-6);
        // the sated pair is not eros (handed to polis)
        assert!(!longings.iter().any(|x| x.subject == "cy" || x.subject == "dee"));
        // bob → ann is over-returned, also rejected
        assert!(!longings.iter().any(|x| x.subject == "bob"));
        assert_eq!(ascending(&longings, 5.0, 0.1).len(), 1);
    }

    #[test]
    fn hopeless_longing_has_no_resource() {
        let reaches = vec![r("ann", "bob", 10.0)]; // bob never returns
        let longings = detect_longings(&reaches);
        let ab = &longings[0];
        assert_eq!(ab.poros, 0.0);
        assert!(ab.ascent.abs() < 1e-6);
        assert_eq!(hopeless(&longings, 0.05).len(), 1);
        assert!(ascending(&longings, 1.0, 0.1).is_empty());
    }
}
