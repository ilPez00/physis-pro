//! kinesis — the motion machine.
//!
//! Grounding: Aristotle's *Physics* — motion as the actualization of a potential,
//! and Zeno's paradox of the arrow (motion is not a sum of static points). The
//! primitive is a **trajectory gestalt**: a gesture reduced to its shape (path
//! length, net reach, straightness), not a raw coordinate. The noise is
//! sub-threshold *jitter* — segments below `min_move` are sensor tremor, not
//! motion, and are rejected as `euclid` rejects the circle. Composing gestures
//! marks the true **reaches**; the dream flags a **degrading** gait (a repeated
//! gesture whose amplitude is declining — pairs with `soma`).
//!
//! Pure + testable; only `register_motions` touches the core. The pose/IMU/video
//! perception layer lives upstream (a standalone `physis-kinesis` sensor repo);
//! this is the thin reducer over a trajectory already sampled.

use std::collections::HashMap;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One sampled position (a pose keypoint or device coordinate).
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// A labelled gesture: an ordered trajectory of points.
#[derive(Debug, Clone)]
pub struct Gesture {
    pub label: String,
    pub points: Vec<Point>,
}

/// A reduced motion gestalt — the shape of one gesture.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    pub label: String,
    /// Total path length over above-jitter segments.
    pub path: f32,
    /// Net displacement (start → end).
    pub net: f32,
    /// net / path ∈ [0, 1]; 1 = a straight reach, →0 = a wandering loop.
    pub straightness: f32,
    /// Number of above-jitter segments.
    pub moves: usize,
    pub hash: String,
}

fn dist(a: Point, b: Point) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Reduce gestures to motion gestalts. Segments shorter than `min_move` are
/// jitter and excluded from the path; a gesture whose whole path is jitter is
/// rejected (it never actualized a motion).
pub fn detect_motions(gestures: &[Gesture], min_move: f32) -> Vec<Motion> {
    let mut out = Vec::new();
    for g in gestures {
        if g.points.len() < 2 {
            continue;
        }
        let mut path = 0.0f32;
        let mut moves = 0usize;
        for w in g.points.windows(2) {
            let d = dist(w[0], w[1]);
            if d >= min_move {
                path += d;
                moves += 1;
            }
        }
        if path < min_move || moves == 0 {
            continue; // all jitter — not a motion
        }
        let net = dist(g.points[0], *g.points.last().unwrap());
        let straightness = (net / path).clamp(0.0, 1.0);
        out.push(Motion {
            label: g.label.clone(),
            path,
            net,
            straightness,
            moves,
            hash: format!("kinesis_{}", g.label),
        });
    }
    out
}

/// Reaches: motions with real net displacement (a directed movement, not a
/// wobble in place).
pub fn reaches(motions: &[Motion], min_net: f32) -> Vec<&Motion> {
    motions.iter().filter(|m| m.net >= min_net).collect()
}

/// Degrading gestures: a label whose repeated path length declines from first to
/// last occurrence by at least `drop` (e.g. 0.2 = 20% shorter) — a weakening
/// gait the dream surfaces (pairs with `soma`).
pub fn degrading(motions: &[Motion], drop: f32) -> Vec<String> {
    let mut by_label: HashMap<&str, Vec<f32>> = HashMap::new();
    for m in motions {
        by_label.entry(&m.label).or_default().push(m.path);
    }
    let mut out: Vec<String> = by_label
        .into_iter()
        .filter(|(_, paths)| {
            paths.len() >= 2 && {
                let first = paths[0];
                let last = *paths.last().unwrap();
                first > 0.0 && last <= first * (1.0 - drop)
            }
        })
        .map(|(label, _)| label.to_string())
        .collect();
    out.sort();
    out
}

/// Adapter: register each motion as a labeled, bridged node.
pub fn register_motions(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    motions: &[Motion],
) -> Vec<String> {
    let mut ids = Vec::new();
    for m in motions {
        let id = core.register_node_from_text(
            &format!(
                "motion: {} (reach {:.1}, straight {:.2})",
                m.label, m.net, m.straightness
            ),
            embedder,
        );
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn reach_detected_jitter_rejected() {
        let gestures = vec![
            // a straight reach
            Gesture {
                label: "reach".into(),
                points: vec![p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)],
            },
            // pure tremor: all segments below min_move
            Gesture {
                label: "tremor".into(),
                points: vec![p(0.0, 0.0), p(0.05, 0.0), p(0.0, 0.05), p(0.05, 0.05)],
            },
        ];
        let motions = detect_motions(&gestures, 0.5);
        assert_eq!(motions.len(), 1, "tremor rejected");
        let r = &motions[0];
        assert_eq!(r.label, "reach");
        assert!((r.path - 10.0).abs() < 1e-3);
        assert!((r.net - 10.0).abs() < 1e-3);
        assert!((r.straightness - 1.0).abs() < 1e-3);
        assert_eq!(reaches(&motions, 5.0).len(), 1);
    }

    #[test]
    fn degrading_gait_flagged() {
        let gestures = vec![
            Gesture {
                label: "step".into(),
                points: vec![p(0.0, 0.0), p(10.0, 0.0)],
            }, // path 10
            Gesture {
                label: "step".into(),
                points: vec![p(0.0, 0.0), p(6.0, 0.0)],
            }, // path 6 (−40%)
            Gesture {
                label: "wave".into(),
                points: vec![p(0.0, 0.0), p(4.0, 0.0)],
            },
            Gesture {
                label: "wave".into(),
                points: vec![p(0.0, 0.0), p(4.0, 0.0)],
            }, // steady
        ];
        let motions = detect_motions(&gestures, 0.5);
        let deg = degrading(&motions, 0.2);
        assert_eq!(deg, vec!["step".to_string()]);
    }
}
