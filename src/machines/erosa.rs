//! erosa — the relational machine, **mythic** genealogy: Eros the child of
//! **Ares** (war, strife) and **Aphrodite/Venus** (love). The inverse of `erosp`.
//!
//! Where the Platonic eros is lack reaching to ascend, this eros is the union of
//! Aphrodite (the binding draw) and Ares (rivalry and seizure): possessive,
//! agonistic desire that conquers and *holds* rather than aspires. It is *eros*
//! only as the child of BOTH parents — pure love without strife is the gentle
//! Platonic longing (→ `erosp`); pure strife without love is mere aggression, not
//! eros at all. Its pathology, and its dream, is strife overrunning love:
//! jealousy, the seizure that destroys the bond it guards. (Aphrodite = Philotes,
//! Ares = Neikos — the Empedoclean dyad worn as desire; see
//! `docs/EMPEDOCLES_DYAD.md`.)
//!
//! Twin of `erosp`: same perception (`Reach`), inverse reduction. Pure + testable.

use std::collections::HashMap;

use super::erosp::Reach;
use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// A mythic erotic vector: conquest born of love and strife.
#[derive(Debug, Clone, PartialEq)]
pub struct Conquest {
    pub subject: String,
    pub object: String,
    /// Aphrodite — the love bond (the mutual draw between subject and object).
    pub aphrodite: f32,
    /// Ares — strife: rival pressure over the object plus the subject's seizing
    /// excess (reaching more than it returns).
    pub ares: f32,
    /// How many other subjects also reach for this object (the rivals).
    pub rivals: usize,
    /// 0–1: balance of the two parents. 1 = love and strife in equal measure
    /// (passionate bond); → 0 = one parent overruns the other (toward jealousy).
    pub balance: f32,
    pub hash: String,
}

fn directed_flow(reaches: &[Reach]) -> HashMap<(String, String), f32> {
    let mut flow: HashMap<(String, String), f32> = HashMap::new();
    for r in reaches {
        *flow
            .entry((r.subject.clone(), r.object.clone()))
            .or_insert(0.0) += r.weight;
    }
    flow
}

/// Reduce directed reaches to conquests. A `subject → object` is *erosa* only
/// when both parents are present: love (a mutual draw) AND strife (rivalry over
/// the object, or the subject seizing more than it gives). Loveless strife and
/// strifeless love are both rejected — the first is not eros, the second is
/// `erosp`.
pub fn detect_conquests(reaches: &[Reach]) -> Vec<Conquest> {
    let flow = directed_flow(reaches);
    let mut out = Vec::new();
    for ((subject, object), reach) in flow.iter().map(|(k, v)| (k.clone(), *v)) {
        if reach <= 0.0 {
            continue;
        }
        let back = *flow.get(&(object.clone(), subject.clone())).unwrap_or(&0.0);
        // Aphrodite: love present in both directions is the bond's measure.
        let aphrodite = reach.min(back);
        // Ares: rivalry from others + the subject's own seizing excess.
        let mut rival_pressure = 0.0;
        let mut rivals = 0usize;
        for ((from, to), w) in flow.iter() {
            if to == &object && from != &subject && from != &object && *w > 0.0 {
                rival_pressure += *w;
                rivals += 1;
            }
        }
        let seizure = (reach - back).max(0.0);
        let ares = rival_pressure + seizure;
        // Child of BOTH parents, or it is not erosa.
        if aphrodite <= 0.0 || ares <= 0.0 {
            continue;
        }
        let balance = aphrodite.min(ares) / aphrodite.max(ares);
        let hash = format!("erosa_{subject}_{object}");
        out.push(Conquest {
            subject,
            object,
            aphrodite,
            ares,
            rivals,
            balance,
            hash,
        });
    }
    out.sort_by(|a, b| {
        (b.aphrodite + b.ares)
            .partial_cmp(&(a.aphrodite + a.ares))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Passionate conquests: love and strife in balance — the coherent erosa, the
/// bond made vivid by contest rather than consumed by it.
pub fn passionate(conquests: &[Conquest], min_balance: f32) -> Vec<&Conquest> {
    conquests
        .iter()
        .filter(|c| c.balance >= min_balance)
        .collect()
}

/// Jealous conquests: strife overruns love (Ares > Aphrodite) — the possessive,
/// destructive pathology. The dream surfaces these to be cooled.
pub fn jealous(conquests: &[Conquest]) -> Vec<&Conquest> {
    conquests.iter().filter(|c| c.ares > c.aphrodite).collect()
}

/// Adapter: register each conquest as a directed, bridged relation, weighted by
/// the love bond (Aphrodite) that holds the strife together.
pub fn register_conquests(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    conquests: &[Conquest],
) -> Vec<String> {
    let mut ids = Vec::new();
    for c in conquests {
        let subject = core.register_node_from_text(&format!("person: {}", c.subject), embedder);
        let conquest = core.register_node_from_text(
            &format!(
                "conquest: {} -> {} ({} rivals)",
                c.subject, c.object, c.rivals
            ),
            embedder,
        );
        core.bridge_node(&conquest);
        core.add_edge_between(&subject, &conquest, c.aphrodite, 1);
        ids.push(conquest);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(subject: &str, object: &str, weight: f32) -> Reach {
        Reach {
            subject: subject.into(),
            object: object.into(),
            weight,
        }
    }

    #[test]
    fn conquest_is_child_of_love_and_strife() {
        let reaches = vec![
            r("ann", "bob", 6.0),
            r("bob", "ann", 6.0), // ann & bob: love both ways
            r("cy", "bob", 4.0),  // cy is a rival for bob
        ];
        let conquests = detect_conquests(&reaches);
        let ab = conquests
            .iter()
            .find(|x| x.subject == "ann" && x.object == "bob")
            .unwrap();
        assert_eq!(ab.aphrodite, 6.0); // min(6,6)
        assert_eq!(ab.rivals, 1); // cy
        assert!(ab.ares >= 4.0); // rival pressure from cy
                                 // cy → bob is loveless (bob does not reach cy) → not eros at all
        assert!(!conquests.iter().any(|x| x.subject == "cy"));
    }

    #[test]
    fn strifeless_love_is_not_erosa() {
        // mutual love, no rival, no seizure → Ares = 0 → belongs to erosp, not here
        let reaches = vec![r("ann", "bob", 5.0), r("bob", "ann", 5.0)];
        assert!(detect_conquests(&reaches).is_empty());
    }

    #[test]
    fn jealous_when_strife_dominates_love() {
        let reaches = vec![
            r("ann", "bob", 1.0),
            r("bob", "ann", 1.0), // a thin love
            r("cy", "bob", 5.0),
            r("dee", "bob", 5.0), // crowded with rivals
        ];
        let conquests = detect_conquests(&reaches);
        let ab = conquests.iter().find(|x| x.subject == "ann").unwrap();
        assert_eq!(ab.aphrodite, 1.0);
        assert_eq!(ab.rivals, 2);
        assert!(ab.ares > ab.aphrodite);
        assert_eq!(jealous(&conquests).len(), 1);
        assert!(passionate(&conquests, 0.5).is_empty());
    }
}
