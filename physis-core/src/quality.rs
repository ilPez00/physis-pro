//! Ontology-driven feedback loop — when the user says "this is bad/wrong",
//! capture the negative signal through the semiotic grid, store it as a
//! vector-space negative example, and bias future classification away from it.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::embed::VectorEmbed;
use crate::models::cosine_sim;

/// A recorded quality failure — what was wrong, where in the grid it landed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityFailure {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub feedback: String,
    pub feedback_embedding: Vec<f32>,
    /// The DOMAIN×MODE cell that was the top hit ("DOMAIN\x00MODE").
    pub top_cell: String,
    pub top_score: f32,
    pub correct_domain: Option<String>,
    pub correct_embedding: Option<Vec<f32>>,
    pub severity: f32,
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
    pub failures: Vec<QualityFailure>,
    /// Per-cell negative weight (cell_key → penalty 0.0–1.0).
    pub cell_penalties: HashMap<String, f32>,
    /// Per-cell positive weight (corrections).
    pub cell_boosts: HashMap<String, f32>,
    embedder: Box<dyn VectorEmbed>,
}

impl QualityTracker {
    pub fn new(embedder: Box<dyn VectorEmbed>) -> Self {
        Self {
            failures: Vec::new(),
            cell_penalties: HashMap::new(),
            cell_boosts: HashMap::new(),
            embedder,
        }
    }

    /// Report a quality failure — analyze feedback through the grid, record it,
    /// and adjust cell penalties.
    pub fn report_failure(
        &mut self,
        feedback: &str,
        cell_centroids: &HashMap<String, Vec<f32>>,
        correct_domain: Option<&str>,
    ) -> &QualityFailure {
        let embedding = self.embedder.embed(feedback);

        let mut scored: Vec<(&str, f32)> = cell_centroids
            .iter()
            .map(|(key, centroid)| (key.as_str(), cosine_sim(&embedding, centroid)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (top_cell, top_score) = scored.first().map(|(k, s)| (*k, *s)).unwrap_or(("UNKNOWN", 0.0));

        let severity = if top_score < 0.3 {
            0.8
        } else if top_score < 0.6 {
            0.5
        } else {
            0.3
        };

        let mut failure = QualityFailure::new(feedback, embedding, top_cell, top_score);

        if let Some(correct) = correct_domain {
            failure.correct_domain = Some(correct.to_string());
            let correct_emb = self.embedder.embed(correct);
            for (key, centroid) in cell_centroids {
                if cosine_sim(&correct_emb, centroid) > 0.6 {
                    let boost = self.cell_boosts.entry(key.clone()).or_insert(0.0);
                    *boost = (*boost + 0.1).min(1.0);
                }
            }
        }

        failure.severity = severity;
        self.failures.push(failure);

        let penalty = self.cell_penalties.entry(top_cell.to_string()).or_insert(0.0);
        *penalty = (*penalty + severity * 0.2).min(1.0);

        // Same cell flagged 3+ times → lock it.
        let same_cell_count = self.failures.iter().filter(|f| f.top_cell == top_cell).count();
        if same_cell_count >= 3 {
            *penalty = 1.0;
        }

        self.failures.last().unwrap()
    }

    /// Report positive feedback — user confirmed the output is good.
    pub fn report_success(&mut self, cell: &str) {
        let boost = self.cell_boosts.entry(cell.to_string()).or_insert(0.0);
        *boost = (*boost + 0.15).min(1.0);
        if let Some(p) = self.cell_penalties.get_mut(cell) {
            *p = (*p - 0.1).max(0.0);
        }
    }

    /// Adjust a raw score by learned penalties and boosts.
    pub fn adjust_score(&self, cell: &str, raw_score: f32) -> f32 {
        let penalty = self.cell_penalties.get(cell).copied().unwrap_or(0.0);
        let boost = self.cell_boosts.get(cell).copied().unwrap_or(0.0);
        let net = 1.0 - penalty + boost;
        (raw_score * net).clamp(0.0, 1.0)
    }

    /// Human-readable summary (used by the CLI).
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
                &f.id[..8],
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
        Ok(serde_json::from_str(&contents)?)
    }

    /// Load from disk, rebuilding penalties from the recorded failures.
    pub fn load_or_new(path: &Path) -> Self {
        let mut t = QualityTracker::new(Box::new(crate::embed::RandomProjectionEmbedder::new(384)));
        let failures = if path.exists() {
            Self::load(path).unwrap_or_default()
        } else {
            Vec::new()
        };
        for f in &failures {
            let penalty = t.cell_penalties.entry(f.top_cell.clone()).or_insert(0.0);
            *penalty = (*penalty + f.severity * 0.2).min(1.0);
        }
        t.failures = failures;
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn make_tracker() -> QualityTracker {
        QualityTracker::new(Box::new(RandomProjectionEmbedder::new(64)))
    }

    #[test]
    fn failure_creates_record_and_penalty() {
        let mut t = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00LIFT".to_string(), vec![0.5; 64]);
        let (top_cell, top_score) = {
            let f = t.report_failure("this classification is completely wrong", &centroids, None);
            (f.top_cell.clone(), f.top_score)
        };
        assert_eq!(t.failures.len(), 1);
        assert_eq!(top_cell, "WORK\x00LIFT");
        assert!(top_score.is_finite());
        assert!(*t.cell_penalties.get("WORK\x00LIFT").unwrap() > 0.0);
    }

    #[test]
    fn adjust_score_with_penalty_reduces() {
        let mut t = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00LIFT".to_string(), vec![0.5; 64]);
        t.report_failure("wrong", &centroids, None);
        assert!(t.adjust_score("WORK\x00LIFT", 0.8) < 0.8);
    }

    #[test]
    fn adjust_score_with_boost_increases() {
        let mut t = make_tracker();
        t.cell_boosts.insert("WORK\x00LIFT".to_string(), 0.5);
        assert!(t.adjust_score("WORK\x00LIFT", 0.8) > 0.8);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let mut t = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("TEST\x00CELL".to_string(), vec![0.5; 64]);
        t.report_failure("test failure", &centroids, None);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("failures.json");
        t.save(&path).unwrap();
        let loaded = QualityTracker::load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].feedback, "test failure");
    }
}
