use crate::capture::RunRecord;
use crate::drift::DriftScore;
use crate::overlay::OverlayEntry;
use crate::refine::{DiscoveryProposal, DreamReport};
use crate::risk::RiskScore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullReport {
    pub run_id: String,
    pub model: String,
    pub task: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub risk: RiskScore,
    pub drift: DriftScore,
    pub dream: DreamReport,
    pub proposals: Vec<DiscoveryProposal>,
    pub overlays_hit: Vec<OverlayEntry>,
    pub prompt_hash: String,
}

pub fn build(
    rec: &RunRecord,
    risk: RiskScore,
    drift: DriftScore,
    dream: DreamReport,
    proposals: Vec<DiscoveryProposal>,
    overlays_hit: Vec<OverlayEntry>,
    prompt_hash: String,
) -> FullReport {
    FullReport {
        run_id: rec.run_id.unwrap().to_string(),
        model: rec.model.clone(),
        task: rec.task.clone(),
        created_at: rec.created_at.unwrap(),
        risk,
        drift,
        dream,
        proposals,
        overlays_hit,
        prompt_hash,
    }
}

pub fn one_line(r: &FullReport) -> String {
    let drift_str = match r.drift.total {
        Some(d) => format!("{d:.2}"),
        None => "n/a".into(),
    };
    format!(
        "risk={:.2} [sem={:.2} str={:.2} top={:.2}] drift={} [local={} ref={} cross={}] proposals={} dream_silent={}",
        r.risk.total,
        r.risk.breakdown.semantic,
        r.risk.breakdown.structural,
        r.risk.breakdown.topical,
        drift_str,
        opt(&r.drift.components.local),
        opt(&r.drift.components.reference),
        opt(&r.drift.components.cross_model),
        r.proposals.len(),
        r.dream.silent_failures.len(),
    )
}

fn opt(v: &Option<f32>) -> String {
    match v {
        Some(x) => format!("{x:.2}"),
        None => "n/a".into(),
    }
}
