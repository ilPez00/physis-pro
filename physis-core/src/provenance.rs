//! Provenance tracking — traceable chains of evidence and reasoning.
//!
//! Every important assertion should answer:
//!   *Who said this?*
//!   *When?*
//!   *Based on what?*
//!   *Why do we believe it?*

use serde::{Deserialize, Serialize};

use crate::models::Score;

/// A single link in a provenance chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceLink {
    /// What this link asserts.
    pub claim: String,
    /// Source of the claim (file, observation, operator, sensor, etc.).
    pub source: String,
    /// When the claim was made or observed.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Confidence in this specific claim (0.0–1.0).
    pub confidence: Score,
    /// Optional reference to the raw data (path, id, URL).
    #[serde(default)]
    pub raw_reference: Option<String>,
    /// Free-text justification or method.
    #[serde(default)]
    pub method: Option<String>,
}

impl ProvenanceLink {
    pub fn new(claim: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            claim: claim.into(),
            source: source.into(),
            timestamp: chrono::Utc::now(),
            confidence: 1.0,
            raw_reference: None,
            method: None,
        }
    }
}

/// A complete provenance chain — why Physis believes something.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvenanceChain {
    /// Ordered chain of evidence/reasoning steps.
    pub links: Vec<ProvenanceLink>,
}

impl ProvenanceChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_link(&mut self, link: ProvenanceLink) {
        self.links.push(link);
    }

    /// Short summary for display.
    pub fn summary(&self) -> String {
        if self.links.is_empty() {
            return "No provenance recorded".to_string();
        }
        let last = &self.links[self.links.len() - 1];
        format!("{} (via {})", last.claim, last.source)
    }
}
