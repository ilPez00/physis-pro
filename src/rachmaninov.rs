use crate::graph::RawNodeKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Directive {
    Focus { node_id: RawNodeKey },
    Expand { node_id: RawNodeKey },
    Prune { node_id: RawNodeKey },
    Synthesize { nodes: Vec<RawNodeKey> },
}

pub struct RachmaninovHolon {
    pub current_goal_vector: Vec<f32>,
    pub active_directives: Vec<Directive>,
    /// Ticks since the last drift WARN was emitted. The holarchy ticks every
    /// ~10s; an empty/quiet graph compares identical zero-vectors at sim=0.0
    /// and used to spam a WARN each tick. Rate-limit to one warning per 90
    /// ticks (~15 min) unless the similarity actually changes.
    ticks_since_last_drift_warn: u32,
    /// Similarity value the last drift WARN reported (dedupe key).
    last_warned_sim: Option<f32>,
}

impl Default for RachmaninovHolon {
    fn default() -> Self {
        Self::new()
    }
}

impl RachmaninovHolon {
    pub fn new() -> Self {
        Self {
            current_goal_vector: vec![],
            active_directives: vec![],
            ticks_since_last_drift_warn: 0,
            last_warned_sim: None,
        }
    }

    /// PDCA: Plan-Do-Check-Act
    pub fn tick(&mut self, state_vector: &[f32]) {
        if state_vector.is_empty() {
            return;
        }

        // QA 8.6: a zero context vector (empty/near-empty holarchy) carries no
        // signal — calling it "drift" is pure noise. Ignore until real content
        // produces a non-degenerate vector.
        let norm: f32 = state_vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-6 {
            return;
        }

        // 1. Check: Compare state_vector to current_goal_vector
        if self.current_goal_vector.is_empty() {
            self.current_goal_vector = state_vector.to_vec();
            return;
        }

        let similarity = crate::models::cosine_sim(&self.current_goal_vector, state_vector);

        // 2. Act: Adjust internal state based on coherence drift
        if similarity < 0.7 {
            // Rate-limit the WARN: only log when the similarity CHANGED
            // meaningfully since the last warning, or 90 quiet ticks (~15 min)
            // have passed. A static graph sits at one similarity forever and
            // used to emit an identical WARN every tick.
            let changed = match self.last_warned_sim {
                Some(prev) => (prev - similarity).abs() > 0.05,
                None => true,
            };
            if changed || self.ticks_since_last_drift_warn >= 90 {
                log::warn!(
                    "Significant ontological drift detected (sim={:.4})",
                    similarity
                );
                self.last_warned_sim = Some(similarity);
                self.ticks_since_last_drift_warn = 0;
            } else {
                self.ticks_since_last_drift_warn += 1;
            }

            // 3. Plan: Generate new directives
            self.active_directives.push(Directive::Synthesize {
                nodes: vec![], // Future: identify specific outlier nodes
            });
        } else if similarity < 0.9 {
            // Minor drift: Focus on existing nodes to stabilize
            self.active_directives.push(Directive::Focus {
                node_id: RawNodeKey { data: 0 }, // Dummy target
            });
        }

        // 4. Do: (Directives are cleared after being processed by PhysisCore in next tick)
        if self.active_directives.len() > 10 {
            self.active_directives.drain(..5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression (QA 8.6): an empty holarchy feeds zero vectors; those must
    /// be ignored entirely — no goal adoption, no directives, no drift WARNs.
    #[test]
    fn zero_signal_is_ignored_not_drift() {
        let mut h = RachmaninovHolon::new();
        for _ in 0..200 {
            h.tick(&[0.0; 8]);
        }
        assert!(
            h.current_goal_vector.is_empty(),
            "zero vector is not a goal"
        );
        assert!(h.active_directives.is_empty(), "no directives from noise");
    }

    /// Real signal still works: first non-zero vector becomes the baseline.
    #[test]
    fn non_zero_signal_adopts_baseline() {
        let mut h = RachmaninovHolon::new();
        h.tick(&[1.0, 0.0, 0.0]);
        assert_eq!(h.current_goal_vector, vec![1.0, 0.0, 0.0]);
    }
}
