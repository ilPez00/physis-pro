//! What a model says about a minted block, and nothing more.
//!
//! A minted block has no inherited history, so the only thing that can be said
//! about it beyond "it appeared here" is what a model makes of its text. That
//! is recorded as an [`Observation`]: attributed, invalidatable, and never
//! consulted when deciding identity.
//!
//! Three rules hold this honest:
//!
//! 1. **Attribution.** Every observation names the embedder that produced it.
//!    Re-annotating with a different model replaces observations; it cannot
//!    touch an id, because ids never came from here.
//! 2. **Scope.** `isolated` says "unlike anything else in this document". It
//!    does not say "unknown", because the corpus this store can see is one
//!    file, and a claim about the world needs a corpus that is actually here.
//! 3. **No authority.** An observation is rendered and may be offered as a
//!    candidate. It never auto-propagates, never re-anchors a link, and never
//!    satisfies a patch check.

use super::BlockId;
use serde::{Deserialize, Serialize};

/// Cosine below which two blocks are not considered neighbours. A document
/// neighbourhood is a small sample, so this is a display floor, not a
/// calibration — it decides what is worth showing, not what is true.
pub const NEIGHBOUR_FLOOR: f32 = 0.60;

/// How many neighbours are kept.
pub const NEIGHBOURS: usize = 5;

/// One model's reading of one block, at one revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub block_id: BlockId,
    pub revision: u64,
    /// The block's content hash when this was read. The invalidation key: an
    /// observation describes particular bytes, so a block whose bytes changed
    /// makes its observation stale without any comparison of meaning.
    #[serde(default)]
    pub content_hash: u64,
    /// Which model produced this, so the record can be invalidated rather than
    /// mistaken for a fact.
    pub embedder: String,
    /// Best DOMAIN×MODE cell, when a classifier was available.
    #[serde(default)]
    pub cell: Option<(String, String)>,
    /// The classifier's score for that cell. On short inputs these compress
    /// (leading cells land within a few hundredths), so read the order, not
    /// the magnitude.
    #[serde(default)]
    pub score: f32,
    /// The cell's member entries that scored highest for this text.
    #[serde(default)]
    pub entries: Vec<String>,
    /// Nearest other blocks in this document, best first.
    #[serde(default)]
    pub neighbours: Vec<(BlockId, f32)>,
    /// No neighbour above [`NEIGHBOUR_FLOOR`]: this block is unlike the rest of
    /// the document, which is the document-scoped form of "diverges from the
    /// representation map".
    #[serde(default)]
    pub isolated: bool,
    /// The block's vector, rounded for storage. Kept so a corpus query is a
    /// cosine over what has already been embedded rather than a re-embed of
    /// every block in every document. Empty when the embedder declined.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector: Vec<f32>,
}

/// Round a vector for storage. Four decimals is far below the precision at
/// which cosine rankings change, and the difference between a 12 KB
/// observations file and a 60 KB one.
pub fn round_vector(v: &[f32]) -> Vec<f32> {
    v.iter().map(|x| (x * 10_000.0).round() / 10_000.0).collect()
}

impl Observation {
    /// One line for a log or a panel.
    pub fn summary(&self) -> String {
        let cell = match &self.cell {
            Some((d, m)) => format!("{d}×{m} {:.2}", self.score),
            None => "unclassified".to_string(),
        };
        format!(
            "{} · {} neighbour(s){} · {}",
            cell,
            self.neighbours.len(),
            if self.isolated { " · isolated in this document" } else { "" },
            self.embedder
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_observation_round_trips_and_names_its_model() {
        let o = Observation {
            block_id: "b1".into(),
            revision: 3,
            content_hash: 42,
            embedder: "minilm".into(),
            cell: Some(("MEASURE".into(), "OPERATE".into())),
            score: 0.87,
            entries: vec!["budget".into()],
            neighbours: vec![("b2".into(), 0.71)],
            isolated: false,
            vector: round_vector(&[0.5, -0.25]),
        };
        let json = serde_json::to_string(&o).unwrap();
        let back: Observation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.embedder, "minilm");
        assert_eq!(back.cell.as_ref().unwrap().1, "OPERATE");
        assert!(back.summary().contains("minilm"));
        assert!(back.summary().contains("MEASURE×OPERATE"));
    }

    #[test]
    fn a_rounded_vector_keeps_its_ranking() {
        // The rounding is for storage size, so it must not reorder anything a
        // cosine would have ranked differently at full precision.
        let a: Vec<f32> = (0..64).map(|i| ((i as f32) * 0.37).sin()).collect();
        let b: Vec<f32> = (0..64).map(|i| ((i as f32) * 0.37 + 0.1).sin()).collect();
        let c: Vec<f32> = (0..64).map(|i| ((i as f32) * 0.37 + 1.5).sin()).collect();
        let cos = |x: &[f32], y: &[f32]| -> f32 {
            x.iter().zip(y).map(|(a, b)| a * b).sum::<f32>()
                / (x.iter().map(|v| v * v).sum::<f32>().sqrt()
                    * y.iter().map(|v| v * v).sum::<f32>().sqrt())
        };
        let (ra, rb, rc) = (round_vector(&a), round_vector(&b), round_vector(&c));
        assert!(
            (cos(&a, &b) > cos(&a, &c)) == (cos(&ra, &rb) > cos(&ra, &rc)),
            "rounding must not flip a comparison"
        );
    }

    #[test]
    fn an_observation_without_a_classifier_is_still_valid() {
        // The embedder may be present while no classifier is; that is a real
        // state and must not be recorded as a classification of nothing.
        let json = r#"{"block_id":"b","revision":0,"embedder":"random-projection"}"#;
        let o: Observation = serde_json::from_str(json).unwrap();
        assert!(o.cell.is_none());
        assert!(o.neighbours.is_empty());
        assert!(!o.isolated, "absent evidence is not a claim of isolation");
        assert!(o.summary().contains("unclassified"));
    }
}
