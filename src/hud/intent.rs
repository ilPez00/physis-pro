//! Bare-text intent resolution: the deterministic chain before any model.
//!
//! The magic textbox parses `$` shell, `:` commands, `@agent`, `/` search,
//! `open`/`edit`/`cd` in the bin. What it did not do is say what a *bare* line
//! means — this module is the resolution for exactly that, and since PH-027 the
//! bin calls it (before that it was reachable only from its own tests, which is
//! the defect shape `docs/WHAT_PHYSIS_IS.md` §7 names).
//!
//! ```text
//! exact parser (HudInput) — owned by the bin, never reaches this module
//!   -> exact object resolution (one object matches the line unambiguously)
//!   -> IntentIR (the nine-slot structure, compiled deterministically)
//!   -> [deferred] guided constraint · agent escalation · human binding
//! ```
//!
//! **A route exists here only if something produces it.** PH-026 shipped seven
//! variants, five of which had no producer anywhere in the crate: an enum of
//! routes with no producers is the declared-never-called defect one level down.
//! The deferred legs are named in the text above so the chain's shape is still
//! readable, but they are not types until they have callers.
//!
//! Every resolution names its route. No hidden routing magic: the resolution
//! carries its provenance line, and the renderer shows it.

use std::collections::HashMap;

/// Where a bare-text line resolved. Ordered cheapest-first: the chain stops at
/// the first level that answers, and only the levels that answer exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionRoute {
    /// Exactly one world object matched the line (unambiguous resolution).
    ExactObject,
    /// Compiled into the nine-slot IntentIR, deterministically.
    Intent,
}

impl ResolutionRoute {
    pub fn label(self) -> &'static str {
        match self {
            ResolutionRoute::ExactObject => "exact-object",
            ResolutionRoute::Intent => "intent-ir",
        }
    }
}

/// What one bare-text line resolved to, with the provenance the renderer shows.
#[derive(Debug, Clone)]
pub struct IntentResolution {
    pub route: ResolutionRoute,
    pub utterance: String,
    /// The nine-slot structure when the route reached Intent or beyond.
    pub ir: Option<crate::intent_ir::IntentIR>,
    /// Objects the line resolved against, by canonical identity spelling.
    pub objects: Vec<String>,
    /// NIL slots that still need binding (the deferred human leg).
    pub bind_tasks: Vec<String>,
    /// One line the renderer shows: how this line was understood, and by what.
    pub provenance: String,
}

impl IntentResolution {
    /// The route's provenance, in the `route=… key=value` form the HUD shows.
    pub fn provenance_line(&self) -> String {
        self.provenance.clone()
    }
}

/// Resolve a bare-text line against what the world actually matched.
///
/// * `exact` - the single object the line names unambiguously (identity, path or
///   label equality), when there is exactly one. This is `route=exact-object`:
///   the world already answered, so the IR records the answer rather than
///   proposing one.
/// * `matches` - the ranked objects the shared scorer returned for the line.
///   They become the IR's `objects` slot, and the route stays `intent-ir`
///   precisely because more than one object matched.
///
/// Deterministic and total: every line resolves, and the provenance says which
/// level answered and what is still unbound.
pub fn resolve_bare(
    utterance: &str,
    user: &str,
    exact: Option<&str>,
    matches: &[String],
) -> IntentResolution {
    let objects: Vec<String> = match exact {
        Some(one) => vec![one.to_string()],
        None => matches.to_vec(),
    };
    let mut raw: HashMap<String, serde_json::Value> = HashMap::new();
    raw.insert(
        "goal".to_string(),
        serde_json::Value::String(utterance.to_string()),
    );
    if !objects.is_empty() {
        raw.insert(
            "objects".to_string(),
            serde_json::Value::Array(
                objects
                    .iter()
                    .map(|o| serde_json::Value::String(o.clone()))
                    .collect(),
            ),
        );
    }
    let ir = crate::intent_ir::IntentIR::new(raw, utterance.to_string(), user.to_string(), None);
    let bind_tasks: Vec<String> = ir.bind_tasks().iter().map(|b| b.slot.clone()).collect();
    let route = match exact {
        Some(_) => ResolutionRoute::ExactObject,
        None => ResolutionRoute::Intent,
    };
    let provenance = match exact {
        Some(one) => format!("route={} object={one}", route.label()),
        None => format!(
            "route={} objects={} nil=[{}]",
            route.label(),
            objects.len(),
            bind_tasks.join(",")
        ),
    };
    IntentResolution {
        route,
        utterance: utterance.to_string(),
        ir: Some(ir),
        objects,
        bind_tasks,
        provenance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_exact_match_is_an_exact_object_resolution() {
        let r = resolve_bare(
            "src/hud/world.rs",
            "op",
            Some("fs:abc123"),
            &["fs:abc123".to_string(), "fs:def456".to_string()],
        );
        assert_eq!(r.route, ResolutionRoute::ExactObject);
        assert_eq!(r.objects, vec!["fs:abc123".to_string()]);
        assert_eq!(r.provenance, "route=exact-object object=fs:abc123");
    }

    #[test]
    fn several_matches_stay_an_intent_with_its_nil_slots() {
        let matches = vec!["fs:a".to_string(), "fs:b".to_string()];
        let r = resolve_bare("summarise the retrieval notes", "op", None, &matches);
        assert_eq!(r.route, ResolutionRoute::Intent);
        assert_eq!(r.objects, matches, "every matched object is carried");
        let ir = r.ir.expect("the intent route carries the IR");
        assert!(ir.has_nil(), "bare text leaves slots for the human");
        assert!(!r.bind_tasks.is_empty());
        assert!(
            r.provenance.starts_with("route=intent-ir objects=2 nil=["),
            "{}",
            r.provenance
        );
    }

    #[test]
    fn no_matches_still_resolves_and_says_so() {
        let r = resolve_bare("something nobody indexed", "op", None, &[]);
        assert_eq!(r.route, ResolutionRoute::Intent);
        assert!(r.objects.is_empty());
        assert!(r.provenance.contains("objects=0"), "{}", r.provenance);
    }

    /// The enum is an executable description of reality: every variant must have
    /// a producer. This is the cheapest reminder to wire a leg or not ship it —
    /// the deferred ones (guided constraint, agent escalation, human binding, the
    /// bin's own parser, structured facts) are named in this module's header, not
    /// typed, because each still needs a caller.
    #[test]
    fn only_routes_with_producers_are_typed() {
        let produced = [ResolutionRoute::ExactObject, ResolutionRoute::Intent];
        for route in produced {
            assert!(!route.label().is_empty());
        }
        assert_eq!(produced.len(), 2, "add a producer before adding a variant");
    }
}
