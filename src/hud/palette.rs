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
}
