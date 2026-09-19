//! Similarity (plan S8): two legs, and a claim that names which one matched.
//!
//! Semantic and lexical similarity are different facts. A pair that shares words
//! is lexically similar whether or not anything understood it, and with no
//! embedder (offline, random projection) only the lexical leg exists at all.
//! This module keeps the legs separate and refuses to let the weaker fact wear
//! the stronger word: **no similarity claim is made on the semantic leg unless
//! the semantic leg actually ran and matched**.

/// Which leg a similarity claim rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Neither leg matched.
    None,
    /// Only the lexical leg matched — words, not meaning.
    Lexical,
    /// Only the semantic leg matched.
    Semantic,
    /// Both legs matched.
    Both,
}

/// A similarity measurement. `semantic` is `None` when no embedder ran — which
/// is not the same as "semantic similarity was zero".
#[derive(Debug, Clone, PartialEq)]
pub struct Similarity {
    pub semantic: Option<f32>,
    pub lexical: f32,
}

impl Similarity {
    /// A lexical-only measurement: the offline, deterministic case.
    pub fn lexical_only(lexical: f32) -> Self {
        Self {
            semantic: None,
            lexical,
        }
    }

    pub fn both(semantic: f32, lexical: f32) -> Self {
        Self {
            semantic: Some(semantic),
            lexical,
        }
    }

    pub fn basis(&self) -> Basis {
        match (self.semantic, self.lexical > 0.0) {
            (Some(s), true) if s > 0.0 => Basis::Both,
            (Some(s), _) if s > 0.0 => Basis::Semantic,
            (_, true) => Basis::Lexical,
            _ => Basis::None,
        }
    }

    /// Whether the semantic leg ran at all. `false` means no embedder was
    /// available, so nothing here can speak about meaning.
    pub fn semantic_ran(&self) -> bool {
        self.semantic.is_some()
    }

    /// The best available score. Semantic wins when it ran and matched;
    /// otherwise the lexical leg is the honest answer.
    pub fn score(&self) -> f32 {
        match self.semantic {
            Some(s) if s > self.lexical => s,
            _ => self.lexical,
        }
    }

    /// The word for what was measured. Never says "semantic" on the lexical leg.
    pub fn claim(&self) -> &'static str {
        match self.basis() {
            Basis::Both => "semantic + lexical",
            Basis::Semantic => "semantic",
            Basis::Lexical => "lexical only",
            Basis::None => "no match",
        }
    }

    /// True only when a semantic leg actually ran. Callers gate any semantic
    /// wording on this rather than on the score being high.
    pub fn is_semantic(&self) -> bool {
        matches!(self.basis(), Basis::Semantic | Basis::Both)
    }

    pub fn render(&self) -> String {
        match self.semantic {
            Some(s) => format!(
                "{:.3} ({}) — semantic {s:.3}, lexical {:.3}",
                self.score(),
                self.claim(),
                self.lexical
            ),
            None => format!(
                "{:.3} ({}) — lexical {:.3}, no embedder ran",
                self.score(),
                self.claim(),
                self.lexical
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_only_never_claims_semantic() {
        let s = Similarity::lexical_only(0.8);
        assert_eq!(s.basis(), Basis::Lexical);
        assert_eq!(s.claim(), "lexical only");
        assert!(!s.is_semantic(), "a word match is not a meaning match");
        assert!(!s.semantic_ran());
        assert_eq!(s.score(), 0.8);
        assert!(s.render().contains("no embedder ran"));
    }

    #[test]
    fn a_high_lexical_score_does_not_become_a_semantic_claim() {
        // The trap this module exists for: a perfect word overlap is still only
        // a word overlap.
        let s = Similarity::lexical_only(1.0);
        assert!(!s.is_semantic());
        assert_eq!(s.claim(), "lexical only");
    }

    #[test]
    fn both_legs_matching_is_labelled_as_both() {
        let s = Similarity::both(0.91, 0.4);
        assert_eq!(s.basis(), Basis::Both);
        assert_eq!(s.claim(), "semantic + lexical");
        assert!(s.is_semantic());
        assert_eq!(s.score(), 0.91, "the semantic leg wins when it is higher");
    }

    #[test]
    fn a_zero_semantic_leg_does_not_claim_semantic() {
        let s = Similarity::both(0.0, 0.6);
        assert_eq!(s.basis(), Basis::Lexical);
        assert_eq!(s.claim(), "lexical only");
        assert!(s.semantic_ran(), "the leg ran; it just did not match");
        assert_eq!(s.score(), 0.6);
    }

    #[test]
    fn nothing_matching_says_so() {
        let s = Similarity::both(0.0, 0.0);
        assert_eq!(s.basis(), Basis::None);
        assert_eq!(s.claim(), "no match");
    }

    #[test]
    fn offline_is_deterministic_legs_only() {
        // With no embedder the only leg is lexical, which is deterministic:
        // same inputs, same number, no model involved.
        let a = Similarity::lexical_only(0.37);
        let b = Similarity::lexical_only(0.37);
        assert_eq!(a, b);
        assert!(!a.semantic_ran());
    }
}
