//! elenchus — the dialogue / dialectic machine. (Socratic cross-examination —
//! NOT "agora", which is the social network.)
//!
//! Perceive a conversation as statements; reduce to Toulmin primitives — **claim
//! / warrant / rebuttal**; compose an argument graph; elaborate by finding the
//! weak points: claims nobody supported, and claims a rebuttal contests
//! (physis's ConstructiveRefutation in miniature). A claim is coherent only if
//! warranted and unrefuted.
//!
//! Pure + testable.
//!
//! Standalone twin: `physis-logos` (Python). It works one layer *below* dialogue:
//! it reduces raw text to **syntactic / rhetorical** primitives (SVO triples,
//! clause types, rhetorical figures) and elaborates by sentence generation. A
//! future `logos` reducer (or an elenchus pre-pass) could feed those forms in as
//! the claims/warrants elenchus argues over.

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Claim,
    /// Supports the claim it `targets`.
    Warrant,
    /// Refutes the claim it `targets`.
    Rebuttal,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub id: String,
    pub kind: Kind,
    pub text: String,
    /// For Warrant/Rebuttal: the claim id it acts on.
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Warranted and unrefuted.
    Supported,
    /// Has a rebuttal (contested → ConstructiveRefutation).
    Contested,
    /// No warrant, no rebuttal — asserted but unearned.
    Unsupported,
}

impl Standing {
    pub fn coherence(self) -> f32 {
        match self {
            Standing::Supported => 1.0,
            Standing::Unsupported => 0.0,
            Standing::Contested => -1.0,
        }
    }
}

/// Standing for each claim, derived from the warrants/rebuttals pointing at it.
/// Contested (any rebuttal) dominates supported.
pub fn evaluate(statements: &[Statement]) -> Vec<(String, Standing)> {
    let mut out = Vec::new();
    for s in statements.iter().filter(|s| s.kind == Kind::Claim) {
        let mut warranted = false;
        let mut rebutted = false;
        for o in statements {
            if o.target.as_deref() == Some(s.id.as_str()) {
                match o.kind {
                    Kind::Warrant => warranted = true,
                    Kind::Rebuttal => rebutted = true,
                    Kind::Claim => {}
                }
            }
        }
        let standing = if rebutted {
            Standing::Contested
        } else if warranted {
            Standing::Supported
        } else {
            Standing::Unsupported
        };
        out.push((s.id.clone(), standing));
    }
    out
}

/// Claims asserted with no warrant and no rebuttal — the questions to press.
pub fn unsupported_claims(statements: &[Statement]) -> Vec<String> {
    evaluate(statements)
        .into_iter()
        .filter(|(_, st)| *st == Standing::Unsupported)
        .map(|(id, _)| id)
        .collect()
}

/// Claims a rebuttal contests — where the argument must be recalibrated.
pub fn contested_claims(statements: &[Statement]) -> Vec<String> {
    evaluate(statements)
        .into_iter()
        .filter(|(_, st)| *st == Standing::Contested)
        .map(|(id, _)| id)
        .collect()
}

/// Adapter: register each statement as a node + edge (warrant/rebuttal → claim).
pub fn register_argument(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    statements: &[Statement],
) -> Vec<String> {
    let mut ids = std::collections::HashMap::new();
    for s in statements {
        let text = format!("{:?}: {}", s.kind, s.text);
        let node = core.register_node_from_text(&text, embedder);
        core.bridge_node(&node);
        ids.insert(s.id.clone(), node);
    }
    for s in statements {
        if let (Some(tgt), Some(from)) = (s.target.as_ref(), ids.get(&s.id)) {
            if let Some(to) = ids.get(tgt) {
                let w = if s.kind == Kind::Rebuttal { -1.0 } else { 1.0 };
                core.add_edge_between(from, to, w, s.kind as u32);
            }
        }
    }
    ids.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: &str, kind: Kind, target: Option<&str>) -> Statement {
        Statement { id: id.into(), kind, text: id.into(), target: target.map(|s| s.into()) }
    }

    #[test]
    fn standing_reflects_warrants_and_rebuttals() {
        let s = vec![
            st("c1", Kind::Claim, None),
            st("w1", Kind::Warrant, Some("c1")), // c1 supported
            st("c2", Kind::Claim, None),
            st("w2", Kind::Warrant, Some("c2")),
            st("r2", Kind::Rebuttal, Some("c2")), // c2 contested (rebuttal wins)
            st("c3", Kind::Claim, None),          // c3 unsupported
        ];
        let eval: std::collections::HashMap<_, _> = evaluate(&s).into_iter().collect();
        assert_eq!(eval["c1"], Standing::Supported);
        assert_eq!(eval["c2"], Standing::Contested);
        assert_eq!(eval["c3"], Standing::Unsupported);
        assert_eq!(unsupported_claims(&s), vec!["c3".to_string()]);
        assert_eq!(contested_claims(&s), vec!["c2".to_string()]);
        assert_eq!(Standing::Contested.coherence(), -1.0);
    }
}
