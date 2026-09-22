//! Validator → adapter (PH-101 Task 10).
//!
//! The allowlist is the whole policy: [`ADAPTER_ALLOWLIST`] names the
//! read-only adapter verbs this module may emit. [`Intent::Execute`] and
//! [`Intent::Unknown`] are refused with a reason — execution authority lives
//! in the world's permission layer, and an unparsed verb is not a license to
//! guess. There is deliberately no shell adapter in this file.

use physis_core::cortex::router::Intent;

use super::CortexIntent;

/// Adapter verbs the cortex may emit. Read-only by construction; kept as a
/// single list so adding a verb is a reviewable one-line diff.
pub const ADAPTER_ALLOWLIST: &[&str] = &["fs.read", "inspect", "query", "embed"];

/// A validated adapter call: the only executable output of this module.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AdapterCall {
    pub adapter: String,
    pub args: Vec<String>,
    pub intent: Intent,
}

/// Why validation refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub reason: String,
}

/// Validate a compiled intent into an adapter call or a refusal.
pub fn validate(intent: &CortexIntent, args: Vec<String>) -> Result<AdapterCall, Refusal> {
    let adapter = match intent.intent {
        Intent::Embed => "embed",
        Intent::Inspect => "inspect",
        Intent::Plan => "query",
        Intent::Execute => {
            return Err(Refusal {
                reason: "execute verbs need the world's permission layer, not the cortex".to_string(),
            })
        }
        Intent::Unknown => {
            return Err(Refusal {
                reason: "unparsed verb: refusing rather than guessing an adapter".to_string(),
            })
        }
    };
    if !ADAPTER_ALLOWLIST.contains(&adapter) {
        return Err(Refusal {
            reason: format!("adapter '{adapter}' is not on the allowlist"),
        });
    }
    Ok(AdapterCall {
        adapter: adapter.to_string(),
        args,
        intent: intent.intent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cortex::compile;

    #[test]
    fn inspect_validates_to_its_adapter() {
        let intent = compile("read the file", "read", "cpu");
        let call = validate(&intent, vec!["notes.md".to_string()]).unwrap();
        assert_eq!(call.adapter, "inspect");
        assert!(ADAPTER_ALLOWLIST.contains(&call.adapter.as_str()));
    }

    #[test]
    fn execute_and_unknown_are_refused() {
        let exec = compile("delete the file", "execute", "cpu");
        assert!(validate(&exec, vec![]).is_err());
        let unknown = compile("frobnicate the file", "frobnicate", "cpu");
        let refusal = validate(&unknown, vec![]).unwrap_err();
        assert!(refusal.reason.contains("refusing rather than guessing"));
    }

    #[test]
    fn no_shell_adapter_exists() {
        assert!(!ADAPTER_ALLOWLIST.iter().any(|a| a.contains("shell") || a.contains("exec")));
    }
}
