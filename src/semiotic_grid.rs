//! The coordinate system for Physis classification.
//!
//! ## The domain axis is open. The count is a default, not a law.
//!
//! This file used to say "the five domains and fourteen modes below remain the
//! common language". The five were never measured as five. E66 put both axes in
//! front of the same size-preserving permutation null that D4 used, on the same
//! 731 ontology entries and the same embeddings:
//!
//! | axis | classes | self-recovery | null | normalised (f−n)/(1−n) |
//! |---|---|---|---|---|
//! | DOMAIN/MODE | 70 | 0.438 | 0.038 | 0.416 |
//! | DOMAIN only | 5 | 0.672 | 0.209 | **0.585** |
//! | MODE only | 14 | 0.536 | 0.121 | 0.472 |
//!
//! **Both axes carve; every class on both clears its own null.** So neither is
//! arbitrary — and nothing in that table says *five* or *fourteen*. The counts
//! are a pack, and the external anchors say as much in the other direction:
//! against WordNet's noun supersenses 35 of 70 cells are `disputed` (D1), and
//! against its fifteen verb supersenses 6 of 14 modes fail to carve while
//! `verb.social` — 12.2% of the corpus's verb mass — is owned by no mode at all
//! (D2a).
//!
//! Therefore: **domains are an open set**, defaulting to the five below and
//! replaceable per deployment via `PHYSIS_DOMAINS`. Modes stay fixed for now
//! because they are the operative axis — tool use, method, workflow — and
//! because D2a's proposals (EXPAND `verb.social`, MERGE `LEARN`+`PLAN`, RETIRE
//! six) are proposals with a measurement, not an approved change. Track D's rule
//! holds: the machine proposes, a person adjudicates, nothing edits the grid
//! silently.

use physis_core::classify::EntrySeed;

/// The default domain pack. Content-side axis: *what the work is about*.
pub const DEFAULT_DOMAINS: [&str; 5] = ["HEAL", "CONSTRUCT", "FABRICATE", "BOND", "STUDY"];

/// Kept as an alias so existing callers and reports do not break.
pub const DOMAINS: [&str; 5] = DEFAULT_DOMAINS;

/// The domain axis in force, defaulting to [`DEFAULT_DOMAINS`].
///
/// `PHYSIS_DOMAINS` overrides it with a comma-separated list, uppercased. This
/// is the whole of "the domain axis is adaptable": a deployment whose content is
/// not healthcare-construction-fabrication-bonding-study says so, and every cell
/// derived from the axis follows. Modes are deliberately not overridable here —
/// see the module header.
pub fn domains() -> Vec<String> {
    domains_from(std::env::var("PHYSIS_DOMAINS").ok().as_deref())
}

/// The pure half, so the axis can be tested without touching process-wide
/// state. The first version of this test set `PHYSIS_DOMAINS` and raced the
/// other two tests in the same file — cargo runs them in parallel threads and
/// env is shared, so one test's override became another's reality.
pub fn domains_from(spec: Option<&str>) -> Vec<String> {
    let fallback = || DEFAULT_DOMAINS.iter().map(|d| d.to_string()).collect();
    match spec {
        Some(v) if !v.trim().is_empty() => {
            let list: Vec<String> = v
                .split(',')
                .map(|d| d.trim().to_uppercase())
                .filter(|d| !d.is_empty())
                .collect();
            if list.is_empty() {
                fallback()
            } else {
                list
            }
        }
        _ => fallback(),
    }
}

/// How many cells the active axes describe. **Not necessarily 70.**
pub fn cell_count() -> usize {
    domains().len() * MODES.len()
}
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
    domains().iter().any(|d| d == domain) && MODES.contains(&mode)
}

/// Add a neutral anchor only for a missing canonical cell. The normal built-in
/// `mode_anchors` pack supplies richer phrases; these fallbacks keep the API's
/// coordinate system complete when a customer supplies a reduced ontology.
pub fn fallback_anchors(existing: &[(String, String)]) -> Vec<EntrySeed> {
    let mut out = Vec::new();
    for domain in domains() {
        let domain = domain.as_str();
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
    fn an_overridden_domain_axis_is_n_wide_not_five() {
        let axis = domains_from(Some("MAKE, TRADE"));
        assert_eq!(axis, vec!["MAKE".to_string(), "TRADE".to_string()]);
        assert_eq!(axis.len() * MODES.len(), 28);
    }

    #[test]
    fn an_empty_or_absent_override_falls_back_to_the_default_pack() {
        assert_eq!(domains_from(None).len(), 5);
        assert_eq!(domains_from(Some("   ")).len(), 5);
        assert_eq!(domains_from(Some(",,,")).len(), 5);
    }

    #[test]
    fn canonical_grid_has_seventy_cells() {
        // The default pack is 5 x 14. The point of the assertion is that the
        // DEFAULT is stable, not that 70 is a property of the world.
        assert_eq!(DEFAULT_DOMAINS.len() * MODES.len(), 70);
        assert_eq!(cell_count(), 70, "no PHYSIS_DOMAINS set, so the default holds");
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
