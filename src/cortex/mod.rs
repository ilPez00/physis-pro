//! Pro-side cortex (PH-101 Task 10): intent with provenance, validated to an
//! adapter call — never model → shell.
//!
//! The pipeline is `compile → validate → adapter`. [`compile`] parses the
//! verb with the core deterministic table (no model); [`validate`] admits
//! only the adapter allowlist and refuses everything else, including
//! [`Intent::Execute`](physis_core::cortex::router::Intent::Execute), whose
//! authority lives in the world's permission layer, not here. A validated
//! [`AdapterCall`] names a read-only adapter and its args; there is no path
//! from this module to a shell.

pub mod validate;

use serde::{Deserialize, Serialize};

use physis_core::cortex::router::Intent;

/// Where an intent came from. A model guess and a deterministic parse must
/// never be interchangeable downstream, so the distinction is structural.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Who compiled this: `"exact"` for the deterministic verb table, or a
    /// model id for anything a model touched.
    pub compiled_by: String,
    pub backend_label: String,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// An intent with its provenance attached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CortexIntent {
    pub intent: Intent,
    pub utterance: String,
    pub provenance: Provenance,
}

/// Compile an utterance's verb deterministically. Model-compiled intents
/// arrive via [`CortexIntent`] construction with their own `compiled_by`,
/// never through this function.
pub fn compile(utterance: &str, verb: &str, backend_label: &str) -> CortexIntent {
    CortexIntent {
        intent: Intent::parse(verb),
        utterance: utterance.to_string(),
        provenance: Provenance {
            compiled_by: "exact".to_string(),
            backend_label: backend_label.to_string(),
            at: chrono::Utc::now(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_marks_deterministic_provenance() {
        let c = compile("read the file", "read", "cpu:random-projection:384");
        assert_eq!(c.intent, Intent::Inspect);
        assert_eq!(c.provenance.compiled_by, "exact");
    }
}
