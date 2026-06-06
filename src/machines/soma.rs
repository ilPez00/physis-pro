//! soma — the body machine.
//!
//! Perceive training sessions (load + recovery); reduce to **effort / recovery /
//! overreaching** episodes; elaborate by detecting *drift* (load rising while
//! recovery falls — the slip before injury/burnout). Coherence maps directly:
//! a protocol is Success only if it produced a real effect without overreaching.
//!
//! Pure + testable; `register_*` is the only core touch.

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One session: `load` (training stress, 0–1) and `recovery` (readiness, 0–1).
#[derive(Debug, Clone)]
pub struct Session {
    pub day: u32,
    pub activity: String,
    pub load: f32,
    pub recovery: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// High load, adequate recovery → productive (coherence +1).
    Effort,
    /// Low load → restoring (coherence 0, inert but necessary).
    Recovery,
    /// High load, poor recovery → digging a hole (coherence −1).
    Overreaching,
}

impl Status {
    /// Map to the engine's coherence rating.
    pub fn coherence(self) -> f32 {
        match self {
            Status::Effort => 1.0,
            Status::Recovery => 0.0,
            Status::Overreaching => -1.0,
        }
    }
}

/// Classify a session. High load = ≥ 0.6; adequate recovery = ≥ 0.5.
pub fn classify(s: &Session) -> Status {
    if s.load >= 0.6 {
        if s.recovery >= 0.5 { Status::Effort } else { Status::Overreaching }
    } else {
        Status::Recovery
    }
}

/// Drift score over the last `window` sessions: load trending up while recovery
/// trends down. Positive → drifting toward overreaching. Compares the mean of
/// the first vs second half of the window.
pub fn drift(sessions: &[Session], window: usize) -> f32 {
    let n = sessions.len();
    if n < window || window < 2 {
        return 0.0;
    }
    let slice = &sessions[n - window..];
    let half = window / 2;
    let mean = |xs: &[Session], f: fn(&Session) -> f32| {
        xs.iter().map(f).sum::<f32>() / xs.len().max(1) as f32
    };
    let load_rise = mean(&slice[half..], |s| s.load) - mean(&slice[..half], |s| s.load);
    let rec_fall = mean(&slice[..half], |s| s.recovery) - mean(&slice[half..], |s| s.recovery);
    load_rise + rec_fall
}

/// Adapter: register each session as a behavioural vector (label encodes status)
/// and bridge it into the shared graph. Returns coherence node ids.
pub fn register_sessions(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    sessions: &[Session],
) -> Vec<String> {
    let mut ids = Vec::new();
    for s in sessions {
        let status = classify(s);
        let text = format!("{} day {}: {:?} (load {:.1} rec {:.1})", s.activity, s.day, status, s.load, s.recovery);
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(day: u32, load: f32, recovery: f32) -> Session {
        Session { day, activity: "lift".into(), load, recovery }
    }

    #[test]
    fn classifies_effort_recovery_overreaching() {
        assert_eq!(classify(&s(1, 0.8, 0.7)), Status::Effort);
        assert_eq!(classify(&s(2, 0.3, 0.9)), Status::Recovery);
        assert_eq!(classify(&s(3, 0.9, 0.3)), Status::Overreaching);
        assert_eq!(classify(&s(3, 0.9, 0.3)).coherence(), -1.0);
    }

    #[test]
    fn drift_detects_load_up_recovery_down() {
        // load climbs 0.4→0.9, recovery falls 0.9→0.4 over 4 sessions.
        let sessions = vec![s(1, 0.4, 0.9), s(2, 0.5, 0.8), s(3, 0.8, 0.5), s(4, 0.9, 0.4)];
        assert!(drift(&sessions, 4) > 0.5, "expected positive drift");
        // steady state → ~no drift
        let steady = vec![s(1, 0.6, 0.6), s(2, 0.6, 0.6), s(3, 0.6, 0.6), s(4, 0.6, 0.6)];
        assert!(drift(&steady, 4).abs() < 0.01);
    }
}
