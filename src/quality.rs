//! Ontology-driven feedback loop — when user says "this is bad/wrong",
//! capture the negative signal through semiotic grid analysis + ontology mapping,
//! store as vector-space negative examples to bias future generations.
//!
//! # Flow
//! 1. User provides negative feedback (text + optional domain/mode context)
//! 2. Feedback is embedded against the semiotic grid → identifies which DOMAIN×MODE cells fired
//! 3. A negative Experience is created in the PDCA actor with grade=0.0
//! 4. A Refutation is stored linking the bad pattern to the offending semiotic coordinates
//! 5. Future classify/dream calls penalize vectors nearby the negative examples

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{OntologyLoader, PhysisProConfig};
use crate::embed::VectorEmbed;
use crate::models::*;
use crate::trie::DynamicVectorTrie;

/// A recorded quality failure — what was wrong, where in the semiotic grid it landed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityFailure {
    /// Unique ID for this failure record.
    pub id: String,
    /// When the failure was reported.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// The raw user feedback text (e.g. "this classification is wrong, it's not a machine domain").
    pub feedback: String,
    /// Embedding of the feedback text.
    pub feedback_embedding: Vec<f32>,
    /// The DOMAIN×MODE cell that was the top hit (where the wrong classification landed).
    pub top_cell: String,
    /// Score of that top cell.
    pub top_score: f32,
    /// The user's stated or implied correct domain (optional).
    pub correct_domain: Option<String>,
    /// Embedding of the correct domain (for shift measurement).
    pub correct_embedding: Option<Vec<f32>>,
    /// Severity: 0.0 (minor) to 1.0 (critical).
    pub severity: f32,
    /// Whether this failure has been "closed" by adjusting weights.
    pub resolved: bool,
}

impl QualityFailure {
    pub fn new(feedback: &str, embedding: Vec<f32>, top_cell: &str, top_score: f32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            feedback: feedback.to_string(),
            feedback_embedding: embedding,
            top_cell: top_cell.to_string(),
            top_score,
            correct_domain: None,
            correct_embedding: None,
            severity: 0.5,
            resolved: false,
        }
    }
}

/// Tracks quality feedback and applies penalty/gain to semiotic cells.
pub struct QualityTracker {
    /// All recorded failures.
    pub failures: Vec<QualityFailure>,
    /// Per-cell negative weight accumulation (cell_key → penalty multiplier, 0.0–1.0).
    /// Higher penalty = cell is less likely to be the top result.
    pub cell_penalties: HashMap<String, f32>,
    /// Per-cell positive weight accumulation (corrections).
    pub cell_boosts: HashMap<String, f32>,
    /// Embedder for classifying feedback.
    embedder: Box<dyn VectorEmbed>,
    /// Ontology loader for semiotic grid lookup.
    ontology: OntologyLoader,
}

impl QualityTracker {
    pub fn new(embedder: Box<dyn VectorEmbed>, ontology: OntologyLoader) -> Self {
        Self {
            failures: Vec::new(),
            cell_penalties: HashMap::new(),
            cell_boosts: HashMap::new(),
            embedder,
            ontology,
        }
    }

    /// Report a quality failure — analyzes feedback through the semiotic grid,
    /// creates a QualityFailure record, and adjusts cell penalties.
    pub fn report_failure(
        &mut self,
        feedback: &str,
        cell_centroids: &HashMap<String, Vec<f32>>,
        correct_domain: Option<&str>,
    ) -> anyhow::Result<&QualityFailure> {
        let embedding = self.embedder.embed(feedback);

        // Find the top semiotic cell this feedback maps to
        let mut scored: Vec<(&str, f32)> = cell_centroids
            .iter()
            .map(|(key, centroid)| (key.as_str(), cosine_sim(&embedding, centroid)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (top_cell, top_score) = scored.first().map(|(k, s)| (*k, *s)).unwrap_or(("UNKNOWN", 0.0));

        // Determine severity based on how far off the feedback was
        let severity = if top_score < 0.3 {
            0.8 // Way off — high severity
        } else if top_score < 0.6 {
            0.5 // Moderate misalignment
        } else {
            0.3 // Close but wrong
        };

        let mut failure = QualityFailure::new(feedback, embedding.clone(), top_cell, top_score);

        if let Some(correct) = correct_domain {
            failure.correct_domain = Some(correct.to_string());
            failure.correct_embedding = Some(self.embedder.embed(correct));
            // Boost the correct cell
            if let Some(c_emb) = &failure.correct_embedding {
                for (key, centroid) in cell_centroids {
                    let sim = cosine_sim(c_emb, centroid);
                    if sim > 0.6 {
                        let boost = self.cell_boosts.entry(key.clone()).or_insert(0.0);
                        *boost = (*boost + 0.1).min(1.0);
                    }
                }
            }
        }

        failure.severity = severity;
        self.failures.push(failure);

        // Apply penalty to the top cell
        let penalty = self.cell_penalties.entry(top_cell.to_string()).or_insert(0.0);
        *penalty = (*penalty + severity * 0.2).min(1.0);

        // Force-resolve duplicate penalties — if same cell gets flagged 3+ times, lock it
        let same_cell_count = self.failures.iter().filter(|f| f.top_cell == top_cell).count();
        if same_cell_count >= 3 {
            *penalty = 1.0;
        }

        Ok(self.failures.last().unwrap())
    }

    /// Report positive feedback — user confirmed output is good.
    pub fn report_success(&mut self, cell: &str) {
        let boost = self.cell_boosts.entry(cell.to_string()).or_insert(0.0);
        *boost = (*boost + 0.15).min(1.0);
        // Reduce penalty for this cell
        if let Some(p) = self.cell_penalties.get_mut(cell) {
            *p = (*p - 0.1).max(0.0);
        }
    }

    /// Adjust a score by applying learned penalties and boosts.
    /// Returns the adjusted score.
    pub fn adjust_score(&self, cell: &str, raw_score: f32) -> f32 {
        let penalty = self.cell_penalties.get(cell).copied().unwrap_or(0.0);
        let boost = self.cell_boosts.get(cell).copied().unwrap_or(0.0);
        let net = 1.0 - penalty + boost;
        (raw_score * net).clamp(0.0, 1.0)
    }

    /// Get a summary of all quality failures grouped by cell.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Quality Tracker ===");
        out.push('\n');
        out.push_str(&format!("Total failures: {}\n", self.failures.len()));
        out.push_str(&format!("Cells with penalties: {}\n", self.cell_penalties.len()));
        out.push_str(&format!("Cells with boosts: {}\n", self.cell_boosts.len()));
        out.push('\n');

        if self.failures.is_empty() {
            out.push_str("No failures recorded.\n");
            return out;
        }

        out.push_str("--- Recent Failures ---\n");
        for f in self.failures.iter().rev().take(5) {
            out.push_str(&format!(
                "  [{}] cell={} score={:.3} sev={:.1} feedback={}\n",
                f.id[..8].to_string(),
                f.top_cell,
                f.top_score,
                f.severity,
                &f.feedback[..f.feedback.len().min(60)],
            ));
        }
        out.push('\n');

        out.push_str("--- Cell Penalties ---\n");
        let mut sorted: Vec<_> = self.cell_penalties.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (cell, penalty) in sorted.iter().take(10) {
            out.push_str(&format!("  {}: penalty={:.2}\n", cell, penalty));
        }

        out
    }

    /// Persist failures to a JSON file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.failures)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load failures from a JSON file.
    pub fn load(path: &Path) -> anyhow::Result<Vec<QualityFailure>> {
        let contents = std::fs::read_to_string(path)?;
        let failures: Vec<QualityFailure> = serde_json::from_str(&contents)?;
        Ok(failures)
    }

    /// Load from disk or return empty if file doesn't exist.
    pub fn load_or_new(path: &Path) -> Self {
        let failures = if path.exists() {
            Self::load(path).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut t = QualityTracker::new(
            Box::new(crate::embed::RandomProjectionEmbedder::new(384)),
            crate::config::OntologyLoader::new(),
        );
        for f in &failures {
            let penalty = t.cell_penalties.entry(f.top_cell.clone()).or_insert(0.0);
            *penalty = (*penalty + f.severity * 0.2).min(1.0);
            if let Some(ref correct) = f.correct_domain {
                let c_emb = t.embedder.embed(correct);
                for (key, _centroid) in &t.embedder_cells() {
                    if key.contains(&f.top_cell) {
                        let boost = t.cell_boosts.entry((*key).clone()).or_insert(0.0);
                        *boost = (*boost + 0.1).min(1.0);
                    }
                }
            }
        }
        t.failures = failures;
        t
    }

    fn embedder_cells(&self) -> std::collections::HashMap<String, Vec<f32>> {
        // Return stored cell centroids — this is populated by the CLI at runtime.
        // For loading purposes, we just track cell names from failures.
        std::collections::HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn make_tracker() -> QualityTracker {
        let embedder: Box<dyn VectorEmbed> = Box::new(RandomProjectionEmbedder::new(384));
        let ontology = OntologyLoader::new();
        QualityTracker::new(embedder, ontology)
    }

    #[test]
    fn test_report_failure_creates_record() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00DO".to_string(), vec![0.5; 384]);

        let result = tracker.report_failure("this classification is completely wrong", &centroids, None);
        assert!(result.is_ok());
        assert_eq!(tracker.failures.len(), 1);
    }

    #[test]
    fn test_cell_penalty_accumulates() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00DO".to_string(), vec![0.5; 384]);

        let _ = tracker.report_failure("wrong", &centroids, None);
        assert!(tracker.cell_penalties.contains_key("WORK\x00DO"));
        assert!(*tracker.cell_penalties.get("WORK\x00DO").unwrap() > 0.0);
    }

    #[test]
    fn test_adjust_score_with_penalty() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00DO".to_string(), vec![0.5; 384]);

        let _ = tracker.report_failure("wrong", &centroids, None);
        let adjusted = tracker.adjust_score("WORK\x00DO", 0.8);
        assert!(adjusted < 0.8, "penalty should reduce score");
    }

    #[test]
    fn test_adjust_score_with_boost() {
        let tracker = make_tracker();
        // Can't call report_success on immutable...we can construct the state manually
        let mut t = make_tracker();
        t.cell_boosts.insert("WORK\x00DO".to_string(), 0.5);
        let adjusted = t.adjust_score("WORK\x00DO", 0.8);
        assert!(adjusted > 0.8, "boost should increase score");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("TEST\x00CELL".to_string(), vec![0.5; 384]);
        let _ = tracker.report_failure("test failure", &centroids, None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("failures.json");
        tracker.save(&path).unwrap();

        let loaded = QualityTracker::load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].feedback, "test failure");
    }

    #[test]
    fn test_summary_format() {
        let tracker = make_tracker();
        let summary = tracker.summary();
        assert!(summary.contains("No failures recorded"));
    }
}
