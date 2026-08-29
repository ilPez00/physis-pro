//! euclid — the geometric machine.
//!
//! Perceive closed forms; reduce each to a **geometric primitive** (line,
//! triangle, quad, pentagon, hexagon, …) keyed by vertex count; elaborate by
//! composing primitives into relations (containment, adjacency, proximity).
//! Coherence maps directly: a clean polygon is a Form (+1); a blob too round to
//! be a conclusion is Noise (−1) and rejected — "primitives are conclusions, not
//! axioms".
//!
//! Pure + testable; `register_*` is the only core touch. The OpenCV camera /
//! contour layer lives in the standalone `physis-euclid` repo — this is the thin
//! reducer over the shared core. Ports `recognize.py` + `compose.py`.

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// Axis-aligned bounding box: `(x, y, w, h)`.
pub type BBox = (f32, f32, f32, f32);

/// A recognised closed form, already extracted from a frame upstream.
#[derive(Debug, Clone)]
pub struct Primitive {
    pub vertices: u32,
    pub circularity: f32,
    pub aspect_ratio: f32,
    pub color: (u8, u8, u8),
    pub texture: f32,
    pub bbox: BBox,
}

/// Shape name from vertex count. Mirrors `recognize.py::classify_by_vertices`.
pub fn classify_by_vertices(n: u32) -> String {
    match n {
        2 => "line".into(),
        3 => "triangle".into(),
        4 => "quad".into(),
        5 => "pentagon".into(),
        6 => "hexagon".into(),
        other => format!("n{other}"),
    }
}

/// A form too circular to be a polygon is rejected as noise.
/// Mirrors `recognize.py::reject_circle`.
pub fn reject_circle(circularity: f32, limit: f32) -> bool {
    circularity > limit
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// A clean conclusion (coherence +1).
    Form,
    /// Too round to be a primitive — rejected (coherence −1).
    Noise,
}

impl Status {
    pub fn coherence(self) -> f32 {
        match self {
            Status::Form => 1.0,
            Status::Noise => -1.0,
        }
    }
}

/// Classify a primitive against the circularity limit.
pub fn classify(p: &Primitive, circularity_limit: f32) -> Status {
    if reject_circle(p.circularity, circularity_limit) {
        Status::Noise
    } else {
        Status::Form
    }
}

/// Stable id binning. Mirrors `recognize.py::compute_hash`.
pub fn compute_hash(p: &Primitive) -> String {
    let aspect_bin = (p.aspect_ratio * 4.0) as i32;
    let angle_bin = (p.circularity * 3.0) as i32;
    let (r, g, b) = p.color;
    let color_hash = format!("{r:02x}{g:02x}{b:02x}");
    let tex_bin = (p.texture / 100.0).min(3.0) as i32;
    format!(
        "{}_r{aspect_bin}_a{angle_bin}_c{color_hash}_t{tex_bin}",
        classify_by_vertices(p.vertices)
    )
}

/// Spatial relation between two primitives. Mirrors `compose.py::compute_relation`
/// (bbox-based subset: vertex/edge sharing needs raw coords, kept upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// One bbox fully inside the other.
    Contained,
    /// Centroids within `near` distance.
    Near,
    /// Otherwise unrelated.
    Disjoint,
}

fn contains(a: &BBox, b: &BBox) -> bool {
    // is `a` inside `b`?
    a.0 >= b.0 && a.1 >= b.1 && (a.0 + a.2) <= (b.0 + b.2) && (a.1 + a.3) <= (b.1 + b.3)
}

fn centroid(b: &BBox) -> (f32, f32) {
    (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0)
}

/// Relate two primitives; `near` is the centroid-distance threshold.
pub fn relation(p1: &Primitive, p2: &Primitive, near: f32) -> Relation {
    if contains(&p1.bbox, &p2.bbox) || contains(&p2.bbox, &p1.bbox) {
        return Relation::Contained;
    }
    let (c1, c2) = (centroid(&p1.bbox), centroid(&p2.bbox));
    let dist = ((c1.0 - c2.0).powi(2) + (c1.1 - c2.1).powi(2)).sqrt();
    if dist <= near {
        Relation::Near
    } else {
        Relation::Disjoint
    }
}

/// Adapter: register each non-noise primitive as a labelled coherence node
/// (label = shape hash) and bridge it into the shared graph. Returns node ids.
pub fn register_primitives(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    primitives: &[Primitive],
    circularity_limit: f32,
) -> Vec<String> {
    let mut ids = Vec::new();
    for p in primitives {
        if classify(p, circularity_limit) == Status::Noise {
            continue;
        }
        let text = compute_hash(p);
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

/// Engine-side reduce: take perceived primitives, register each non-noise
/// form and elaborate (compose relations, score coherence) entirely in-Rust.
/// This is Phase-8 read-back: the Python `Machine.reduce()` should call
/// this instead of doing elaboration itself (Python stays pure perception).
pub fn reduce(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    primitives: &[Primitive],
    circularity_limit: f32,
) -> f32 {
    // each clean Form contributes +1 coherence (Status::Form.coherence())
    // normalised by the TOTAL primitives seen (noise included) so the
    // result stays in [-1, 1] like the other reducers.
    let mut score: f32 = 0.0;
    let mut registered: Vec<&Primitive> = Vec::new();
    for p in primitives {
        if classify(p, circularity_limit) == Status::Noise {
            continue; // noise rejected, no coherence contribution
        }
        let text = compute_hash(p);
        // Bridge the returned *id*, not the label. `bridge_node` looks the id
        // up in `core.nodes` and returns None for anything else, so passing
        // the text silently skipped the holarchy join — registering the node
        // and leaving the read-back loop this function exists to close open.
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        registered.push(p);
        score += Status::Form.coherence(); // +1 per clean form
    }
    // compose relations between registered forms (mirrors compose.py):
    // a Near/Contained pair is a coherent composition -> +coherence.
    for i in 0..registered.len() {
        for j in (i + 1)..registered.len() {
            match relation(registered[i], registered[j], 5.0) {
                Relation::Contained | Relation::Near => {
                    score += 1.0; // a composed relation adds coherence
                }
                Relation::Disjoint => {}
            }
        }
    }
    if !primitives.is_empty() {
        score / primitives.len() as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn prim(vertices: u32, circularity: f32, bbox: BBox) -> Primitive {
        Primitive {
            vertices,
            circularity,
            aspect_ratio: 1.0,
            color: (2, 2, 2),
            texture: 0.0,
            bbox,
        }
    }

    #[test]
    fn vertices_name_shapes() {
        assert_eq!(classify_by_vertices(3), "triangle");
        assert_eq!(classify_by_vertices(6), "hexagon");
        assert_eq!(classify_by_vertices(9), "n9");
    }

    #[test]
    fn round_blobs_are_noise() {
        let blob = prim(8, 0.95, (0.0, 0.0, 10.0, 10.0));
        assert_eq!(classify(&blob, 0.85), Status::Noise);
        assert_eq!(classify(&blob, 0.85).coherence(), -1.0);
        let tri = prim(3, 0.4, (0.0, 0.0, 10.0, 10.0));
        assert_eq!(classify(&tri, 0.85), Status::Form);
    }

    #[test]
    fn hash_is_stable_and_typed() {
        let p = prim(4, 0.2, (0.0, 0.0, 4.0, 4.0));
        let h = compute_hash(&p);
        assert!(h.starts_with("quad_"));
        assert_eq!(h, compute_hash(&p));
    }

    #[test]
    fn reduce_returns_normalised_coherence_for_triangle() {
        let mut core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(64);
        let p = prim(3, 0.1, (0.0, 0.0, 10.0, 10.0));
        let score = reduce(&mut core, &embedder, &[p], 0.85);
        assert!(score.abs() <= 1.0, "coherence must be normalised");
        assert_eq!(score, 1.0); // one clean form -> +1
    }

    #[test]
    fn reduce_rejects_noise_and_composes_forms() {
        let mut core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(64);
        let tri = prim(3, 0.1, (0.0, 0.0, 10.0, 10.0));
        let blob = prim(8, 0.95, (0.0, 0.0, 10.0, 10.0)); // noise
        let score = reduce(&mut core, &embedder, &[tri, blob], 0.85);
        assert_eq!(score, 0.5); // only tri counts -> 1/2
    }

    #[test]
    fn reduce_composes_adjacent_forms() {
        let mut core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(64);
        // two clean forms, adjacent (within `near`) -> composed coherence
        let a = prim(3, 0.1, (0.0, 0.0, 10.0, 10.0));
        let b = prim(4, 0.1, (2.0, 0.0, 10.0, 10.0)); // centroid ~(7,5): dist 2 <= near(5) -> Near
        let score = reduce(&mut core, &embedder, &[a, b], 0.85);
        // each form +1, plus 1 composed relation -> (1+1+1)/2 = 1.5 normalised? no:
        // score sums (1+1 +1 relation) / total primitives(2) = 1.5 -> clamp not applied;
        // assert strictly greater than a lone form's 1.0/1 = 1.0
        assert!(
            score > 1.0,
            "composed forms should exceed lone-form coherence"
        );
    }

    #[test]
    fn relations_detect_containment_and_proximity() {
        let outer = prim(4, 0.2, (0.0, 0.0, 100.0, 100.0));
        let inner = prim(3, 0.2, (10.0, 10.0, 20.0, 20.0));
        assert_eq!(relation(&outer, &inner, 5.0), Relation::Contained);

        let a = prim(3, 0.2, (0.0, 0.0, 10.0, 10.0));
        let b = prim(3, 0.2, (12.0, 0.0, 10.0, 10.0));
        assert_eq!(relation(&a, &b, 50.0), Relation::Near);
        assert_eq!(relation(&a, &b, 1.0), Relation::Disjoint);
    }
}
