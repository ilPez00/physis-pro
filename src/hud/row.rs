//! Projection rows: what a lens hands the renderer, before any string exists.
//!
//! PH-026's lenses returned rendered lines, which forced every consumer either
//! to parse them back or to re-derive the object — the `render string -> parse
//! string -> guess object` loop. A row keeps the **identity** and the *facts*
//! (relation, origin, score) structured until the last moment, so keyboard
//! navigation, selection, Enter, the palette, WHY and relation inspection all
//! operate on the same canonical `ObjectId`.
//!
//! The App still owns the *state* markers (selected, changed, affected, intent,
//! pinned): those are properties of the operator's session, not of the world,
//! and a row must not pretend to carry them.

use crate::nav::RelationOrigin;
use crate::world_object::WorldObjectId;

/// How a row sits relative to the projection's centre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowRelation {
    /// The row *is* the centre of the projection.
    Center,
    /// A recorded relation, with its kind as stored (`relates to`, `same`, …).
    Recorded(String),
    /// A similarity neighbour from the shared ranked list.
    Near,
    /// A containment child (a directory's entry, a file's symbol).
    Child,
}

impl RowRelation {
    /// The short label a renderer shows for this relation.
    pub fn label(&self) -> &str {
        match self {
            RowRelation::Center => "center",
            RowRelation::Recorded(kind) => kind,
            RowRelation::Near => "near",
            RowRelation::Child => "contains",
        }
    }

    /// `(direction_glyph, kind)` for a graph-grammar row. `direction` is the
    /// arrow glyph showing how the row relates to the centre; `kind` is the
    /// relation label (e.g. `near`, `same`, `relates to`).
    pub fn kind_and_direction(&self) -> (&'static str, &str) {
        match self {
            RowRelation::Center => ("·", "center"),
            RowRelation::Recorded(_k) => ("→", self.label()),
            RowRelation::Near => ("⋯", "near"),
            RowRelation::Child => ("▸", "contains"),
        }
    }
}

/// One row of a projection. Everything here is world-derived; everything the
/// session adds (markers, affordances on demand) is layered by the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionRow {
    /// Canonical identity: the `origin:key` spelling of `nav::ObjectId`, which
    /// `world_object::parse_object_id` turns back into the exact id. Never a
    /// display form (`ObjectId::short()` truncates the content hash).
    pub object_id: WorldObjectId,
    pub label: String,
    pub path: String,
    pub relation: RowRelation,
    /// How the edge is known. `None` only for rows that are not edges (the
    /// centre, and containment, which is structural rather than asserted).
    pub origin: Option<RelationOrigin>,
    /// The score the shared scorer produced, when this row came from ranking.
    /// A similarity row is always `RowRelation::Near` + `Inferred`.
    pub score: Option<f32>,
}

/// A named subset of a projection's rows: what a lens decided the data
/// justifies calling something (`strong`, `bridge`, `sibling`, `recorded`).
/// Ids, not row copies — a row appears once in `rows`, and a group names rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionGroup {
    pub name: String,
    pub object_ids: Vec<String>,
}

impl ProjectionGroup {
    pub fn new(name: impl Into<String>, object_ids: Vec<String>) -> Self {
        Self {
            name: name.into(),
            object_ids,
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.object_ids.iter().any(|i| i == id)
    }
}

/// One lens's output: the centre it was built around, its rows, and the
/// categories it put them in. A row appears exactly once in `rows`; `groups`
/// only names rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    /// Which lens produced this, by its operator-facing name.
    pub lens: String,
    pub center: Option<ProjectionRow>,
    pub rows: Vec<ProjectionRow>,
    /// Categories the lens put rows in, by name. Never a second copy of a row.
    pub groups: Vec<ProjectionGroup>,
}

impl ProjectionRow {
    /// A similarity row. Its origin is `Inferred` **by construction**: there is
    /// no constructor that lets a neighbour claim a stronger epistemic status.
    pub fn near(
        object_id: impl Into<String>,
        label: impl Into<String>,
        path: impl Into<String>,
        score: f32,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            label: label.into(),
            path: path.into(),
            relation: RowRelation::Near,
            origin: Some(RelationOrigin::Inferred),
            score: Some(score),
        }
    }

    /// A row whose edge is known, carrying the origin the store recorded.
    pub fn related(
        object_id: impl Into<String>,
        label: impl Into<String>,
        path: impl Into<String>,
        kind: impl Into<String>,
        origin: RelationOrigin,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            label: label.into(),
            path: path.into(),
            relation: RowRelation::Recorded(kind.into()),
            origin: Some(origin),
            score: None,
        }
    }

    /// The centre row of a projection: not an edge, so no origin.
    pub fn center(
        object_id: impl Into<String>,
        label: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            label: label.into(),
            path: path.into(),
            relation: RowRelation::Center,
            origin: None,
            score: None,
        }
    }

    /// True when this row is a similarity neighbour — the only rows whose score
    /// is meaningful, and the only ones a renderer may label "near".
    pub fn is_similarity(&self) -> bool {
        matches!(self.relation, RowRelation::Near)
    }
}

impl Projection {
    pub fn new(
        lens: impl Into<String>,
        center: Option<ProjectionRow>,
        rows: Vec<ProjectionRow>,
    ) -> Self {
        Self {
            lens: lens.into(),
            center,
            rows,
            groups: Vec::new(),
        }
    }

    pub fn with_groups(mut self, groups: Vec<ProjectionGroup>) -> Self {
        self.groups = groups;
        self
    }

    /// The group with this name, when the lens produced one.
    pub fn group(&self, name: &str) -> Option<&ProjectionGroup> {
        self.groups.iter().find(|g| g.name == name)
    }

    /// Rows in a named group, in row order. Empty when the lens made no such
    /// group — a missing category is not a fabricated one.
    pub fn rows_in(&self, name: &str) -> Vec<&ProjectionRow> {
        match self.group(name) {
            Some(g) => self
                .rows
                .iter()
                .filter(|r| g.contains(&r.object_id))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Every row including the centre, centre first.
    pub fn all_rows(&self) -> Vec<&ProjectionRow> {
        let mut out: Vec<&ProjectionRow> = Vec::new();
        if let Some(c) = &self.center {
            out.push(c);
        }
        out.extend(self.rows.iter());
        out
    }

    /// Snapshot: flatten into `id,label` rows for the store, centre first.
    pub fn to_rows(&self) -> Vec<&ProjectionRow> { self.all_rows() }
}

impl crate::hud::Lens for Projection {
    fn name(&self) -> &str { &self.lens }

    fn render_rows(&self) -> Vec<String> {
        use crate::hud::lens_semantic::STRONG_SCORE;
        let mut lines = Vec::new();
        if let Some(c) = &self.center {
            lines.push(format!("● {}", c.label));
        }
        for r in self.all_rows().iter() {
            if self.center.as_ref().is_some_and(|c| c.object_id == r.object_id) { continue; }
            let (dir, kind) = r.relation.kind_and_direction();
            let score = r.score.unwrap_or(0.0);
            let mark = if score >= STRONG_SCORE { "◆" } else { "·" };
            lines.push(format!("  {} {} [{}] {} (id: {})", mark, dir, kind, r.label, r.object_id));
        }
        lines
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_object::parse_object_id;

    #[test]
    fn a_similarity_row_cannot_claim_a_stronger_origin() {
        // The epistemic invariant, held by construction rather than by a
        // renderer's discipline: the only neighbour constructor sets Inferred.
        let row = ProjectionRow::near("fs:abc", "a.rs", "a.rs", 0.7);
        assert_eq!(row.origin, Some(RelationOrigin::Inferred));
        assert!(row.is_similarity());
    }

    #[test]
    fn a_recorded_row_carries_the_origin_the_store_recorded() {
        let row = ProjectionRow::related(
            "fs:abc",
            "a.rs",
            "a.rs",
            "relates to",
            RelationOrigin::User,
        );
        assert_eq!(row.origin, Some(RelationOrigin::User));
        assert!(!row.is_similarity(), "a recorded edge is not a neighbour");
        assert_eq!(row.relation.label(), "relates to");
    }

    #[test]
    fn the_centre_is_not_an_edge_and_says_so() {
        let row = ProjectionRow::center("fs:abc", "a.rs", "a.rs");
        assert_eq!(row.origin, None, "the centre is not a claim about an edge");
        assert_eq!(row.relation, RowRelation::Center);
        assert_eq!(row.score, None);
    }

    #[test]
    fn every_row_identity_parses_back_to_the_canonical_id() {
        let full = "0e4bfb603e53a1b2c3d4e5f60718293a4b5c6d7e";
        let spell = format!("fs:{full}");
        let row = ProjectionRow::near(spell.clone(), "a.rs", "a.rs", 0.9);
        let oid = parse_object_id(&row.object_id);
        assert_eq!(oid.origin, "fs");
        assert_eq!(oid.key, full, "the full key survives, not a 12-char prefix");
    }

    #[test]
    fn all_rows_lists_the_centre_first() {
        let p = Projection::new(
            "semantic",
            Some(ProjectionRow::center("fs:1", "c", "c")),
            vec![ProjectionRow::near("fs:2", "n", "n", 0.5)],
        );
        let ids: Vec<&str> = p.all_rows().iter().map(|r| r.object_id.as_str()).collect();
        assert_eq!(ids, vec!["fs:1", "fs:2"]);
    }
}
