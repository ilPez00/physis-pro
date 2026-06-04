use crate::graph::RawNodeKey;
use serde::{Serialize, Deserialize};

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
        }
    }

    /// PDCA: Plan-Do-Check-Act
    pub fn tick(&mut self, state_vector: &[f32]) {
        if state_vector.is_empty() {
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
            log::warn!("Significant ontological drift detected (sim={:.4})", similarity);
            
            // 3. Plan: Generate new directives
            self.active_directives.push(Directive::Synthesize { 
                nodes: vec![] // Future: identify specific outlier nodes
            });
        } else if similarity < 0.9 {
            // Minor drift: Focus on existing nodes to stabilize
            self.active_directives.push(Directive::Focus { 
                node_id: RawNodeKey { data: 0 } // Dummy target
            });
        }

        // 4. Do: (Directives are cleared after being processed by PhysisCore in next tick)
        if self.active_directives.len() > 10 {
            self.active_directives.drain(..5);
        }
    }
}
