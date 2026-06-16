//! chroma — the color machine.
//!
//! Grounding: Goethe's *Theory of Colours* — color known by its *relations* on
//! the wheel, not by wavelength. The primitive is a **color relation** between
//! two saturated hues: complement (~180°), analogous (~30°), or triad (~120°).
//! The noise is the *muddy* near-grey: a low-chroma color has no place on the
//! wheel and is rejected, as `euclid` refuses the circle. Composing relations
//! gives a palette; the dream surfaces the recurring clash.
//!
//! Pure + testable; only `register_relations` touches the core.

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// An RGB color (0–255 per channel).
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A saturated hue extracted from a color: angle on the wheel (0–360°) and its
/// chroma (saturation, 0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hue {
    pub deg: f32,
    pub chroma: f32,
}

/// The Goethean relation between two hues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Complement,
    Analogous,
    Triad,
    Clash,
}

/// A reduced relation between two named/indexed colors.
#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    pub a: usize,
    pub b: usize,
    pub kind: Kind,
    pub hash: String,
}

/// Hue angle + chroma from RGB (HSV-style). Chroma = (max − min) / max.
pub fn to_hue(c: Color) -> Hue {
    let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let chroma = if max > 0.0 { d / max } else { 0.0 };
    let deg = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };
    Hue { deg: (deg + 360.0) % 360.0, chroma }
}

/// Smallest angular distance between two hues (0–180°).
fn arc(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

/// Reduce colors to saturated hues, dropping the muddy near-grey below `min_chroma`.
pub fn detect_hues(colors: &[Color], min_chroma: f32) -> Vec<(usize, Hue)> {
    colors
        .iter()
        .enumerate()
        .map(|(i, &c)| (i, to_hue(c)))
        .filter(|(_, h)| h.chroma >= min_chroma)
        .collect()
}

/// Classify the relation between two hues by their arc, within `tol` degrees.
pub fn relation_kind(a: Hue, b: Hue, tol: f32) -> Kind {
    let d = arc(a.deg, b.deg);
    if (d - 180.0).abs() <= tol {
        Kind::Complement
    } else if d <= 30.0 + tol {
        Kind::Analogous
    } else if (d - 120.0).abs() <= tol {
        Kind::Triad
    } else {
        Kind::Clash
    }
}

/// All pairwise relations among the saturated hues of `colors`.
pub fn detect_relations(colors: &[Color], min_chroma: f32, tol: f32) -> Vec<Relation> {
    let hues = detect_hues(colors, min_chroma);
    let mut out = Vec::new();
    for i in 0..hues.len() {
        for j in (i + 1)..hues.len() {
            let (ia, ha) = hues[i];
            let (ib, hb) = hues[j];
            let kind = relation_kind(ha, hb, tol);
            out.push(Relation { a: ia, b: ib, kind, hash: format!("chroma_{ia}_{ib}") });
        }
    }
    out
}

/// Harmonies: complement / analogous / triad — the palette's resolved relations.
pub fn harmonies(relations: &[Relation]) -> Vec<&Relation> {
    relations.iter().filter(|r| r.kind != Kind::Clash).collect()
}

/// Clashes: the unresolved relation the dream surfaces.
pub fn clashes(relations: &[Relation]) -> Vec<&Relation> {
    relations.iter().filter(|r| r.kind == Kind::Clash).collect()
}

/// Adapter: register each color relation as a labeled, bridged node.
pub fn register_relations(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    relations: &[Relation],
) -> Vec<String> {
    let mut ids = Vec::new();
    for r in relations {
        let kind = match r.kind {
            Kind::Complement => "complement",
            Kind::Analogous => "analogous",
            Kind::Triad => "triad",
            Kind::Clash => "clash",
        };
        let id = core
            .register_node_from_text(&format!("color relation: {} <{kind}> {}", r.a, r.b), embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color { r: 230, g: 20, b: 20 };
    const CYAN: Color = Color { r: 20, g: 230, b: 230 }; // ~complement of red
    const ORANGE: Color = Color { r: 230, g: 140, b: 20 }; // analogous to red
    const GREY: Color = Color { r: 128, g: 128, b: 128 }; // muddy — rejected

    #[test]
    fn grey_rejected_complement_detected() {
        let hues = detect_hues(&[RED, CYAN, GREY], 0.2);
        assert_eq!(hues.len(), 2, "grey dropped");
        let rels = detect_relations(&[RED, CYAN], 0.2, 20.0);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].kind, Kind::Complement);
    }

    #[test]
    fn analogous_and_clash_partition() {
        let rels = detect_relations(&[RED, ORANGE], 0.2, 20.0);
        assert_eq!(rels[0].kind, Kind::Analogous);
        assert_eq!(harmonies(&rels).len(), 1);
        assert!(clashes(&rels).is_empty());
    }
}
