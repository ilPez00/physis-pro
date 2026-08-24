//! lex — the legal / contract machine.
//!
//! Perceive a document as clauses; reduce to **obligation / right / condition**
//! primitives; compose a dependency graph (an obligation `requires` conditions);
//! elaborate by simulating breach paths — if a condition fails, which obligations
//! can no longer be satisfied? Pairs with the `ingest` feature (doc → clauses).
//!
//! Pure + testable.

use std::collections::HashSet;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Obligation,
    Right,
    Condition,
}

#[derive(Debug, Clone)]
pub struct Clause {
    pub id: String,
    pub kind: Kind,
    pub text: String,
    /// Condition clause ids this obligation/right depends on.
    pub requires: Vec<String>,
}

/// Obligations with no preconditions — always due.
pub fn unconditional_obligations(clauses: &[Clause]) -> Vec<String> {
    clauses
        .iter()
        .filter(|c| c.kind == Kind::Obligation && c.requires.is_empty())
        .map(|c| c.id.clone())
        .collect()
}

/// Given a set of failed condition ids, the obligations that can no longer be
/// satisfied (any required condition failed) — the breach paths.
pub fn breach_paths(clauses: &[Clause], failed: &[String]) -> Vec<String> {
    let failed: HashSet<&str> = failed.iter().map(String::as_str).collect();
    clauses
        .iter()
        .filter(|c| c.kind == Kind::Obligation)
        .filter(|c| c.requires.iter().any(|r| failed.contains(r.as_str())))
        .map(|c| c.id.clone())
        .collect()
}

/// Conditions referenced by an obligation/right but not defined as a clause —
/// the unhandled contingencies (the gap a dream would surface).
pub fn dangling_conditions(clauses: &[Clause]) -> Vec<String> {
    let defined: HashSet<&str> = clauses
        .iter()
        .filter(|c| c.kind == Kind::Condition)
        .map(|c| c.id.as_str())
        .collect();
    let mut missing: Vec<String> = clauses
        .iter()
        .flat_map(|c| c.requires.iter())
        .filter(|r| !defined.contains(r.as_str()))
        .cloned()
        .collect();
    missing.sort();
    missing.dedup();
    missing
}

/// Adapter: register clauses as nodes + `requires` edges into the shared graph.
pub fn register_clauses(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    clauses: &[Clause],
) -> Vec<String> {
    let mut ids = std::collections::HashMap::new();
    for c in clauses {
        let node = core.register_node_from_text(&format!("{:?}: {}", c.kind, c.text), embedder);
        core.bridge_node(&node);
        ids.insert(c.id.clone(), node);
    }
    for c in clauses {
        if let Some(from) = ids.get(&c.id) {
            for r in &c.requires {
                if let Some(to) = ids.get(r) {
                    core.add_edge_between(from, to, 1.0, c.kind as u32);
                }
            }
        }
    }
    ids.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, kind: Kind, requires: &[&str]) -> Clause {
        Clause {
            id: id.into(),
            kind,
            text: id.into(),
            requires: requires.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn breach_and_dangling() {
        let clauses = vec![
            c("pay", Kind::Obligation, &["delivered"]),
            c("deliver", Kind::Obligation, &[]),
            c("delivered", Kind::Condition, &[]),
            c("warranty", Kind::Right, &["registered"]), // `registered` never defined
        ];
        assert_eq!(
            unconditional_obligations(&clauses),
            vec!["deliver".to_string()]
        );
        // if `delivered` fails, `pay` can't be satisfied
        assert_eq!(
            breach_paths(&clauses, &["delivered".into()]),
            vec!["pay".to_string()]
        );
        // `registered` is required but undefined → dangling
        assert_eq!(
            dangling_conditions(&clauses),
            vec!["registered".to_string()]
        );
    }
}
