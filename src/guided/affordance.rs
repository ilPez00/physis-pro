//! Affordances: what the current world permits, derived — never hand-listed.
//!
//! The source is the registry's capability table (`config/actions.json`), the
//! same one `physis-pro action` uses. An affordance is therefore not a new
//! concept: it is a `Capability` seen from the generation side, plus the set of
//! registered objects whose kind it covers.
//!
//! Permissions ride here too. `Access::ReadOnly` removes the mutating verbs
//! *before* the constraint is compiled, so a read-only actor cannot emit a
//! write: the verb is not in the language, not rejected after the fact.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::registry::Registry;

use super::constraint::{ChoiceSet, ChoiceValue, Constraint, Field, FreeField, ObjectSet};

/// A handle to a world object.
///
/// The field is private and the only constructors are the world listings, so a
/// caller cannot type an id into existence: holding an `ObjectHandle` is the
/// proof that the object was in the world. That is the whole reason `target` is
/// not a `String`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectHandle(String);

impl ObjectHandle {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Only world listings call this. Public so `Registry`-derived code in this
    /// module can build objects without a second constructor path.
    fn from_world(oid: String) -> Self {
        Self(oid)
    }
}

impl fmt::Display for ObjectHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A world object as the constraint layer sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRef {
    pub handle: ObjectHandle,
    pub kind: String,
    pub path: String,
    pub label: String,
    /// The object's own `state` meta, when it has one (a service that is
    /// running, a machine that is stopped). Read from the registry's meta map,
    /// so no new store is created for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl ObjectRef {
    /// World-listing constructor: the only way an `ObjectRef` comes into being.
    pub(crate) fn from_world(
        oid: String,
        kind: String,
        path: String,
        label: String,
        state: Option<String>,
    ) -> Self {
        Self {
            handle: ObjectHandle::from_world(oid),
            kind,
            path,
            label,
            state,
        }
    }
}

/// Verbs whose legality depends on the object's own `state`. Data, not a switch
/// buried in the compiler, and open-ended: a verb with no gate is legal in every
/// state. The values are the vocabulary `registry` meta may hold.
pub fn state_gate(verb: &str) -> Option<&'static [&'static str]> {
    match verb {
        "start" => Some(&["stopped", "failed", "unknown"]),
        "stop" | "restart" => Some(&["running"]),
        _ => None,
    }
}

/// Whether a verb is legal while the object is in `state`.
///
/// No gate means yes. With a gate, a missing state reads as `unknown`: an object
/// whose state the world has not established must not be handed a transition
/// that assumes it.
pub fn verb_allowed_in_state(verb: &str, state: Option<&str>) -> bool {
    let Some(allowed) = state_gate(verb) else {
        return true;
    };
    let state = state.unwrap_or("unknown");
    allowed.contains(&state)
}

/// Verbs that only read. Their complement inside `capability::TEN_VERBS` is the
/// mutating set — one list, so the two cannot drift.
pub const READ_VERBS: [&str; 4] = ["read", "show", "inspect", "search"];

/// Who is asking. Read-only is the default everywhere: widening is a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

impl Access {
    pub fn allows(self, verb: &str) -> bool {
        match self {
            Access::ReadWrite => true,
            Access::ReadOnly => READ_VERBS.contains(&verb),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Access::ReadOnly => "read-only",
            Access::ReadWrite => "read-write",
        }
    }
}

/// A capability seen from the generation side: the verb it grants, the kinds it
/// covers, and the risk a caller must respect when actually calling it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Affordance {
    pub verb: String,
    pub capability: String,
    pub object_kinds: Vec<String>,
    pub risk: String,
    pub tier: String,
    pub latency_p50_s: Option<f64>,
}


/// A bounded integer argument. Not a free string: the executor parses a number,
/// so the constraint says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberSpec {
    pub name: String,
    pub min: u64,
    pub max: u64,
}

/// Extra argument a verb takes, so a free-text field appears only when a verb
/// actually needs one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArgSpec {
    /// A free-text argument (a query, a body) — the model's only genuine
    /// generation slot.
    Text { name: String, max_chars: usize },
    /// A closed set of modes.
    Mode { name: String, values: Vec<String> },
    /// A boolean flag, emitted by Physis unless the operator sets it.
    Flag { name: String },
    /// A bounded integer.
    Number(NumberSpec),
}

impl ArgSpec {
    pub fn name(&self) -> String {
        match self {
            ArgSpec::Text { name, .. }
            | ArgSpec::Mode { name, .. }
            | ArgSpec::Flag { name } => name.clone(),
            ArgSpec::Number(spec) => spec.name.clone(),
        }
    }
}

/// Arguments a verb takes, taken from what the executors actually read
/// (`registry.rs`: `limit`, `content`, `cmd` + `confirm`). Data in one place, so
/// a constraint cannot invent an argument no executor consumes.
pub fn args_for(verb: &str) -> Vec<ArgSpec> {
    match verb {
        "read" => vec![ArgSpec::Number(NumberSpec {
            name: "limit".into(),
            min: 1,
            max: 200_000,
        })],
        "write" => vec![ArgSpec::Text {
            name: "content".into(),
            max_chars: 4000,
        }],
        "execute" => vec![
            ArgSpec::Text {
                name: "cmd".into(),
                max_chars: 400,
            },
            ArgSpec::Flag {
                name: "confirm".into(),
            },
        ],
        _ => Vec::new(),
    }
}

/// One verb and the objects it is legal on, right now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerbBranch {
    pub verb: String,
    pub capability: String,
    pub risk: String,
    pub targets: Vec<ObjectRef>,
    pub args: Vec<ArgSpec>,
    /// `permitted` is a property of the actor, `possible` of the world. They are
    /// reported apart: an action can be possible and not permitted.
    pub permitted: bool,
}

/// The action language of one world slice: exactly the sentences the model is
/// allowed to produce, compiled from state rather than written in a prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionLanguage {
    pub branches: Vec<VerbBranch>,
}

impl ActionLanguage {
    /// The verbs in the language, in stable order.
    pub fn verbs(&self) -> Vec<String> {
        self.branches.iter().map(|b| b.verb.clone()).collect()
    }

    pub fn targets(&self, verb: &str) -> &[ObjectRef] {
        self.branches
            .iter()
            .find(|b| b.verb == verb)
            .map(|b| b.targets.as_slice())
            .unwrap_or(&[])
    }

    pub fn branch(&self, verb: &str) -> Option<&VerbBranch> {
        self.branches.iter().find(|b| b.verb == verb)
    }

    /// Argument fields over the whole language, deduped by name in first-seen
    /// order, so the constraint is deterministic.
    fn arguments(&self) -> Vec<ArgSpec> {
        let mut out: Vec<ArgSpec> = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for branch in &self.branches {
            for arg in &branch.args {
                if seen.insert(arg.name()) {
                    out.push(arg.clone());
                }
            }
        }
        out
    }
}

impl ActionLanguage {
    /// Compile the language into the constraint IR: a verb choice, the union of
    /// legal targets, and the union of argument fields. The per-verb relation
    /// stays in `targets_by_verb` because no flat schema can carry it.
    pub fn constraint(&self) -> Constraint {
        let verbs: Vec<ChoiceValue> = self
            .branches
            .iter()
            .map(|b| {
                ChoiceValue::new(
                    b.verb.clone(),
                    format!("{} via {} ({})", b.verb, b.capability, b.risk),
                )
            })
            .collect();

        let mut targets: Vec<ObjectRef> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for branch in &self.branches {
            for target in &branch.targets {
                if seen.insert(target.handle.clone()) {
                    targets.push(target.clone());
                }
            }
        }

        let mut fields = vec![
            Field {
                name: "verb".into(),
                constraint: Constraint::Choice(ChoiceSet {
                    name: "Verb".into(),
                    values: verbs,
                }),
                required: true,
            },
            Field {
                name: "target".into(),
                constraint: Constraint::ObjectSet(ObjectSet {
                    name: "Target".into(),
                    objects: targets,
                }),
                required: true,
            },
        ];
        for arg in self.arguments() {
            let name = arg.name();
            let constraint = match arg {
                ArgSpec::Text { max_chars, .. } => Constraint::FreeText(FreeField {
                    name: name.clone(),
                    max_chars,
                    placeholder: Some("free text".into()),
                }),
                ArgSpec::Mode { values, .. } => Constraint::Choice(ChoiceSet {
                    name: name.clone(),
                    values: values
                        .iter()
                        .map(|v| ChoiceValue::new(v.clone(), v.clone()))
                        .collect(),
                }),
                ArgSpec::Flag { .. } => Constraint::Choice(ChoiceSet {
                    name: name.clone(),
                    values: vec![
                        ChoiceValue::new("true", "set"),
                        ChoiceValue::new("false", "unset"),
                    ],
                }),
                ArgSpec::Number(spec) => Constraint::FreeText(FreeField {
                    name: name.clone(),
                    max_chars: 12,
                    placeholder: Some(format!("integer {}..{}", spec.min, spec.max)),
                }),
            };
            fields.push(Field {
                name,
                constraint,
                required: false,
            });
        }
        Constraint::Sequence(fields)
    }
}

/// The world as the constraint layer sees it: registered objects plus the
/// capabilities that match their kinds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldView {
    pub objects: Vec<ObjectRef>,
    pub affordances: Vec<Affordance>,
}

impl WorldView {
    /// Build from the registry that already exists. No new object model: these
    /// are the objects and capabilities `action list` prints.
    pub fn from_registry(reg: &Registry) -> Self {
        let objects: Vec<ObjectRef> = reg
            .list_objects()
            .into_iter()
            .map(|(oid, kind, path)| {
                let label = reg
                    .object(&oid)
                    .and_then(|o| o.meta.get("label"))
                    .and_then(|v| v.as_str())
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| {
                        if path.is_empty() {
                            oid.clone()
                        } else {
                            path.clone()
                        }
                    });
                let state = reg
                    .object(&oid)
                    .and_then(|o| o.meta.get("state"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                ObjectRef::from_world(oid, kind, path, label, state)
            })
            .collect();

        let mut affordances: Vec<Affordance> = Vec::new();
        for (name, verbs, risk, tier) in reg.list_capabilities() {
            let Some(cap) = reg.get_capability(&name) else {
                continue;
            };
            let p50 = cap.cost.get("latency_p50_s").copied();
            for verb in verbs {
                affordances.push(Affordance {
                    verb,
                    capability: name.clone(),
                    object_kinds: cap.object_kinds.clone(),
                    risk: risk.clone(),
                    tier: tier.clone(),
                    latency_p50_s: p50,
                });
            }
        }
        affordances.sort_by(|a, b| a.verb.cmp(&b.verb).then(a.capability.cmp(&b.capability)));
        Self {
            objects,
            affordances,
        }
    }

    /// Objects of one kind, in listing order.
    pub fn objects_of_kind(&self, kind: &str) -> Vec<&ObjectRef> {
        self.objects.iter().filter(|o| o.kind == kind).collect()
    }

    /// Resolve an operator-facing string to one world object: exact handle
    /// first, then exact path, then a unique label/path substring. An ambiguous
    /// string resolves to `None` — a focus the world cannot pin is not a focus.
    pub fn resolve(&self, needle: &str) -> Option<ObjectRef> {
        let needle = needle.trim();
        if needle.is_empty() {
            return None;
        }
        if let Some(exact) = self.objects.iter().find(|o| o.handle.as_str() == needle) {
            return Some(exact.clone());
        }
        if let Some(exact) = self.objects.iter().find(|o| o.path == needle) {
            return Some(exact.clone());
        }
        let lower = needle.to_lowercase();
        let mut hits = self.objects.iter().filter(|o| {
            o.path.to_lowercase().contains(&lower) || o.label.to_lowercase().contains(&lower)
        });
        let first = hits.next()?.clone();
        if hits.next().is_some() {
            return None;
        }
        Some(first)
    }

    /// Affordances whose capability covers this kind.
    pub fn affordances_for_kind(&self, kind: &str) -> Vec<&Affordance> {
        self.affordances
            .iter()
            .filter(|a| a.object_kinds.iter().any(|k| k == kind))
            .collect()
    }

    /// The action language: one branch per (verb, capability) the access level
    /// permits, carrying the objects it is legal on. A verb with no legal target
    /// is not a branch — an empty alternative is still an alternative, and it
    /// must not exist.
    pub fn language(&self, access: Access) -> ActionLanguage {
        let mut branches: Vec<VerbBranch> = Vec::new();
        for affordance in &self.affordances {
            if !access.allows(&affordance.verb) {
                continue;
            }
            let targets: Vec<ObjectRef> = self
                .objects
                .iter()
                .filter(|o| affordance.object_kinds.iter().any(|k| k == &o.kind))
                .filter(|o| verb_allowed_in_state(&affordance.verb, o.state.as_deref()))
                .cloned()
                .collect();
            if targets.is_empty() {
                continue;
            }
            branches.push(VerbBranch {
                verb: affordance.verb.clone(),
                capability: affordance.capability.clone(),
                risk: affordance.risk.clone(),
                targets,
                args: args_for(&affordance.verb),
                permitted: true,
            });
        }
        branches.sort_by(|a, b| a.verb.cmp(&b.verb).then(a.capability.cmp(&b.capability)));
        ActionLanguage { branches }
    }

    /// Deterministic version of this world slice: the same objects and
    /// affordances hash the same, so a stale constraint is visibly stale.
    pub fn version(&self) -> String {
        let mut handles: Vec<String> = self
            .objects
            .iter()
            .map(|o| format!("{}:{}", o.handle, o.kind))
            .collect();
        handles.sort();
        handles.dedup();
        let mut aff: Vec<String> = self
            .affordances
            .iter()
            .map(|a| format!("{}:{}", a.verb, a.capability))
            .collect();
        aff.sort();
        aff.dedup();
        crate::mvp::log::hash_canonical(&[handles.join("\n"), aff.join("\n")])
    }
}
