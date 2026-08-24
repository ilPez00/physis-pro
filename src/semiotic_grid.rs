//! The stable public coordinate system for Physis classification.
//!
//! Ontology packs may add specialist entries, but the five domains and fourteen
//! modes below remain the common language used by APIs, reports, and users.

use physis_core::classify::EntrySeed;

pub const DOMAINS: [&str; 5] = ["HEAL", "CONSTRUCT", "FABRICATE", "BOND", "STUDY"];
pub const MODES: [&str; 14] = [
    "LIFT",
    "REST",
    "WALK",
    "WORK",
    "CREATE",
    "LEARN",
    "DESTROY",
    "SENSE",
    "GUIDE",
    "PLAY",
    "BRAINSTORM",
    "MAINTAIN",
    "MOVE",
    "PLAN",
];

pub fn is_canonical(domain: &str, mode: &str) -> bool {
    DOMAINS.contains(&domain) && MODES.contains(&mode)
}

/// Add a neutral anchor only for a missing canonical cell. The normal built-in
/// `mode_anchors` pack supplies richer phrases; these fallbacks keep the API's
/// coordinate system complete when a customer supplies a reduced ontology.
pub fn fallback_anchors(existing: &[(String, String)]) -> Vec<EntrySeed> {
    let mut out = Vec::new();
    for domain in DOMAINS {
        for mode in MODES {
            if existing.iter().any(|(d, m)| d == domain && m == mode) {
                continue;
            }
            let name = format!("{domain} {mode} anchor");
            let text = format!(
                "{domain} {mode}; {domain_lower} activity in {mode_lower} mode",
                domain_lower = domain.to_lowercase(),
                mode_lower = mode.to_lowercase(),
            );
            out.push(EntrySeed {
                name,
                domain: domain.to_string(),
                mode: mode.to_string(),
                text,
                facets: Default::default(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_grid_has_seventy_cells() {
        assert_eq!(DOMAINS.len() * MODES.len(), 70);
    }

    #[test]
    fn fallback_anchors_fill_only_missing_cells() {
        let existing = vec![("HEAL".into(), "LIFT".into())];
        let anchors = fallback_anchors(&existing);
        assert_eq!(anchors.len(), 69);
        assert!(anchors
            .iter()
            .all(|a| !(a.domain == "HEAL" && a.mode == "LIFT")));
    }
}
