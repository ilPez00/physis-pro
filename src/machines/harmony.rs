//! harmony — the music machine.
//!
//! Perceive a melody (pitches); reduce to **intervals** (the elaborable unit —
//! not raw notes, like euclid's polygons-not-circles); find recurring **motifs**
//! (repeated interval runs); elaborate by variation (transpose / invert). The
//! interval representation is key-invariant, so the same shape is recognized in
//! any key.
//!
//! Pure + testable.
//!
//! Standalone twin: `physis-pythagoras` (Python). Same interval core, but adds
//! two primitives not yet ported here — **rhythm** (temporal interval ratios
//! alongside pitch intervals) and **counterpoint** as the elaboration step.
//! Fold those in here rather than adding a separate `pythagoras` module.

use std::collections::HashMap;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// Consecutive pitch intervals (semitone deltas). Key-invariant melody shape.
pub fn intervals(notes: &[i32]) -> Vec<i32> {
    notes.windows(2).map(|w| w[1] - w[0]).collect()
}

/// Recurring interval runs of length `len` that appear ≥ 2 times — the motifs.
/// Returned most-frequent first.
pub fn motifs(notes: &[i32], len: usize) -> Vec<(Vec<i32>, usize)> {
    let iv = intervals(notes);
    if len == 0 || iv.len() < len {
        return vec![];
    }
    let mut counts: HashMap<Vec<i32>, usize> = HashMap::new();
    for w in iv.windows(len) {
        *counts.entry(w.to_vec()).or_default() += 1;
    }
    let mut found: Vec<(Vec<i32>, usize)> =
        counts.into_iter().filter(|(_, c)| *c >= 2).collect();
    found.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    found
}

/// Transpose: shift every pitch by `semitones` (a variation in another key).
pub fn transpose(notes: &[i32], semitones: i32) -> Vec<i32> {
    notes.iter().map(|n| n + semitones).collect()
}

/// Invert around the first pitch: each interval flips sign (mirror variation).
pub fn invert(notes: &[i32]) -> Vec<i32> {
    if notes.is_empty() {
        return vec![];
    }
    let pivot = notes[0];
    let mut out = vec![pivot];
    for &n in &notes[1..] {
        out.push(pivot - (n - pivot));
    }
    out
}

/// Adapter: register each motif as a labeled, bridged node.
pub fn register_motifs(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    notes: &[i32],
    len: usize,
) -> Vec<String> {
    let mut ids = Vec::new();
    for (motif, count) in motifs(notes, len) {
        let text = format!("motif {:?} ×{}", motif, count);
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_are_key_invariant() {
        // same shape, different key → identical intervals
        assert_eq!(intervals(&[60, 62, 64]), vec![2, 2]);
        assert_eq!(intervals(&[67, 69, 71]), vec![2, 2]);
    }

    #[test]
    fn finds_repeated_motif() {
        // "up2 up2 down1" appears twice
        let melody = vec![60, 62, 64, 63,  /* repeat */ 65, 67, 69, 68];
        let m = motifs(&melody, 3);
        assert!(m.iter().any(|(seq, c)| *seq == vec![2, 2, -1] && *c == 2), "motifs: {:?}", m);
    }

    #[test]
    fn transpose_and_invert() {
        assert_eq!(transpose(&[60, 64, 67], 5), vec![65, 69, 72]);
        // invert around 60: 64→56, 67→53
        assert_eq!(invert(&[60, 64, 67]), vec![60, 56, 53]);
    }
}
