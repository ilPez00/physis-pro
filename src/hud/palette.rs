//! The command palette as a projection of actions, not a menu.
//!
//! Selecting a file, service, agent, host or printer exposes only the actions
//! actually registered for it. No second manually-written action list: every
//! row below derives from the guided WorldView, which itself derives from the
//! live registry. The object's kind selects the affordances, each affordance
//! contributes its verb, and the risk tier rides along so the renderer gates
//! destructive rows behind confirmation. Reads the world; never calls it.

use crate::guided::WorldView;

/// One palette row: a verb the focused object offers, with the capability
/// that authorises it and the risk that gates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteRow {
    pub verb: String,
    pub capability: String,
    pub risk: String,
    pub target: String,
}

impl PaletteRow {
    /// High-risk rows ask first; the palette never hides a registered action.
    pub fn needs_confirm(&self) -> bool {
        self.risk == "high"
    }
}

/// Derive the palette for one object from the world view. Empty for an
/// unknown target: no actions, never a guessed menu. Deterministic in
/// WorldView order (verb, then capability).
pub fn palette_for(view: &WorldView, handle: &str) -> Vec<PaletteRow> {
    // Resolve through the world's own resolver: exact handle first, then
    // path, then unique substring - the same address the operator typed.
    let obj = match view.resolve(handle) {
        Some(o) if o.handle.as_str() == handle => o,
        _ => match view.objects.iter().find(|o| o.handle.as_str() == handle) {
            Some(o) => o.clone(),
            None => return Vec::new(),
        },
    };
    view.affordances
        .iter()
        .filter(|a| a.object_kinds.iter().any(|k| k == &obj.kind))
        .map(|a| PaletteRow {
            verb: a.verb.clone(),
            capability: a.capability.clone(),
            risk: a.risk.clone(),
            target: obj.handle.as_str().to_string(),
        })
        .collect()
}

/// The affordances for one projection row, by its canonical identity.
///
/// A row carries the full `origin:key` spelling, while the registry addresses
/// objects by the **short** form (that is the two-forms split documented in
/// `world_object::spell_object_id`). This is the one place that crosses between
/// them, so "what may I do to *this* object?" is answered from the registry
/// rather than remapped by each renderer.
///
/// Empty means exactly that: no registered capability for this object. It never
/// falls back to "all commands", because that would answer a different question.
pub fn affordances_for_identity(view: &WorldView, identity: &str) -> Vec<PaletteRow> {
    // Verbatim first: whatever the caller named is what the registry may hold
    // (`docs`, `host:local`). Then the canonical short spelling, which is how
    // `register_nav_index` keys `fs:` objects. Neither leg guesses.
    let mut rows = palette_for(view, identity);
    if rows.is_empty() {
        let short = crate::world_object::parse_object_id(identity).short();
        if short != identity {
            rows = palette_for(view, &short);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;

    fn world() -> WorldView {
        let mut reg = Registry::new("/tmp/physis-palette-test-obs.jsonl");
        crate::registry::seed(&mut reg, "/tmp");
        WorldView::from_registry(&reg)
    }

    #[test]
    fn palette_lists_only_registered_verbs_for_the_kind() {
        // THE seam test: the palette is derived, never written. Whatever the
        // seeded registry offers a `repo` object is exactly what renders.
        let view = world();
        let repo = view.objects_of_kind("repo");
        assert!(!repo.is_empty(), "seeded registry has a repo object");
        let handle = repo[0].handle.as_str().to_string();
        let rows = palette_for(&view, &handle);
        assert!(!rows.is_empty(), "a repo offers actions");
        for row in &rows {
            let aff = view
                .affordances
                .iter()
                .find(|a| a.verb == row.verb && a.capability == row.capability)
                .expect("row traces to a real affordance");
            assert!(aff.object_kinds.iter().any(|k| k == &repo[0].kind));
        }
    }

    #[test]
    fn unknown_target_offers_nothing_not_a_guess() {
        let view = world();
        assert!(palette_for(&view, "nope:missing").is_empty());
    }

    #[test]
    fn high_risk_rows_ask_first() {
        let view = world();
        let handles: Vec<String> = view
            .objects
            .iter()
            .map(|o| o.handle.as_str().to_string())
            .collect();
        let rows: Vec<PaletteRow> = handles
            .iter()
            .flat_map(|h| palette_for(&view, h))
            .collect();
        assert!(!rows.is_empty());
        for row in &rows {
            assert_eq!(row.needs_confirm(), row.risk == "high");
        }
    }

    #[test]
    fn a_projection_row_identity_reaches_its_registered_affordances() {
        // §14's seam. Two real callers, two spellings:
        //   * a bare handle (`docs`) — what `registry::seed` registers;
        //   * a full `origin:key` — what a projection row carries, while
        //     `register_nav_index` keys the object by the *short* form. That
        //     crossing is what this function exists for, so it is built here
        //     rather than assumed.
        use crate::registry::Registry;
        use std::collections::HashMap;

        let view = world();
        let repo = view.objects_of_kind("repo");
        assert!(!repo.is_empty(), "precondition: the seeded world has a repo");
        let handle = repo[0].handle.as_str().to_string();
        assert!(
            !affordances_for_identity(&view, &handle).is_empty(),
            "the verbatim handle reaches its capabilities"
        );

        // A nav-style object: the registry holds the short spelling.
        const LONG: &str = "0e4bfb603e53a1b2c3d4e5f60718293a4b5c6d7e";
        let short_handle = format!("fs:{}", &LONG[..12]);
        let mut reg = Registry::new("/tmp/physis-palette-identity-obs.jsonl");
        reg.register_object(short_handle.clone(), "file".into(), "a.rs".into(), HashMap::new());
        for cap in crate::capability::default_capabilities() {
            reg.add_capability(cap);
        }
        let nav_view = WorldView::from_registry(&reg);
        let full_spelling = format!("fs:{LONG}");
        let rows = affordances_for_identity(&nav_view, &full_spelling);
        assert_eq!(
            rows.len(),
            palette_for(&nav_view, &short_handle).len(),
            "the full spelling reaches what the short handle reaches"
        );
        assert!(!rows.is_empty(), "and it is not empty");

        // An object nobody registered gets nothing — never a fallback menu.
        assert!(affordances_for_identity(&view, "fs:not-a-registered-handle").is_empty());
    }
}
