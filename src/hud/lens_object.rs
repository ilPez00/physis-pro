//! Object lens data: one focused object and its surrounding relations.

use std::collections::BTreeSet;

use crate::nav::{NavHit, ObjectId, RelationOrigin};
use crate::nav::workspace::NavWorkspace;
use crate::world_object::{spell_object_id, RelationKind, WorldObject, WorldObjectId};

/// One rendered edge in the graph grammar. Carries the target's **identity**
/// (the full `origin:key` spelling), not just its label: a row that only knows
/// a display string cannot be acted on, selected or compared against the list,
/// which is how a projection silently becomes a second, weaker view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensEdge {
    pub id: WorldObjectId,
    pub direction: String,
    pub glyph: String,
    pub kind: String,
    pub label: String,
    pub path: String,
    pub origin: RelationOrigin,
}

/// The object lens: center, recorded edges, near-neighbour rows (Inferred).
#[derive(Debug, Clone)]
pub struct ObjectLens {
    pub center: WorldObject,
    pub edges: Vec<LensEdge>,
    pub near: Vec<LensEdge>,
}

impl crate::hud::Lens for ObjectLens {
    fn name(&self) -> &str { "graph" }
    fn render_rows(&self) -> Vec<String> { self.render() }
}

impl ObjectLens {
    pub fn edge_glyph(origin: RelationOrigin) -> &'static str {
        match origin {
            RelationOrigin::User => "───",
            RelationOrigin::Deterministic => "═══",
            RelationOrigin::Inferred => "- -",
        }
    }

    /// Project over recorded relations plus the current retrieval
    /// neighbourhood. Takes the near slice from the same ranked list the
    /// world projection shows — never re-ranks — so graph and list agree.
    pub fn project(ws: &NavWorkspace, center: &NavHit, hits: &[NavHit]) -> Self {
        let label = center.object.label.clone();
        // Identity is the *full* spelling of the nav id, so the lens's centre
        // parses back to the exact object the list showed. `short()` stays for
        // the row labels below, where it is a display form, not an identity.
        let mut obj = WorldObject::new(
            crate::world_object::spell_object_id(&center.object.id),
            kind_of(center),
            label.clone(),
        );
        obj.containment = center
            .object
            .source
            .as_ref()
            .and_then(|s| std::path::Path::new(&s.path).parent())
            .map(|p| format!("fs:{}", p.display()));

        let label_of = |id: &ObjectId| {
            ws.index()
                .objects
                .get(&serde_json::to_string(id).unwrap())
                .map(|o| o.label.clone())
                .unwrap_or_else(|| id.short())
        };
        let path_of = |id: &ObjectId| {
            ws.index()
                .objects
                .get(&serde_json::to_string(id).unwrap())
                .and_then(|o| o.source.as_ref())
                .map(|s| s.path.clone())
                .unwrap_or_default()
        };

        // Dedup by *identity*, never by label. Labels are display strings and
        // nothing guarantees they are unique across producers; identity is.
        // (Measured: not reachable through the fs index today — a file and its
        // passage carry distinct labels with spans — so this is a guarantee
        // held in advance, not a bug that was seen. The check below is what
        // makes that guarantee testable.)
        let center_id = spell_object_id(&center.object.id);
        let mut edges = Vec::new();
        let mut seen: BTreeSet<WorldObjectId> = BTreeSet::new();
        for r in ws.relations_of(&center.object) {
            let (other, kind, dir) = if r.a == center.object.id {
                (&r.b, r.kind.clone(), "→")
            } else {
                (&r.a, r.kind.clone(), "←")
            };
            let name = label_of(other);
            let target_id = spell_object_id(other);
            if target_id == center_id || !seen.insert(target_id.clone()) {
                continue;
            }
            edges.push(LensEdge {
                id: target_id,
                direction: dir.to_string(),
                glyph: Self::edge_glyph(r.origin).to_string(),
                kind,
                label: name,
                path: path_of(other),
                origin: r.origin,
            });
            obj = obj.relate_with_origin(RelationKind::SimilarTo, other.short(), r.origin);
        }

        let mut near = Vec::new();
        for h in hits.iter().take(6) {
            let target_id = spell_object_id(&h.object.id);
            if target_id == center_id || !seen.insert(target_id.clone()) {
                continue;
            }
            let p = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            near.push(LensEdge {
                id: target_id,
                direction: "·".to_string(),
                glyph: Self::edge_glyph(RelationOrigin::Inferred).to_string(),
                kind: format!("near {:.2}", h.score.total),
                label: h.object.label.clone(),
                path: p,
                origin: RelationOrigin::Inferred,
            });
            obj = obj.relate(RelationKind::SimilarTo, h.object.id.short());
        }

        ObjectLens { center: obj, edges, near }
    }

    /// Plain-text rows in the graph grammar (state markers stay in the bin).
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("● {}", self.center.content));
        for e in &self.edges {
            lines.push(format!(
                "  {} {} [{}] {} {}→  {}",
                e.direction, e.glyph, e.kind, e.label, e.glyph, e.path
            ));
        }
        for n in &self.near {
            lines.push(format!(
                "  {}  {} [{}] {} {}  {}",
                n.direction, n.glyph, n.kind, n.label, n.glyph, n.path
            ));
        }
        lines.push(String::new());
        lines.push("solid = user · double = deterministic · dashed = inferred/near".to_string());
        lines
    }
}

fn kind_of(hit: &NavHit) -> crate::world_object::ObjectKind {
    use crate::nav::ObjectKind as NavKind;
    use crate::world_object::ObjectKind as ViewKind;
    match hit.object.kind {
        NavKind::File => ViewKind::File,
        NavKind::Passage => ViewKind::Symbol,
        NavKind::Directory => ViewKind::Project,
        NavKind::Concept => ViewKind::Concept,
        NavKind::Note => ViewKind::Concept,
        NavKind::Other => ViewKind::Other("nav".into()),
    }
}
