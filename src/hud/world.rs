//! The HUD's world slice: live state plus the focus it is rendered around.
//!
//! The HUD already had all of these values; they lived as loosely-related
//! fields on `App` in `src/bin/hud.rs` (cursor row, `selected`, zoom stack,
//! `view`, query). This module names the contract the projection system needs:
//!
//! ```text
//! cursor      what keyboard navigation currently points at (a row index)
//! selection   explicitly selected object(s) - survives projection changes
//! focus       the object/scope the current projection is built around
//! intent      what the human is trying to accomplish (the query/IR text)
//! trail       how the human reached this point (zoom stack + crumbs)
//! projection  how the same world is currently being shown (the view)
//! ```
//!
//! The invariant the vertical slice proves: switching projections never moves
//! the focus. `FocusState::switch_projection` is the one function that
//! enforces it - it changes only the projection id and leaves the focused
//! object untouched. A view switch is a re-projection of the same world,
//! never a pseudo-app open.

use crate::nav::ObjectId;

/// What is under the keyboard cursor right now: a row index into whatever the
/// current projection rendered. Projection-local and ephemeral - it is the one
/// piece of state a projection switch is allowed to reset, because rows belong
/// to the projection, not to the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Row index into the current projection's rows.
    pub row: usize,
}

impl Cursor {
    pub fn new(row: usize) -> Self {
        Self { row }
    }
}

/// Explicitly selected object(s). Unlike the cursor, the selection names world
/// objects by canonical id, so it is meaningful in every projection: select a
/// function in the project projection, switch to dependency, and the same
/// function is still selected.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    /// Canonical nav ids, in selection order.
    pub ids: Vec<ObjectId>,
}

impl Selection {
    pub fn empty() -> Self {
        Self { ids: Vec::new() }
    }

    pub fn single(id: ObjectId) -> Self {
        Self { ids: vec![id] }
    }

    pub fn contains(&self, id: &ObjectId) -> bool {
        self.ids.iter().any(|s| s == id)
    }

    /// Toggle membership; returns true when the id is selected afterwards.
    pub fn toggle(&mut self, id: ObjectId) -> bool {
        if let Some(pos) = self.ids.iter().position(|s| s == &id) {
            self.ids.remove(pos);
            false
        } else {
            self.ids.push(id);
            true
        }
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }
}

/// The object/scope the current projection is built around. `None` is the
/// workspace root: the unscoped world the HUD opens on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Focus {
    /// Canonical nav id of the focused object, or `None` for the root scope.
    pub object: Option<ObjectId>,
}

impl Focus {
    pub fn root() -> Self {
        Self { object: None }
    }

    pub fn on(id: ObjectId) -> Self {
        Self { object: Some(id) }
    }

    /// Move the focus to another object. Projection switches must not call
    /// this - see [`FocusState::switch_projection`].
    pub fn refocus(&mut self, id: Option<ObjectId>) {
        self.object = id;
    }
}

/// How the human reached this point: the zoom stack (outermost scope first)
/// plus the crumb trail the header shows. Human-owned history; the reducer
/// appends, projections only read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Trail {
    /// Zoom scopes entered, outermost first (mirrors the HUD zoom stack).
    pub stack: Vec<String>,
    /// Breadcrumb labels shown in the header.
    pub crumbs: Vec<String>,
}

impl Trail {
    pub fn push_scope(&mut self, scope: String, crumb: String) {
        self.stack.push(scope);
        self.crumbs.push(crumb);
    }

    pub fn pop_scope(&mut self) -> Option<String> {
        self.crumbs.pop();
        self.stack.pop()
    }
}

/// The eight shipped HUD projections, named the way the operator opens them.
/// This mirrors the `View` enum from `src/bin/hud.rs`, lifted so lenses can be
/// registered against it without the bin depending on library internals in
/// reverse. The set grows into the nine-lens programme (project, semantic,
/// schema, content, object, dependency, temporal, activity, fleet); the world
/// and graph projections are the vertical slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionId {
    /// The repository as a navigable world (world view).
    #[default]
    World,
    /// The focused object and its surrounding relations (graph view).
    ObjectGraph,
    /// Cited answers over the world (notebook view).
    Notebook,
    /// The task graph folded from observations (tasks view).
    Tasks,
    /// The agent roster and live availability (agents view).
    Agents,
    /// Editable runtime settings (settings view).
    Settings,
    /// Setup diagnostics (setup view).
    Setup,
    /// Command help (help view).
    Help,
}

impl ProjectionId {
    /// The operator-facing name - the same strings `--view` accepts today, so
    /// snapshots and muscle memory keep working while views become data.
    pub fn name(self) -> &'static str {
        match self {
            ProjectionId::World => "world",
            ProjectionId::ObjectGraph => "graph",
            ProjectionId::Notebook => "notebook",
            ProjectionId::Tasks => "tasks",
            ProjectionId::Agents => "agents",
            ProjectionId::Settings => "settings",
            ProjectionId::Setup => "setup",
            ProjectionId::Help => "help",
        }
    }

    pub fn all() -> [ProjectionId; 8] {
        [
            ProjectionId::World,
            ProjectionId::ObjectGraph,
            ProjectionId::Notebook,
            ProjectionId::Tasks,
            ProjectionId::Agents,
            ProjectionId::Settings,
            ProjectionId::Setup,
            ProjectionId::Help,
        ]
    }
}

/// Focus + selection + projection + trail: the complete answer to "what is the
/// operator looking at". The world state itself lives in
/// [`crate::nav::workspace::NavWorkspace`]; this struct is the lens state that
/// travels with the operator across projections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusState {
    pub cursor: Cursor,
    pub selection: Selection,
    pub focus: Focus,
    pub projection: ProjectionId,
    pub trail: Trail,
}

impl FocusState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch how the world is shown. Changes *only* the projection: the
    /// focused object, the explicit selection and the trail are untouched, so
    /// a view switch is a re-projection of the same world and the same object
    /// stays focused. The cursor resets because rows belong to the projection.
    pub fn switch_projection(&mut self, next: ProjectionId) {
        self.projection = next;
        self.cursor = Cursor::default();
    }

    /// Descend the focus into an object (zoom in): the old scope joins the
    /// trail so `Esc` climbs back out with the focus intact.
    pub fn descend(&mut self, scope: String, crumb: String, id: ObjectId) {
        self.trail.push_scope(scope, crumb);
        self.focus.refocus(Some(id));
        self.cursor = Cursor::default();
    }

    /// Climb one scope out (zoom out / Esc). Returns the scope left behind.
    pub fn ascend(&mut self) -> Option<String> {
        self.trail.pop_scope()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(origin: &str, key: &str) -> ObjectId {
        ObjectId::new(origin, key)
    }

    #[test]
    fn projection_switch_keeps_focus_selection_and_trail() {
        // THE invariant: switching world -> graph -> world never moves the
        // operator off the object they were looking at.
        let file = oid("fs", "abc123");
        let mut state = FocusState::new();
        state.focus.refocus(Some(file.clone()));
        state.selection.toggle(file.clone());
        state.trail.push_scope("src".into(), "src".into());

        state.switch_projection(ProjectionId::ObjectGraph);
        assert_eq!(state.projection, ProjectionId::ObjectGraph);
        assert_eq!(state.focus.object, Some(file.clone()));
        assert!(state.selection.contains(&file));
        assert_eq!(state.trail.stack, vec!["src".to_string()]);
        assert_eq!(state.cursor.row, 0, "cursor resets: rows belong to the projection");

        state.switch_projection(ProjectionId::World);
        assert_eq!(state.focus.object, Some(file.clone()));
        assert!(state.selection.contains(&file));
    }

    #[test]
    fn descend_pushes_trail_and_refocuses_ascend_climbs_back() {
        let mut state = FocusState::new();
        let dir = oid("fs", "dir1");
        let file = oid("fs", "file1");
        state.focus.refocus(Some(dir.clone()));
        state.descend("src".into(), "src".into(), file.clone());
        assert_eq!(state.focus.object, Some(file));
        assert_eq!(state.trail.stack.len(), 1);
        let left = state.ascend();
        assert_eq!(left, Some("src".to_string()));
        assert!(state.trail.stack.is_empty());
    }

    #[test]
    fn selection_toggle_is_idempotent_and_ordered() {
        let mut sel = Selection::empty();
        let a = oid("fs", "a");
        let b = oid("fs", "b");
        assert!(sel.toggle(a.clone()));
        assert!(sel.toggle(b.clone()));
        assert_eq!(sel.ids, vec![a.clone(), b.clone()]);
        assert!(!sel.toggle(a.clone()));
        assert_eq!(sel.ids, vec![b]);
        assert!(!sel.contains(&a));
    }

    #[test]
    fn projection_names_match_the_snapshot_view_names() {
        // `--view <name>` keeps working while views become data: the names the
        // operator types are the names this table carries.
        let names: Vec<&str> = ProjectionId::all().iter().map(|p| p.name()).collect();
        assert_eq!(
            names,
            vec!["world", "graph", "notebook", "tasks", "agents", "settings", "setup", "help"]
        );
    }
}
