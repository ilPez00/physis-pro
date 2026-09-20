//! World → constraint compilation.
//!
//! Pure with respect to the world: the same world slice, focus, intent and
//! access produce the same context, including the same provenance hash. Nothing
//! here reads a clock except to stamp `generated_at` on the provenance, and
//! nothing here calls a model.

use std::collections::BTreeMap;

use super::affordance::{Access, ObjectRef, WorldView};
use super::constraint::{
    ConstraintProvenance, GenerationContext, CONSTRAINT_VERSION,
};

/// Compile the current world slice into the model's allowed language.
pub struct ConstraintCompiler;

impl ConstraintCompiler {
    pub fn compile(
        world: &WorldView,
        focus: Option<&ObjectRef>,
        intent: &str,
        access: Access,
    ) -> GenerationContext {
        let language = world.language(access);
        let mut targets_by_verb: BTreeMap<String, Vec<ObjectRef>> = BTreeMap::new();
        let mut capability_by_verb: BTreeMap<String, String> = BTreeMap::new();
        for branch in &language.branches {
            targets_by_verb.insert(branch.verb.clone(), branch.targets.clone());
            capability_by_verb
                .entry(branch.verb.clone())
                .or_insert_with(|| branch.capability.clone());
        }

        // The focus scales the world slice down before the model sees it: the
        // focus object first, then its kind's neighbours, then everything else
        // the language can reach. Ordering is part of the contract — a model
        // reads the head of this list first.
        let mut visible: Vec<ObjectRef> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        if let Some(f) = focus {
            if seen.insert(f.handle.clone()) {
                visible.push(f.clone());
            }
            for o in world
                .objects
                .iter()
                .filter(|o| o.kind == f.kind && o.handle != f.handle)
            {
                if seen.insert(o.handle.clone()) {
                    visible.push(o.clone());
                }
            }
        }
        for verb in targets_by_verb.keys() {
            for t in &targets_by_verb[verb] {
                if seen.insert(t.handle.clone()) {
                    visible.push(t.clone());
                }
            }
        }

        let mut sources: Vec<String> = Vec::new();
        if let Some(f) = focus {
            sources.push(f.handle.to_string());
        }
        for branch in &language.branches {
            sources.push(format!("{}:{}", branch.verb, branch.capability));
        }
        sources.dedup();

        let provenance = ConstraintProvenance {
            world_version: world.version(),
            sources,
            visible_objects: visible.len(),
            affordances: language.branches.len(),
            access: access.label().to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            version: CONSTRAINT_VERSION,
        };

        GenerationContext {
            intent: intent.to_string(),
            focus: focus.cloned(),
            visible_objects: visible,
            constraint: language.constraint(),
            provenance,
            targets_by_verb,
            capability_by_verb,
        }
    }
}
