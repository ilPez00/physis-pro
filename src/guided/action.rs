//! The action schema: what a guided generation produces.
//!
//! `target` is an `ObjectHandle`, never a free `String`, so an action that
//! names an object the world does not contain cannot be constructed from typed
//! data at all — and `validate` still checks it, because a handle can also
//! arrive from data that predates this world (a cached model reply, a file).

use std::fmt;

use super::affordance::ObjectHandle;
use super::constraint::GenerationContext;

/// Arguments, one variant per shape the executors accept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GuidedArgs {
    #[default]
    None,
    /// `key=value` pairs, already typed.
    Named(Vec<(String, String)>),
}

impl GuidedArgs {
    /// Render as the `key=value` tail the action line parser reads.
    pub fn action_tail(&self) -> String {
        match self {
            GuidedArgs::None => String::new(),
            GuidedArgs::Named(pairs) => pairs
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

/// One permitted transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuidedAction {
    pub verb: String,
    pub target: ObjectHandle,
    pub args: GuidedArgs,
}

impl GuidedAction {
    pub fn new(verb: impl Into<String>, target: ObjectHandle, args: GuidedArgs) -> Self {
        Self {
            verb: verb.into(),
            target,
            args,
        }
    }

    /// The line the existing action surface dispatches.
    pub fn action_line(&self) -> String {
        let tail = self.args.action_tail();
        if tail.is_empty() {
            format!("{} {}", self.verb, self.target)
        } else {
            format!("{} {} {}", self.verb, self.target, tail)
        }
    }

    /// Check the action against the constraint it was generated under. This is
    /// the second line of the defence — the backend is the first, and a backend
    /// that lies (or a reply from an older world) lands here.
    pub fn validate(&self, ctx: &GenerationContext) -> Result<(), Rejection> {
        let targets = ctx.targets(&self.verb);
        if targets.is_empty() {
            return Err(Rejection::VerbNotInLanguage {
                verb: self.verb.clone(),
            });
        }
        if !targets.iter().any(|t| t.handle == self.target) {
            return Err(Rejection::TargetNotVisible {
                verb: self.verb.clone(),
                target: self.target.to_string(),
            });
        }
        Ok(())
    }
}

/// Why an action is not in the language. Enumerated, so a caller can report the
/// reason instead of a boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The verb is not afforded by anything in this world slice (including: the
    /// access level removed it).
    VerbNotInLanguage { verb: String },
    /// The verb exists but not for this object.
    TargetNotVisible { verb: String, target: String },
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::VerbNotInLanguage { verb } => write!(
                f,
                "rejected: `{verb}` is not in this world's action language \
                 (not afforded, or not permitted at this access level)"
            ),
            Rejection::TargetNotVisible { verb, target } => write!(
                f,
                "rejected: `{target}` is not a legal target for `{verb}` in this world slice"
            ),
        }
    }
}

impl std::error::Error for Rejection {}
