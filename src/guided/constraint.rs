//! The constraint IR: the one semantic representation of what the model may say.
//!
//! Provider-native schemas are compilation *targets*, never the source of truth
//! (`Constraint::json_schema`, `Constraint::grammar`). Phase A ships the three
//! constraint kinds a world can answer for itself:
//!
//! - `Choice` — a closed value set (verbs, modes, flags).
//! - `ObjectSet` — a closed set of real object handles.
//! - `FreeText` — the one place the model writes novel text (a query, a body).
//!
//! A `Sequence` composes them into one action. Nothing here validates
//! semantics: `GuidedAction::validate` does, against `GenerationContext`, which
//! also carries the per-verb target relation a flat schema cannot express.

use serde::{Deserialize, Serialize};

use super::affordance::ObjectRef;

/// One permitted value. `id` is what the model emits; `display` is what a human
/// reads; `metadata` ties it back to the world object when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceValue {
    pub id: String,
    pub display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ObjectRef>,
}

impl ChoiceValue {
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            metadata: None,
        }
    }

    pub fn of_object(object: &ObjectRef) -> Self {
        Self {
            id: object.handle.as_str().to_string(),
            display: format!("{} {} {}", object.handle, object.kind, object.path),
            metadata: Some(object.clone()),
        }
    }
}

/// A closed set of values: literal constraint, `Choice` in Outlines terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceSet {
    pub name: String,
    pub values: Vec<ChoiceValue>,
}

/// A closed set of world objects. Kept distinct from `ChoiceSet` because it
/// must be *derivable from the world*, and because a target is not a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectSet {
    pub name: String,
    pub objects: Vec<ObjectRef>,
}

/// The one field where the model generates: free text, bounded because an
/// unbounded field is a way to smuggle structure back in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreeField {
    pub name: String,
    pub max_chars: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

/// One named slot of the action. Fields are ordered, so the fixed text between
/// them (field names, punctuation) is Physis's to emit, not the model's to
/// regenerate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub constraint: Constraint,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Constraint {
    Choice(ChoiceSet),
    ObjectSet(ObjectSet),
    FreeText(FreeField),
    Sequence(Vec<Field>),
}

/// The contract this build reads; a compiled constraint is stamped with it, so a
/// stale context is visible rather than silent.
pub const CONSTRAINT_VERSION: u32 = 1;


/// How much of a generation Physis already knows. Counted, not estimated:
/// `fixed` is what Physis emits itself, `entropy_bits` is the choice the model
/// actually faces (sum of log2 over the closed-set fields), `free_fields` is
/// where novel text is genuinely required.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenClasses {
    pub fixed: usize,
    pub choice_fields: usize,
    pub choice_candidates: usize,
    pub free_fields: usize,
    pub entropy_bits: f64,
}

impl Constraint {
    fn walk_tokens(&self, classes: &mut TokenClasses) {
        match self {
            Constraint::Choice(set) => {
                classes.choice_fields += 1;
                classes.choice_candidates += set.values.len();
                classes.fixed += 1; // the field name itself
                if set.values.len() > 1 {
                    classes.entropy_bits += (set.values.len() as f64).log2();
                }
            }
            Constraint::ObjectSet(set) => {
                classes.choice_fields += 1;
                classes.choice_candidates += set.objects.len();
                classes.fixed += 1;
                if set.objects.len() > 1 {
                    classes.entropy_bits += (set.objects.len() as f64).log2();
                }
            }
            Constraint::FreeText(_) => {
                classes.free_fields += 1;
                classes.fixed += 1;
            }
            Constraint::Sequence(fields) => {
                for field in fields {
                    classes.fixed += 1; // field name
                    field.constraint.walk_tokens(classes);
                }
            }
        }
    }

    /// How much inference Physis made unnecessary for this shape.
    pub fn token_classes(&self) -> TokenClasses {
        let mut classes = TokenClasses {
            fixed: 0,
            choice_fields: 0,
            choice_candidates: 0,
            free_fields: 0,
            entropy_bits: 0.0,
        };
        self.walk_tokens(&mut classes);
        classes
    }

    /// JSON Schema, for providers that take a schema. Phase B adapters compile
    /// from this; nothing in Phase A talks to a provider.
    pub fn json_schema(&self) -> serde_json::Value {
        match self {
            Constraint::Choice(set) => serde_json::json!({
                "type": "string",
                "enum": set.values.iter().map(|v| v.id.clone()).collect::<Vec<_>>(),
            }),
            Constraint::ObjectSet(set) => serde_json::json!({
                "type": "string",
                "enum": set.objects.iter().map(|o| o.handle.as_str()).collect::<Vec<_>>(),
            }),
            Constraint::FreeText(field) => serde_json::json!({
                "type": "string",
                "maxLength": field.max_chars,
            }),
            Constraint::Sequence(fields) => {
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                for field in fields {
                    properties.insert(field.name.clone(), field.constraint.json_schema());
                    if field.required {
                        required.push(serde_json::Value::String(field.name.clone()));
                    }
                }
                serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                    "additionalProperties": false,
                })
            }
        }
    }

    /// A grammar rendering: alternatives for closed sets, a non-terminal for
    /// free text. This is the shape a llama.cpp-style backend consumes, and the
    /// shape a human reads to see what the model could have said.
    pub fn grammar(&self) -> String {
        match self {
            Constraint::Choice(set) => {
                let alternatives: Vec<String> =
                    set.values.iter().map(|v| quoted(&v.id)).collect();
                format!("(<{}> := {})", set.name, alternatives.join(" | "))
            }
            Constraint::ObjectSet(set) => {
                let alternatives: Vec<String> = set
                    .objects
                    .iter()
                    .map(|o| quoted(o.handle.as_str()))
                    .collect();
                format!("(<{}> := {})", set.name, alternatives.join(" | "))
            }
            Constraint::FreeText(field) => {
                format!("(<{}> := <text:{}>)", field.name, field.max_chars)
            }
            Constraint::Sequence(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|field| field.constraint.grammar())
                    .collect();
                parts.join(" ")
            }
        }
    }
}

fn quoted(text: &str) -> String {
    format!("\"{text}\"")
}

/// Where a constraint came from. A constraint the operator cannot trace is an
/// oracle, and this project does not ship oracles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintProvenance {
    /// Deterministic version of the world slice the constraint was built from.
    pub world_version: String,
    /// Objects the constraint was derived from (focus first).
    pub sources: Vec<String>,
    pub visible_objects: usize,
    pub affordances: usize,
    pub access: String,
    pub generated_at: String,
    pub version: u32,
}

/// Everything the model may say about one step, plus why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationContext {
    pub intent: String,
    pub focus: Option<ObjectRef>,
    pub visible_objects: Vec<ObjectRef>,
    pub constraint: Constraint,
    pub provenance: ConstraintProvenance,
    /// Per-verb legal targets. A flat schema cannot express "this target is
    /// legal for that verb", so the relation rides beside the constraint and
    /// `GuidedAction::validate` enforces it.
    pub targets_by_verb: std::collections::BTreeMap<String, Vec<ObjectRef>>,
    /// The capability each verb would call, so an observation names the
    /// executor rather than re-deriving it.
    pub capability_by_verb: std::collections::BTreeMap<String, String>,
}

impl GenerationContext {
    /// The verbs currently legal on the world slice.
    pub fn verbs(&self) -> Vec<String> {
        self.targets_by_verb.keys().cloned().collect()
    }

    /// Targets legal for one verb, empty when the verb is not in the language.
    pub fn targets(&self, verb: &str) -> &[ObjectRef] {
        self.targets_by_verb
            .get(verb)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// The human card: what the world resolved to, what the model may choose,
    /// and how much of the answer Physis already knew.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("INTENT\n  {}\n", self.intent));
        out.push_str("\nRESOLVED WORLD\n");
        match &self.focus {
            Some(f) => out.push_str(&format!("  focus: {} ({})\n", f.handle, f.kind)),
            None => out.push_str("  focus: none\n"),
        }
        out.push_str(&format!(
            "  visible objects: {}\n  access: {}\n",
            self.visible_objects.len(),
            self.provenance.access
        ));
        out.push_str(&format!(
            "  world version: {} · constraint v{}\n",
            self.provenance.world_version, self.provenance.version
        ));
        out.push_str("\nVALID TRANSITIONS\n");
        if self.targets_by_verb.is_empty() {
            out.push_str("  (none — no capability matches this world slice)\n");
        }
        for (verb, targets) in &self.targets_by_verb {
            out.push_str(&format!("  {verb} → {} target(s)\n", targets.len()));
            for target in targets.iter().take(8) {
                out.push_str(&format!("      {} ({})\n", target.handle, target.kind));
            }
            if targets.len() > 8 {
                out.push_str(&format!("      … {} more\n", targets.len() - 8));
            }
        }
        out.push_str("\nCONSTRAINT\n");
        out.push_str(&format!("  {}\n", self.constraint.grammar()));
        out.push_str("\nSCHEMA (json)\n");
        out.push_str(&format!(
            "  {}\n",
            serde_json::to_string(&self.constraint.json_schema()).unwrap_or_default()
        ));
        let classes = self.constraint.token_classes();
        out.push_str("\nENTROPY\n");
        out.push_str(&format!(
            "  fixed tokens Physis emits: {}\n  choice fields: {} ({} candidates, {:.2} bits)\n  free-generation fields: {}\n  sources: {}\n",
            classes.fixed,
            classes.choice_fields,
            classes.choice_candidates,
            classes.entropy_bits,
            classes.free_fields,
            self.provenance.sources.join(", ")
        ));
        out
    }
}
