//! Phase 9.2 — anti-hallucination grounding gate (clean-room pattern
//! adaptation from docs/reference/repowise-adaptation.md P3; no source code
//! reused).
//!
//! The gate is the trust primitive for every LLM-produced field that claims
//! provenance: a `decision` / `rationale` / `source_quote` that does not
//! token-match the verbatim `source_text` it cites is dropped, and a candidate
//! whose every gated field fails is rejected outright. Fluent-but-invented
//! content must never become institutional memory.
//!
//! Operates on plain [`Candidate`] data so any producer (decision mining,
//! dream outputs, wiki generation) can call it without module cycles.

/// One produced claim awaiting grounding verification.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The action/choice being claimed.
    pub decision: String,
    /// Why — must be grounded in the source.
    pub rationale: String,
    /// Verbatim span allegedly from `source_text`.
    pub source_quote: String,
    /// The verbatim source the candidate claims to come from.
    pub source_text: String,
    /// Stamped by [`apply_gate`].
    pub verification: Verdict,
}

/// Strongest surviving verdict across the gated fields, in priority order:
/// all fields grounded → [`Verdict::Grounded`]; some → [`Verdict::Partial`];
/// none → the candidate is rejected and never returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Grounded,
    Partial,
    Unverified,
}

impl Verdict {
    /// Stable tag for logs/provenance records.
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Grounded => "grounded",
            Verdict::Partial => "partial",
            Verdict::Unverified => "unverified",
        }
    }
}

// Gated fields (all enforced equally): decision, rationale, source_quote.
/// Normalize into comparable tokens: lowercase, split on non-alphanumeric.
fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// True when every token of `needle` appears in `haystack_tokens`
/// (order-free containment — punctuation and case are already folded).
fn token_match(needle: &str, haystack_tokens: &[String]) -> bool {
    let n = tokens(needle);
    if n.is_empty() || haystack_tokens.is_empty() {
        return false;
    }
    n.iter().all(|t| haystack_tokens.contains(t))
}

/// Drop ungrounded fields, reject candidates with no surviving evidence.
///
/// Returns `(kept_candidates_with_verification_stamped, rejected_count)`.
/// An empty `source_text` rejects unconditionally — nothing can be grounded
/// against nothing.
pub fn apply_gate(candidates: Vec<Candidate>) -> (Vec<Candidate>, usize) {
    let mut kept = Vec::with_capacity(candidates.len());
    let mut rejected = 0;
    for mut c in candidates {
        if c.source_text.trim().is_empty() {
            rejected += 1;
            continue;
        }
        let hay = tokens(&c.source_text);
        // All three gated fields enforced equally (GATED_FIELDS documents the set).
        let grounded_decision = token_match(&c.decision.clone(), &hay);
        let grounded_rationale = token_match(&c.rationale.clone(), &hay);
        let grounded_quote = token_match(&c.source_quote.clone(), &hay);
        let grounded_n = usize::from(grounded_decision)
            + usize::from(grounded_rationale)
            + usize::from(grounded_quote);
        c.verification = match grounded_n {
            3 => Verdict::Grounded,
            _ if grounded_n > 0 => Verdict::Partial,
            _ => {
                rejected += 1;
                continue;
            }
        };
        // Drop ungrounded FIELDS outright: invented prose must never be stored.
        if !grounded_decision {
            c.decision.clear();
        }
        if !grounded_rationale {
            c.rationale.clear();
        }
        if !grounded_quote {
            c.source_quote.clear();
        }
        kept.push(c);
    }
    kept.sort_by_key(|c| match c.verification {
        Verdict::Grounded => 0,
        Verdict::Partial => 1,
        Verdict::Unverified => 2,
    });
    (kept, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_match_folds_case_and_punctuation() {
        assert!(token_match(
            "ACID, transactions!",
            &tokens("sled gives acid transactions.")
        ));
        assert!(!token_match(
            "quantum ledger",
            &tokens("sled gives acid transactions.")
        ));
        assert!(!token_match("", &tokens("anything")));
    }

    #[test]
    fn empty_candidate_triple_is_rejected() {
        let c = Candidate {
            decision: String::new(),
            rationale: String::new(),
            source_quote: String::new(),
            source_text: "some source".into(),
            verification: Verdict::Unverified,
        };
        let (kept, rejected) = apply_gate(vec![c]);
        assert_eq!((kept.len(), rejected), (0, 1));
    }
}
