//! The semantic lens: the focused object's meaning-neighbourhood as rows.
//!
//! It reads the **same** ranked list the world projection shows (no second
//! ranking, no second search engine) and splits it into the categories the data
//! can actually justify:
//!
//! * `center` — the focused object;
//! * `strong` — neighbours at or above a score threshold;
//! * `weak` — the rest of the ranked slice, kept because "not strong" is not
//!   "irrelevant" and hiding them would be a silent filter;
//! * `bridges` — candidates close to the centre *and* to at least one strong
//!   neighbour, computed by cosine over the embeddings the index already
//!   persisted. A bridge is a two-hop claim, so it is reported separately rather
//!   than promoted into the neighbour list;
//! * `siblings` — candidates sharing the centre's directory (deterministic
//!   structure, not similarity).
//!
//! Everything similarity-derived is `RelationOrigin::Inferred` by construction
//! (`ProjectionRow::near`). Nothing here re-ranks or calls a model.

use std::collections::BTreeSet;

use crate::backends::{Observation, TemporalStore};
use crate::hud::row::{Projection, ProjectionGroup, ProjectionRow};
use crate::nav::NavHit;
use crate::world_object::spell_object_id;

/// Score at or above which a neighbour is called *strong*. One number, named
/// here so a renderer never invents its own threshold.
pub const STRONG_SCORE: f32 = 0.5;

/// Cosine similarity between two vectors, `0.0` for a degenerate pair. Vectors
/// that do not share a length are not comparable: that is a space mismatch, and
/// reporting `0.0` keeps it visible as "unrelated" instead of panicking in a
/// projection.
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom <= f32::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

/// Candidates that are close to the centre *and* to at least one strong
/// neighbour: the two-hop claim, computed, never inferred from a name.
///
/// Pure, so the rule can be tested with hand-made vectors rather than whatever
/// an embedder happens to produce on this machine.
pub fn bridges(
    center: &[f32],
    strong: &[(String, Vec<f32>)],
    candidates: &[(String, Vec<f32>)],
    threshold: f32,
) -> Vec<String> {
    let mut out = Vec::new();
    for (id, vec) in candidates {
        if cosine(center, vec) < threshold {
            continue;
        }
        if strong
            .iter()
            .any(|(_, sv)| cosine(sv, vec) >= threshold)
        {
            out.push(id.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The semantic projection over one focused object and the ranked list.
pub fn project_rows(center: &NavHit, ranked: &[NavHit]) -> Projection {
    let center_path = center
        .object
        .source
        .as_ref()
        .map(|s| s.path.clone())
        .unwrap_or_else(|| center.object.label.clone());
    let center_row = ProjectionRow::center(
        spell_object_id(&center.object.id),
        center.object.label.clone(),
        center_path.clone(),
    );
    let center_dir = std::path::Path::new(&center_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut strong: Vec<(String, Vec<f32>)> = Vec::new();
    let mut strong_ids: BTreeSet<String> = BTreeSet::new();
    let mut rest: Vec<(String, Vec<f32>)> = Vec::new();
    let mut rows: Vec<ProjectionRow> = Vec::new();
    let mut siblings: Vec<ProjectionRow> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for h in ranked.iter().take(12) {
        let id = spell_object_id(&h.object.id);
        if id == center_row.object_id || !seen.insert(id.clone()) {
            continue;
        }
        let path = h
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| h.object.label.clone());
        if !h.object.embedding.is_empty() {
            if h.score.total >= STRONG_SCORE {
                strong.push((id.clone(), h.object.embedding.clone()));
                strong_ids.insert(id.clone());
            } else {
                rest.push((id.clone(), h.object.embedding.clone()));
            }
        }
        let row = ProjectionRow::near(id, h.object.label.clone(), path.clone(), h.score.total);
        if !center_dir.is_empty()
            && std::path::Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
                == center_dir
        {
            siblings.push(row.clone());
        }
        rows.push(row);
    }

    // Bridges are computed over what the ranking did not already call strong.
    let bridge_ids = bridges(
        &center.object.embedding,
        &strong,
        &rest,
        STRONG_SCORE,
    );

    rows.sort_by(|a, b| {
        let rank = |r: &ProjectionRow| match &r.relation {
            _ if bridge_ids.contains(&r.object_id) => 0,
            _ if strong_ids.contains(&r.object_id) => 1,
            _ => 2,
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| {
                b.score
                    .unwrap_or(0.0)
                    .partial_cmp(&a.score.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    // Rows stay unique: the categories name rows, they do not copy them. A
    // sibling is a row the ranking already produced; duplicating it into the
    // list would make the projection's length depend on how many categories
    // happened to overlap.
    let sibling_ids: Vec<String> = siblings.iter().map(|r| r.object_id.clone()).collect();
    let strong_ids: Vec<String> = strong_ids.into_iter().collect();
    Projection::new("semantic", Some(center_row), rows).with_groups(vec![
        ProjectionGroup::new("bridge", bridge_ids),
        ProjectionGroup::new("strong", strong_ids),
        ProjectionGroup::new("sibling", sibling_ids),
    ])
}

/// Register each ranked row as a typed `Observation` on the `TemporalStore`,
/// so the lens projection is also a temporal write. Only non-center rows are
/// emitted (the center is the focus, not a neighbour record). `seq` follows
/// the render order so the store sees the projection's sort, then score desc.
///
/// `cell` is the `DOMAIN/MODE` label resolved by the caller (the HUD owns the
/// OntologicalMap and classifier); pass `None` when the cell is not yet known
/// — `Observation.cell` stays `Option`, never fabricated.
pub fn register_projection(
    center_object_id: &str,
    rows: &[crate::hud::row::ProjectionRow],
    store: &mut dyn TemporalStore,
    component: &str,
    actor: &str,
    cell: Option<&str>,
) {
    for (seq, row) in rows.iter().enumerate() {
        if row.object_id == center_object_id {
            continue;
        }
        store.register(
            &row.object_id,
            component,
            Observation {
                seq: seq as u64,
                at: None,
                actor: actor.to_string(),
                object: row.object_id.clone(),
                component: component.to_string(),
                row: seq as u64,
                cell: cell.map(str::to_string),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{ObjectId, ObjectKind, PhysisObject, RetrievalScore, SourceRef};

    fn hit(path: &str, key: &str, score: f32, embedding: Vec<f32>) -> NavHit {
        NavHit {
            object: PhysisObject {
                id: ObjectId::new("fs", key),
                kind: ObjectKind::File,
                label: path.to_string(),
                source: Some(SourceRef {
                    path: path.to_string(),
                    start_line: None,
                    end_line: None,
                }),
                mtime: 0.0,
                hash: key.to_string(),
                text: path.to_string(),
                embedding,
            },
            score: RetrievalScore {
                total: score,
                components: Default::default(),
            },
        }
    }

    #[test]
    fn every_row_is_inferred_and_addressable() {
        // The epistemic rule for a similarity projection: no row may claim a
        // recorded origin, and every row must parse back to the exact id the
        // index holds (a 64-char content hash, not a 12-char display prefix).
        const LONG_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        const LONG_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let center = hit("src/a.rs", "aaaa", 1.0, vec![1.0, 0.0, 0.0]);
        let ranked = vec![
            hit("src/b.rs", LONG_B, 0.9, vec![1.0, 0.1, 0.0]),
            hit("src/c.rs", LONG_C, 0.2, vec![0.0, 1.0, 0.0]),
        ];
        let p = project_rows(&center, &ranked);
        assert_eq!(p.lens, "semantic");
        assert_eq!(p.rows.len(), 2, "every ranked object becomes one row");
        let keys: Vec<String> = p
            .rows
            .iter()
            .map(|r| crate::world_object::parse_object_id(&r.object_id).key)
            .collect();
        assert!(
            keys.contains(&LONG_B.to_string()) && keys.contains(&LONG_C.to_string()),
            "the full keys survive: {keys:?}"
        );
        for row in &p.rows {
            assert_eq!(
                row.origin,
                Some(crate::nav::RelationOrigin::Inferred),
                "similarity never claims a recorded origin: {row:?}"
            );
            assert!(row.is_similarity());
        }
    }

    #[test]
    fn strong_and_sibling_are_measured_categories_not_guesses() {
        let center = hit("src/a.rs", "aaaa", 1.0, vec![1.0, 0.0, 0.0]);
        let ranked = vec![
            hit("src/b.rs", "bbbb", 0.9, vec![1.0, 0.1, 0.0]), // strong + sibling
            hit("doc/c.rs", "cccc", 0.1, vec![0.0, 1.0, 0.0]), // weak, other dir
        ];
        let p = project_rows(&center, &ranked);
        let strong = p.rows_in("strong");
        assert_eq!(strong.len(), 1, "one row clears the strong threshold");
        assert!(strong[0].path.starts_with("src/"), "{:?}", strong[0]);
        let siblings = p.rows_in("sibling");
        assert_eq!(siblings.len(), 1, "one row shares the centre's directory");
        assert_eq!(siblings[0].path, "src/b.rs");
        // A category can be legitimately empty; that is not an error.
        assert!(p.rows_in("no-such-group").is_empty());
    }

    #[test]
    fn a_bridge_is_a_two_hop_claim_over_the_vectors() {
        // Hand-made vectors, so the rule is tested rather than the embedder.
        // centre ~ x; neighbour A ~ x; candidate C is close to the centre AND to
        // A, so it is a bridge. Candidate D is close to the centre only.
        let centre = vec![1.0, 0.0, 0.0];
        let strong = vec![("a".to_string(), vec![0.99, 0.1, 0.0])];
        let candidates = vec![
            ("c".to_string(), vec![0.98, 0.15, 0.0]),
            ("d".to_string(), vec![1.0, 0.9, 0.0]),
        ];
        let out = bridges(&centre, &strong, &candidates, 0.8);
        assert_eq!(out, vec!["c".to_string()], "only the two-hop candidate bridges");
    }

    #[test]
    fn a_space_mismatch_is_unrelated_not_a_panic() {
        // Vectors from different embedders are not comparable. The rule reports
        // "unrelated" instead of crashing a projection — a projection is not
        // allowed to take the HUD down because someone swapped a model.
        assert_eq!(cosine(&[1.0, 0.0], &[1.0, 0.0, 0.0]), 0.0);
        assert_eq!(cosine(&[], &[]), 0.0);
        assert!(bridges(&[1.0, 0.0], &[], &[("x".to_string(), vec![1.0, 0.0, 0.0])], 0.5).is_empty());
    }

    #[test]
    fn register_projection_writes_observations_excluding_center() {
        use crate::hud::row::ProjectionRow;

        // Build rows by hand: center + two neighbours.
        let rows: Vec<ProjectionRow> = vec![
            ProjectionRow::center("core:node:0", "center", "/src/a.rs"),
            ProjectionRow::near("core:node:1", "neighbor-1", "/src/b.rs", 0.9),
            ProjectionRow::near("core:node:2", "neighbor-2", "/doc/c.rs", 0.4),
        ];

        // Use the native store as a witness: register_projection should write
        // exactly one Observation per non-center row, ordered by seq.
        let mut store = crate::backends::PhysisNativeStore::new();
        register_projection(
            "core:node:0",
            &rows,
            &mut store,
            "semantics",
            "hud",
            None,
        );

        // Center excluded → 2 observations, one per neighbor.
        assert_eq!(store.range("core:node:1", "semantics", "", "").len(), 1, "neighbor-1 registered");
        assert_eq!(store.range("core:node:2", "semantics", "", "").len(), 1, "neighbor-2 registered");
        assert_eq!(store.range("core:node:0", "semantics", "", "").len(), 0, "center excluded");
        assert_eq!(
            store.latest_at("core:node:1", "semantics", None).map(|o| o.actor),
            Some("hud".to_string()),
            "actor propagated from the projection"
        );
    }
}
