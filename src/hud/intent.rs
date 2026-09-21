//! Bare-text intent resolution: the deterministic chain before any model.
//!
//! The magic textbox already parses `$` shell, `:` commands, `@agent`, `/`
//! search, `open`/`edit`/`cd` in the bin. What it did not do is say what a
//! *bare* line means. This module is the resolution chain for exactly that:
//!
//! ```text
//! exact parser (HudInput — already handled by the bin)
//!   -> exact structured fact (a `fact:` answer the world already holds)
//!   -> exact object resolution (WorldView::resolve — one named object)
//!   -> IntentIR (the nine-slot structure, compiled deterministically)
//!   -> guided constraint (the world compiled into the model's language)
//!   -> explicit agent escalation (a card, never a hidden call)
//!   -> human (the bind tasks for the NIL slots)
//! ```
//!
//! Every step names its route. No hidden routing magic: the resolution
//! carries its provenance line, and the renderer shows it.

use std::collections::HashMap;

/// Where a bare-text line resolved. Ordered cheapest-first: the chain tries
/// each level in order and stops at the first that answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionRoute {
    /// The bin's exact parser already owned this line (not bare text at all).
    Parsed,
    /// An exact structured fact the world already holds (no model, no search).
    ExactFact,
    /// Exactly one world object matched (WorldView::resolve was unambiguous).
    ExactObject,
    /// Compiled into the nine-slot IntentIR, deterministically.
    Intent,
    /// Compiled further into a guided constraint for a model to choose inside.
    Guided,
    /// No deterministic level answered: the named agent is asked, on a worker.
    AgentEscalation,
    /// Not even an agent is configured: the human gets the bind tasks.
    Human,
}

impl ResolutionRoute {
    pub fn label(self) -> &'static str {
        match self {
            ResolutionRoute::Parsed => "parsed",
            ResolutionRoute::ExactFact => "exact-fact",
            ResolutionRoute::ExactObject => "exact-object",
            ResolutionRoute::Intent => "intent-ir",
            ResolutionRoute::Guided => "guided",
            ResolutionRoute::AgentEscalation => "agent",
            ResolutionRoute::Human => "human",
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
    /// Objects the line resolved against, by handle.
    pub objects: Vec<String>,
    /// NIL slots that still need binding (the human leg of the chain).
    pub bind_tasks: Vec<String>,
    /// One line the renderer shows: how this line was understood, and by what.
    pub provenance: String,
}

/// Compile a bare-text line into the nine-slot IntentIR without a model: the
/// goal is the utterance, objects/constraints/context carry what the caller
/// resolved, everything else is NIL with an explicit bind task. Deterministic
/// and total: every line compiles, and `render()` shows what is still empty.
pub fn compile_bare(utterance: &str, user: &str, objects: Vec<String>) -> IntentResolution {
    let mut raw: HashMap<String, serde_json::Value> = HashMap::new();
    raw.insert("goal".to_string(), serde_json::Value::String(utterance.to_string()));
    if !objects.is_empty() {
        raw.insert(
            "objects".to_string(),
            serde_json::Value::Array(
                objects.iter().map(|o| serde_json::Value::String(o.clone())).collect(),
            ),
        );
    }
    let ir = crate::intent_ir::IntentIR::new(raw, utterance.to_string(), user.to_string(), None);
    let bind_tasks = ir.bind_tasks().iter().map(|b| b.slot.clone()).collect::<Vec<_>>();
    let route = if objects.len() == 1 {
        ResolutionRoute::ExactObject
    } else {
        ResolutionRoute::Intent
    };
    let provenance = format!(
        "route={} objects={} nil=[{}] (deterministic compile, no model)",
        route.label(),
        objects.len(),
        bind_tasks.join(","),
    );
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
    fn bare_text_compiles_to_intent_with_nil_bind_tasks() {
        let r = compile_bare("summarise the retrieval notes", "op", vec![]);
        assert_eq!(r.route, ResolutionRoute::Intent);
        let ir = r.ir.expect("intent route carries the IR");
        assert!(ir.has_nil(), "bare text leaves slots for the human");
        assert!(!r.bind_tasks.is_empty());
        assert!(r.provenance.contains("route=intent-ir"), "{}", r.provenance);
        assert!(r.provenance.contains("no model"), "{}", r.provenance);
    }

    #[test]
    fn one_resolved_object_is_an_exact_object_resolution() {
        let r = compile_bare("read it", "op", vec!["fs:abc123".to_string()]);
        assert_eq!(r.route, ResolutionRoute::ExactObject);
        assert_eq!(r.objects, vec!["fs:abc123".to_string()]);
        assert!(r.provenance.contains("route=exact-object"), "{}", r.provenance);
    }

    #[test]
    fn every_resolution_names_its_route() {
        for route in [
            ResolutionRoute::Parsed,
            ResolutionRoute::ExactFact,
            ResolutionRoute::ExactObject,
            ResolutionRoute::Intent,
            ResolutionRoute::Guided,
            ResolutionRoute::AgentEscalation,
            ResolutionRoute::Human,
        ] {
            assert!(!route.label().is_empty());
        }
    }
}
