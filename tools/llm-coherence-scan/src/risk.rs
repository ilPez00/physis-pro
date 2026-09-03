use crate::engine::PhysisSignals;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RiskBreakdown {
    pub semantic: f32,
    pub structural: f32,
    pub topical: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RiskScore {
    pub total: f32,
    pub breakdown: RiskBreakdown,
}

const W_SEM: f32 = 0.5;
const W_STR: f32 = 0.3;
const W_TOP: f32 = 0.2;

pub fn coherence_risk(s: &PhysisSignals) -> RiskScore {
    let total = (W_SEM * s.semantic + W_STR * s.structural + W_TOP * s.topical)
        .clamp(0.0, 1.0);
    RiskScore {
        total,
        breakdown: RiskBreakdown {
            semantic: s.semantic,
            structural: s.structural,
            topical: s.topical,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one() {
        assert!((W_SEM + W_STR + W_TOP - 1.0).abs() < 1e-6);
    }

    #[test]
    fn breakdown_is_bounded() {
        let s = PhysisSignals { semantic: 0.3, structural: 0.2, topical: 0.1 };
        let r = coherence_risk(&s);
        assert!((0.0..=1.0).contains(&r.total));
    }
}
