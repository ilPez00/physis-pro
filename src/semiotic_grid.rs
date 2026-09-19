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
//! ## The mode axis, reworked (2026-09-19)
//!
//! D2a measured every mode against WordNet's verb supersenses. Acting on that
//! measurement, the mode axis is now nine modes, each named for the supersense
//! it owns:
//!
//! | action | change | evidence |
//! |---|---|---|
//! | RETIRE | LIFT, SENSE, CREATE, WALK, BRAINSTORM, DESTROY | `carves = false` against their own null |
//! | MERGE | LEARN+PLAN → COGNITION · GUIDE+WORK → COMMUNICATION · MOVE+PLAY → MOTION | each pair shares one dominant supersense |
//! | EXPAND | SOCIAL (12.2% of verb mass) · CREATE (7.9%) · POSSESS (5.6%) · STATIVE (5.3%) | unowned supersenses with real corpus mass |
//!
//! `CREATE` appears in both columns: the old mode carved nothing (its dominant
//! supersense was `verb.contact`, not creation), so it is retired as a word and
//! re-anchored to `verb.creation`, which no mode owned. `MODE_OWNERS` carries
//! the measurement per mode, so the axis states its own justification.
//!
//! ## The unit is the domain plus its facets
//!
//! The joint cell recovers 0.416 against the domain's 0.585, so the cell is the
//! weaker coordinate. A mode that is unknown or misclassified must not be able
//! to weaken a coordinate the domain already fixes: **domain + facets is the
//! unit, and mode is an optional refinement** (facets nest under a cell without
//! enlarging the grid — `physis-core/src/models.rs`, `struct Facets`).
//!
//! Domains remain an open set, defaulting to the five below and replaceable per
//! deployment via `PHYSIS_DOMAINS`. Track D's rule still holds: the machine
//! proposes, a person adjudicates. This rework was adjudicated; nothing edits
//! the grid silently.

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

/// How many cells the active axes describe. **Not necessarily 45.**
pub fn cell_count() -> usize {
    domains().len() * MODES.len()
}

/// The operative axis: nine modes, each the owner of one verb supersense.
pub const MODES: [&str; 9] = [
    "COMMUNICATION",
    "COGNITION",
    "MOTION",
    "REST",
    "MAINTAIN",
    "SOCIAL",
    "CREATE",
    "POSSESS",
    "STATIVE",
];

/// What a mode owns and the measurement that put it there.
///
/// A mode that owns nothing is a word the corpus has no use for: that is D2a's
/// finding and this axis's reason to exist. `evidence` is prose because the two
/// kinds of entry — a surviving mode's own carve, and a newly-owned supersense's
/// corpus mass — are different measurements and forcing them into one numeric
/// schema would hide which is which.
pub struct ModeOwner {
    pub mode: &'static str,
    pub supersense: &'static str,
    pub evidence: &'static str,
}

pub const MODE_OWNERS: [ModeOwner; 9] = [
    ModeOwner {
        mode: "COMMUNICATION",
        supersense: "verb.communication",
        evidence: "GUIDE carved at share 0.361 vs null 0.182 (+0.179); WORK merged in (share 0.190, +0.045)",
    },
    ModeOwner {
        mode: "COGNITION",
        supersense: "verb.cognition",
        evidence: "LEARN carved at share 0.272 vs null 0.154 (+0.117); PLAN merged in (share 0.239, +0.079)",
    },
    ModeOwner {
        mode: "MOTION",
        supersense: "verb.motion",
        evidence: "MOVE carved at share 0.250 vs null 0.163 (+0.087); PLAY merged in (share 0.255, +0.071)",
    },
    ModeOwner {
        mode: "REST",
        supersense: "verb.contact",
        evidence: "carved at share 0.217 vs null 0.164 (+0.053)",
    },
    ModeOwner {
        mode: "MAINTAIN",
        supersense: "verb.change",
        evidence: "carved at share 0.181 vs null 0.151 (+0.029)",
    },
    ModeOwner {
        mode: "SOCIAL",
        supersense: "verb.social",
        evidence: "unowned before this rework; 12.2% of the corpus's verb mass (D2a)",
    },
    ModeOwner {
        mode: "CREATE",
        supersense: "verb.creation",
        evidence: "unowned before this rework at 7.9% of verb mass; the retired CREATE owned verb.contact, not this",
    },
    ModeOwner {
        mode: "POSSESS",
        supersense: "verb.possession",
        evidence: "unowned before this rework; 5.6% of verb mass",
    },
    ModeOwner {
        mode: "STATIVE",
        supersense: "verb.stative",
        evidence: "unowned before this rework; 5.3% of verb mass",
    },
];

/// The supersense a mode owns, if it is one of the reworked modes.
pub fn owned_supersense(mode: &str) -> Option<&'static str> {
    MODE_OWNERS
        .iter()
        .find(|m| m.mode == mode)
        .map(|m| m.supersense)
}

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
        assert_eq!(axis.len() * MODES.len(), 18);
    }

    #[test]
    fn every_mode_owns_a_supersense_and_the_retired_ones_are_gone() {
        // The rework's whole point: no mode is a word without an owner.
        assert_eq!(MODE_OWNERS.len(), MODES.len());
        for mode in MODES {
            assert!(owned_supersense(mode).is_some(), "{mode} owns nothing");
        }
        for retired in [
            "LIFT",
            "SENSE",
            "WALK",
            "BRAINSTORM",
            "DESTROY",
            "PLAN",
            "LEARN",
            "GUIDE",
            "WORK",
            "MOVE",
            "PLAY",
        ] {
            assert!(
                !MODES.contains(&retired),
                "{retired} should be retired or merged away"
            );
        }
        // The four heavy unowned supersenses now have owners.
        for s in [
            "verb.social",
            "verb.creation",
            "verb.possession",
            "verb.stative",
        ] {
            assert!(
                MODE_OWNERS.iter().any(|m| m.supersense == s),
                "{s} is still unowned"
            );
        }
    }

    #[test]
    fn an_empty_or_absent_override_falls_back_to_the_default_pack() {
        assert_eq!(domains_from(None).len(), 5);
        assert_eq!(domains_from(Some("   ")).len(), 5);
        assert_eq!(domains_from(Some(",,,")).len(), 5);
    }

    #[test]
    fn canonical_grid_is_five_by_nine_after_the_rework() {
        // The default pack is 5 x 9 since the mode axis was reworked. The point
        // of the assertion is that the DEFAULT is stable, not that 45 is a
        // property of the world.
        assert_eq!(DEFAULT_DOMAINS.len() * MODES.len(), 45);
        assert_eq!(
            cell_count(),
            45,
            "no PHYSIS_DOMAINS set, so the default holds"
        );
    }

    #[test]
    fn fallback_anchors_fill_only_missing_cells() {
        let existing = vec![("HEAL".into(), "REST".into())];
        let anchors = fallback_anchors(&existing);
        assert_eq!(anchors.len(), 44);
        assert!(anchors
            .iter()
            .all(|a| !(a.domain == "HEAL" && a.mode == "REST")));
    }
}
