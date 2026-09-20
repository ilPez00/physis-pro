//! Guided Execution — Physis-native constrained generation.
//!
//! The model is asked only for the choice Physis cannot make. Everything else
//! (which objects exist, which verbs are legal on them, which ids are real,
//! which fields exist) is compiled from the world into the constraint, so an
//! invalid action is not a validation failure to retry — it is not in the
//! language at all.
//!
//! ```text
//! registry (objects + capabilities)   ← the world, already built
//!        ↓  WorldView            (affordance.rs)
//!        ↓  ActionLanguage       (affordance.rs)   verbs × legal targets
//!        ↓  ConstraintCompiler   (compile.rs)      GenerationContext (IR)
//!        ↓  GuidedModel          (model.rs)        mock now, adapters later
//!        ↓  GuidedAction::validate
//!        ↓  action::dispatch     (the existing executor)
//!        ↓  GuidedActionObservation (observe.rs)   into the registry's log
//! ```
//!
//! What this module does **not** do: it does not add an object model, an action
//! model, a permission model, or a log. Those are `nav`, `registry`,
//! `capability` and `capability::Observations`. It is a translation layer
//! between world state and model decoding.
//!
//! Phase A (this module) ships: typed affordances, `Choice`/`ObjectSet`/
//! `FreeText` constraints, the `GuidedAction` schema, a world → constraint
//! compiler, a deterministic mock model, one-pass deterministic resolution for
//! exact facts, validation, and outcome observation. Later phases add provider
//! backends and CFG composition on top of the same IR — not a second IR.

pub mod action;
pub mod affordance;
pub mod compile;
pub mod constraint;
pub mod model;
pub mod observe;

pub use action::{GuidedAction, GuidedArgs, Rejection};
pub use affordance::{
    Access, ActionLanguage, Affordance, ArgSpec, NumberSpec, ObjectHandle, ObjectRef, VerbBranch,
    WorldView, READ_VERBS,
};
pub use compile::ConstraintCompiler;
pub use constraint::{
    ChoiceSet, ChoiceValue, Constraint, ConstraintProvenance, Field, FreeField, GenerationContext,
    ObjectSet, TokenClasses,
};
pub use model::{GuidedModel, Metrics, MockGuidedModel, Route};
pub use observe::GuidedActionObservation;

use crate::registry::Registry;

/// One step of the loop: what the world allowed, what was chosen, and how the
/// choice was reached.
#[derive(Debug, Clone)]
pub struct GuidedStep {
    pub context: GenerationContext,
    pub action: GuidedAction,
    /// `Route::Exact` means no model was consulted at all.
    pub route: Route,
}

/// The loop over one world state. Holds the access mode and the counters — not
/// the world, the log or the capabilities: those live where they already lived.
#[derive(Debug)]
pub struct GuidedExecution {
    access: Access,
    pub metrics: Metrics,
}

impl Default for GuidedExecution {
    fn default() -> Self {
        Self {
            access: Access::ReadOnly,
            metrics: Metrics::default(),
        }
    }
}

impl GuidedExecution {
    /// Read-only by default: an actor must ask for write reach, so a grammar can
    /// never widen by accident.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_access(access: Access) -> Self {
        Self {
            access,
            metrics: Metrics::default(),
        }
    }

    pub fn access(&self) -> Access {
        self.access
    }

    /// Compile the world, resolve or choose an action, validate it against the
    /// constraint it came from, and (when `execute`) hand it to the existing
    /// action surface. The observation is recorded either way.
    pub fn step(
        &mut self,
        reg: &mut Registry,
        focus: Option<&str>,
        intent: &str,
        model: &dyn GuidedModel,
        execute: bool,
    ) -> Result<(GuidedStep, Option<String>), String> {
        let world = WorldView::from_registry(reg);
        let focus = focus.and_then(|f| world.resolve(f));
        let ctx = ConstraintCompiler::compile(&world, focus.as_ref(), intent, self.access);

        // Deterministic short-circuit: when the intent already names exactly one
        // object and the only legal verbs on it are read-shaped, Physis answers
        // and the model is never asked. This is the "CPU temperature" path.
        if let Some(action) = model::resolve_exact(&ctx, intent) {
            self.metrics.deterministic_bypasses += 1;
            let outcome = if execute {
                Some(Self::execute(reg, &action)?)
            } else {
                None
            };
            let step = GuidedStep {
                context: ctx,
                action,
                route: Route::Exact,
            };
            GuidedActionObservation::from_step(&step, intent, outcome.as_deref(), execute)
                .record(&crate::registry::default_obs_path());
            return Ok((step, outcome));
        }

        let action = model.choose(&ctx, intent)?;
        self.metrics.model_calls += 1;
        if let Err(rejection) = action.validate(&ctx) {
            self.metrics.rejections += 1;
            return Err(rejection.to_string());
        }
        let outcome = if execute {
            Some(Self::execute(reg, &action)?)
        } else {
            None
        };
        let step = GuidedStep {
            context: ctx,
            action,
            route: Route::Model,
        };
        GuidedActionObservation::from_step(&step, intent, outcome.as_deref(), execute)
            .record(&crate::registry::default_obs_path());
        Ok((step, outcome))
    }

    /// Hand the chosen action to the surface that already exists: one executor
    /// for the CLI, the HUD and guided execution.
    fn execute(reg: &mut Registry, action: &GuidedAction) -> Result<String, String> {
        let (_, rendered) = crate::action::dispatch(reg, &action.action_line())?;
        Ok(rendered)
    }
}

impl GuidedStep {
    /// The one card, rendered once and used by every frontend (CLI, HUD, and
    /// later the textbox). A second renderer would drift from this one.
    pub fn render(&self, metrics: &Metrics) -> String {
        let mut out = self.context.render();
        out.push_str("\nCHOSEN\n");
        out.push_str(&format!("  {}\n", self.action.action_line()));
        out.push_str(&format!(
            "  route: {}\n",
            match self.route {
                Route::Exact => "exact - Physis answered, no model was asked",
                Route::Model => "model - chosen within the constraint",
            }
        ));
        out.push_str(&format!(
            "  runnable as: physis-pro action call {}\n",
            self.action.action_line()
        ));
        out.push_str(&format!(
            "\nCOST OF THIS STEP\n  model calls: {} · deterministic bypasses: {} · rejections: {}\n",
            metrics.model_calls, metrics.deterministic_bypasses, metrics.rejections
        ));
        match metrics.bypass_rate() {
            Some(rate) => out.push_str(&format!(
                "  bypass rate: {:.0}% of steps never reached a model\n",
                rate * 100.0
            )),
            None => out.push_str("  bypass rate: — (no steps yet)\n"),
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use std::collections::HashMap;

    /// A test world: a registry with three objects and two capabilities. Built
    /// from the same constructors the real registry uses — no fixture format.
    fn registry_with(objects: &[(&str, &str, &str, Option<&str>)]) -> Registry {
        let dir = std::env::temp_dir().join(format!("physis-guided-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut reg = Registry::new(dir.join("guided_observations.jsonl"));
        for (oid, kind, path, state) in objects {
            let mut meta = HashMap::new();
            meta.insert("label".to_string(), serde_json::Value::String(path.to_string()));
            if let Some(s) = state {
                meta.insert("state".to_string(), serde_json::Value::String(s.to_string()));
            }
            reg.register_object(oid.to_string(), kind.to_string(), path.to_string(), meta);
        }
        reg
    }

    fn read_cap() -> Capability {
        Capability::new(
            "fs.read".into(),
            vec!["file".into(), "note".into(), "service".into()],
            vec!["read".into()],
            "api".into(),
            "reversible".into(),
            HashMap::new(),
            None,
        )
    }

    fn service_caps() -> Vec<Capability> {
        ["status", "start", "stop", "restart"]
            .iter()
            .map(|verb| {
                Capability::new(
                    format!("service.{verb}"),
                    vec!["service".into()],
                    vec![(*verb).to_string()],
                    "cli".into(),
                    "reversible".into(),
                    HashMap::new(),
                    None,
                )
            })
            .collect()
    }

    fn write_cap() -> Capability {
        Capability::new(
            "fs.write".into(),
            vec!["file".into()],
            vec!["write".into()],
            "api".into(),
            "draft_only".into(),
            HashMap::new(),
            None,
        )
    }

    fn world_with_service(state: Option<&str>, access: Access) -> GenerationContext {
        let mut reg = registry_with(&[(
            "service:praxis-web",
            "service",
            "praxis-web.service",
            state,
        )]);
        reg.add_capability(read_cap());
        for cap in service_caps() {
            reg.add_capability(cap);
        }
        let world = WorldView::from_registry(&reg);
        ConstraintCompiler::compile(&world, world.resolve("praxis-web").as_ref(), "restart praxis", access)
    }

    /// A state change moves the language: a running service can be stopped or
    /// restarted, a stopped one can be started, and neither can do the other's
    /// verbs. The grammar is world state, not prompt configuration.
    #[test]
    fn world_state_changes_the_language() {
        let running = world_with_service(Some("running"), Access::ReadWrite);
        let verbs = running.verbs();
        assert!(verbs.contains(&"stop".to_string()), "{verbs:?}");
        assert!(verbs.contains(&"restart".to_string()), "{verbs:?}");
        assert!(!verbs.contains(&"start".to_string()), "a running service is not started: {verbs:?}");

        let stopped = world_with_service(Some("stopped"), Access::ReadWrite);
        let verbs = stopped.verbs();
        assert!(verbs.contains(&"start".to_string()), "{verbs:?}");
        assert!(!verbs.contains(&"stop".to_string()), "a stopped service is not stopped: {verbs:?}");

        // An object whose state is unknown gets only the safe transition.
        let unknown = world_with_service(None, Access::ReadWrite);
        assert!(unknown.verbs().contains(&"start".to_string()));
        assert!(!unknown.verbs().contains(&"stop".to_string()));
    }

    /// Read-only access removes the mutating verbs before compilation, so a
    /// write is not a rejected action — it is not a sentence in the language.
    #[test]
    fn access_removes_mutating_verbs_from_the_language() {
        let mut reg = registry_with(&[("fs:a", "file", "src/a.rs", None)]);
        reg.add_capability(read_cap());
        reg.add_capability(write_cap());
        let world = WorldView::from_registry(&reg);
        let target = world.resolve("src/a.rs");

        let ro = ConstraintCompiler::compile(&world, target.as_ref(), "edit a", Access::ReadOnly);
        assert_eq!(ro.verbs(), vec!["read"], "read-only keeps reads only");
        let rw = ConstraintCompiler::compile(&world, target.as_ref(), "edit a", Access::ReadWrite);
        assert!(rw.verbs().contains(&"write".to_string()));
    }

    /// An object the world does not contain cannot be emitted — and if a stale
    /// reply carries one anyway, validation refuses it.
    #[test]
    fn an_invalid_object_can_neither_be_offered_nor_accepted() {
        let mut reg = registry_with(&[("fs:a", "file", "src/a.rs", None)]);
        reg.add_capability(read_cap());
        let world = WorldView::from_registry(&reg);
        let ctx = ConstraintCompiler::compile(&world, None, "read a", Access::ReadWrite);

        assert_eq!(ctx.targets("read").iter().map(|t| t.handle.to_string()).collect::<Vec<_>>(), vec!["fs:a"]);
        let ghost = GuidedAction::new(
            "read",
            ObjectRef::from_world("fs:ghost".into(), "file".into(), "src/ghost.rs".into(), "ghost".into(), None).handle,
            GuidedArgs::None,
        );
        assert_eq!(
            ghost.validate(&ctx),
            Err(Rejection::TargetNotVisible {
                verb: "read".into(),
                target: "fs:ghost".into()
            })
        );
        let unknown_verb = GuidedAction::new("delete", ctx.targets("read")[0].handle.clone(), GuidedArgs::None);
        assert_eq!(
            unknown_verb.validate(&ctx),
            Err(Rejection::VerbNotInLanguage { verb: "delete".into() })
        );
    }

    /// An exact fact does not go through generation: one named object, one legal
    /// read verb, zero model calls.
    #[test]
    fn deterministic_resolution_bypasses_the_model() {
        let mut reg = registry_with(&[("sensor:cpu", "note", "cpu.temperature", None)]);
        reg.add_capability(read_cap());
        let world = WorldView::from_registry(&reg);
        let ctx = ConstraintCompiler::compile(&world, None, "cpu temperature", Access::ReadOnly);
        let action = model::resolve_exact(&ctx, "cpu temperature").expect("one sensor, one read verb");
        assert_eq!(action.action_line(), "read sensor:cpu");

        // Two candidates is a choice, and a choice belongs to the model.
        let mut reg = registry_with(&[
            ("sensor:cpu", "note", "cpu.temperature", None),
            ("sensor:gpu", "note", "gpu.temperature", None),
        ]);
        reg.add_capability(read_cap());
        let world = WorldView::from_registry(&reg);
        let ctx = ConstraintCompiler::compile(&world, None, "temperature", Access::ReadOnly);
        assert!(model::resolve_exact(&ctx, "temperature").is_none());
        // …and the mock model still stays inside the world while choosing.
        let chosen = MockGuidedModel.choose(&ctx, "gpu temperature").unwrap();
        assert!(chosen.validate(&ctx).is_ok());
        assert_eq!(chosen.target.to_string(), "sensor:gpu");
    }

    /// One IR, three backends: the mock chooser, a JSON schema, and a grammar
    /// all read the same compiled context.
    #[test]
    fn one_constraint_compiles_to_three_backends() {
        let ctx = world_with_service(Some("running"), Access::ReadWrite);
        let schema = ctx.constraint.json_schema();
        let grammar = ctx.constraint.grammar();

        let verbs = schema["properties"]["verb"]["enum"].as_array().unwrap();
        assert!(verbs.iter().any(|v| v == "restart"));
        let targets = schema["properties"]["target"]["enum"].as_array().unwrap();
        assert_eq!(targets.len(), 1, "one service in this world");
        assert!(grammar.contains("\"restart\""), "{grammar}");
        assert!(grammar.contains("\"service:praxis-web\""), "{grammar}");
        // The mock backend consumes the very same context and stays inside it.
        let action = MockGuidedModel.choose(&ctx, "restart praxis-web").unwrap();
        assert!(action.validate(&ctx).is_ok());
        assert_eq!(action.verb, "restart");
    }

    /// What the constraint already knows: fixed tokens Physis emits, the closed
    /// sets the model chooses within, and the one free field. No model tokens
    /// are claimed — that needs a real backend.
    #[test]
    fn token_classes_measure_the_entropy_physis_removed() {
        let ctx = world_with_service(Some("running"), Access::ReadWrite);
        let classes = ctx.constraint.token_classes();
        assert_eq!(classes.choice_fields, 2, "verb + target");
        // 4 verbs legal while the service is running (read, status, stop,
        // restart — `start` is gone) plus the one service to act on.
        assert_eq!(classes.choice_candidates, 4 + 1);
        // The one free field is `read`'s bounded `limit`; the service verbs need
        // no model-written text at all. Bounded, because an unbounded field is a
        // way to smuggle structure back in.
        assert_eq!(classes.free_fields, 1);
        assert_eq!(
            ctx.constraint.json_schema()["properties"]["limit"]["maxLength"],
            serde_json::json!(12)
        );
        assert!(classes.entropy_bits > 0.0);
    }

    /// Every step leaves evidence in the log the action surface already writes,
    /// and the counters say how the choice was reached.
    #[test]
    fn a_step_records_outcome_evidence_and_counts_bypasses() {
        let mut reg = registry_with(&[("sensor:cpu", "note", "cpu.temperature", None)]);
        reg.add_capability(read_cap());
        let mut exec = GuidedExecution::with_access(Access::ReadOnly);
        let (step, outcome) = exec
            .step(&mut reg, None, "cpu temperature", &MockGuidedModel, false)
            .unwrap();
        assert_eq!(step.route, Route::Exact);
        assert!(outcome.is_none(), "not executed means no outcome, not a failure");
        assert_eq!(exec.metrics.model_calls, 0);
        assert_eq!(exec.metrics.deterministic_bypasses, 1);
        assert_eq!(exec.metrics.bypass_rate(), Some(1.0));
        let obs = GuidedActionObservation::from_step(&step, "cpu temperature", None, false);
        assert!(!obs.executed);
        assert_eq!(obs.target, "sensor:cpu");
        assert_eq!(obs.capability, "fs.read");
    }

    /// Provenance is part of the contract: the same world compiles to the same
    /// version, and a changed world does not.
    #[test]
    fn provenance_versions_the_world_slice() {
        let a = world_with_service(Some("running"), Access::ReadWrite);
        let b = world_with_service(Some("running"), Access::ReadWrite);
        assert_eq!(a.provenance.world_version, b.provenance.world_version);
        assert!(a.provenance.sources.iter().any(|s| s == "service:praxis-web"));
        let stopped = world_with_service(Some("stopped"), Access::ReadWrite);
        assert_ne!(a.provenance.sources, stopped.provenance.sources);
        assert_eq!(a.provenance.version, constraint::CONSTRAINT_VERSION);
        // The card a human reads names the intent, the transitions and the cap.
        let card = a.render();
        assert!(card.contains("VALID TRANSITIONS"), "{card}");
        assert!(card.contains("ENTROPY"), "{card}");
    }
}
