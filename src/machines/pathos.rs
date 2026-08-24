//! pathos — the affective machine.
//!
//! Grounding: the Stoic *pathē* and Spinoza's affects — emotion as a movement of
//! the soul's power of acting, indexed to a cause. The primitive is an **affect
//! pattern**: a recurring trigger carrying a characteristic valence (pleasure ↔
//! pain) and arousal (calm ↔ activated). The noise is the *flat* episode — near
//! zero valence and arousal, no movement of the soul — rejected as `harmony`
//! rejects the inharmonic. Composing patterns marks the **amplifiers** (what
//! activates the negative) and the **soothers** (what restores), and the dream
//! surfaces the trigger to anticipate.
//!
//! Pure + testable; only `register_affects` touches the core.

use std::collections::HashMap;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One affective episode tied to a trigger. `valence` ∈ [-1, 1]; `arousal` ∈ [0, 1].
#[derive(Debug, Clone)]
pub struct Episode {
    pub trigger: String,
    pub valence: f32,
    pub arousal: f32,
}

/// A recurring affect pattern: the mean valence/arousal a trigger carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Affect {
    pub trigger: String,
    pub valence: f32,
    pub arousal: f32,
    pub count: usize,
    pub hash: String,
}

/// Reduce episodes to recurring affect patterns. A trigger must recur at least
/// `min_count` times; episodes whose movement is below `flat` (|valence| and
/// arousal both under it) are flat noise and excluded from the mean.
pub fn detect_affects(episodes: &[Episode], min_count: usize, flat: f32) -> Vec<Affect> {
    let min = min_count.max(1);
    let mut agg: HashMap<&str, (f32, f32, usize)> = HashMap::new();
    for e in episodes {
        if e.valence.abs() < flat && e.arousal < flat {
            continue; // flat — no movement of the soul
        }
        let a = agg.entry(&e.trigger).or_insert((0.0, 0.0, 0));
        a.0 += e.valence;
        a.1 += e.arousal;
        a.2 += 1;
    }
    let mut out: Vec<Affect> = agg
        .into_iter()
        .filter(|(_, (_, _, n))| *n >= min)
        .map(|(trigger, (vsum, asum, count))| {
            let n = count as f32;
            Affect {
                trigger: trigger.to_string(),
                valence: vsum / n,
                arousal: asum / n,
                count,
                hash: format!("pathos_{trigger}"),
            }
        })
        .collect();
    // most activated first
    out.sort_by(|a, b| {
        b.arousal
            .partial_cmp(&a.arousal)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Amplifiers: negative-valence, high-arousal patterns — what activates distress.
pub fn amplifiers(affects: &[Affect], min_arousal: f32) -> Vec<&Affect> {
    affects
        .iter()
        .filter(|a| a.valence < 0.0 && a.arousal >= min_arousal)
        .collect()
}

/// Soothers: positive-valence, low-arousal patterns — what restores calm.
pub fn soothers(affects: &[Affect], max_arousal: f32) -> Vec<&Affect> {
    affects
        .iter()
        .filter(|a| a.valence > 0.0 && a.arousal <= max_arousal)
        .collect()
}

/// Adapter: register each affect pattern as a labeled, bridged node.
pub fn register_affects(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    affects: &[Affect],
) -> Vec<String> {
    let mut ids = Vec::new();
    for a in affects {
        let id = core.register_node_from_text(
            &format!(
                "affect: {} (v{:+.2} a{:.2})",
                a.trigger, a.valence, a.arousal
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

    fn ep(trigger: &str, valence: f32, arousal: f32) -> Episode {
        Episode {
            trigger: trigger.into(),
            valence,
            arousal,
        }
    }

    #[test]
    fn recurring_affect_detected_flat_rejected() {
        let episodes = vec![
            ep("deadline", -0.8, 0.9),
            ep("deadline", -0.6, 0.8), // amplifier
            ep("walk", 0.7, 0.2),
            ep("walk", 0.6, 0.25), // soother
            ep("commute", 0.02, 0.03),
            ep("commute", -0.01, 0.02), // flat noise
        ];
        let affects = detect_affects(&episodes, 2, 0.1);
        let dl = affects.iter().find(|a| a.trigger == "deadline").unwrap();
        assert!((dl.valence - (-0.7)).abs() < 1e-3);
        assert!((dl.arousal - 0.85).abs() < 1e-3);
        // flat commute excluded
        assert!(!affects.iter().any(|a| a.trigger == "commute"));
        assert_eq!(amplifiers(&affects, 0.5).len(), 1);
        assert_eq!(soothers(&affects, 0.5).len(), 1);
    }
}
