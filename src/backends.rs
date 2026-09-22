//! Backend seams for pluggable stores, renderers, and history backends.
//!
//! Introduced 2026-09-22 from `docs/research/2026-09-22-rerun-mesa-findings.md`:
//! three narrowly-scoped traits so Rerun/Mesa backends can be wired later
//! (P0–P3 integration) **without deleting any existing Physis module**.
//! Each trait has exactly one default implementation backed by the existing
//! Physis-native stack; optional adapters are stub-gated behind features and
//! contain `todo!()` bodies until a future packet fleshes them out.
//!
//! Why only these three (no SemanticResolver/ProjectionBackend/etc. yet):
//! the findings explicitly defer speculative traits — these three are the only
//! seams for which a rerun/mesa backend has a measurable analogue today.

use std::path::Path;

/// Typed object storage (cells, components, relations).
///
/// Default: `PhysisNativeStore` (existing index JSON + sled in `src/nav/`).
/// Optional: `RerunChunkStore` wrapping `re_chunk_store::ChunkStore`
/// (see findings §Rerun store — verdict YES). Gated on `feature = "store-rerun"`.
///
/// The object-id space stays Physis (`WorldObjectId`/`ObjectId`); a rerun
/// backend maps identity → `EntityPath` on `locate`, so semantic identity does
/// not collapse onto a storage locator (H19).
pub trait WorldStore {
    /// Lookup an object's current cell + content by semantic identity.
    fn get_object(&self, identity: &str) -> Option<crate::world_object::WorldObject>;
    /// Persist an object (append-only; provenance stamped by caller).
    /// The default impl is `WorldStore::get_object`-only read access to the
    /// live store via `nav::NavWorkspace`; mutation still flows through the
    /// reducer (`hud/event.rs:reduce`) and the ledger.
    fn put_object(&self, _object: crate::world_object::WorldObject) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("put_object not implemented for this backend"))
    }
    /// Optional: expose the storage locator (Rerun `EntityPath`) for this id.
    fn locate(&self, _identity: &str) -> Option<String> { None }
}

/// Physis-native default backed by the live `nav` index + `ledger`.
/// Constructed through the public facade; no behavior change vs today.
pub struct PhysisNativeStore;

/// Rerun adapter skeleton. Gated so the `re_chunk_store` dependency is never
/// required unless someone opts in. Body is a `todo!()` — the findings commit
/// the verdict (YES, ChunkStore is usable), the implementation lands in a later
/// packet to keep this change behaviour-preserving.
#[cfg(feature = "store-rerun")]
pub struct RerunChunkStore;
#[cfg(feature = "store-rerun")]
impl WorldStore for RerunChunkStore {
    fn get_object(&self, _identity: &str) -> Option<crate::world_object::WorldObject> {
        // findings: re_chunk_store::ChunkStore + QueryCache::latest_at
        // (store/re_chunk_store/src/store.rs:527, store_app/re_query/src/latest_at.rs:43)
        todo!("PH-114-rerun-store: wrap ChunkStore; identity→EntityPath via locate")
    }
    fn locate(&self, _identity: &str) -> Option<String> {
        // Rerun EntityPath is a locator, not Physis identity (H19).
        todo!("PH-114-rerun-store: identity→EntityPath mapping")
    }
}

/// Temporal retrieval (latest-at / range) over a typed store.
///
/// Default: `LedgerStore` (`src/ledger.rs Causality`/`CausalLedger`).
/// Optional: `RerunTimeSeries` via `re_query::QueryCache::latest_at`
/// (findings §Rerun store). Gated on `feature = "temporal-rerun"`.
///
/// `Observation` here is a structural placeholder for a ledger/temporal record
/// — physis does not yet expose one publicly, so the seam is declared with a
/// local struct (never invented: the type is empty and TODO-documented) rather
/// than depending on a private path.
pub trait TemporalStore {
    /// Latest record of `component` for `entity` at `time_ref` (None = now).
    fn latest_at(&self, entity: &str, component: &str, time_ref: Option<&Path>) -> Option<Observation>;
    /// Range over a span; returns provenance-ordered records.
    fn range(&self, entity: &str, component: &str, _from: &str, _to: &str) -> Vec<Observation>;
}

/// Placeholder temporal record; shape deferred to PH-112/PH-115.
pub struct Observation {
    pub entity: String,
    pub component: String,
    pub _marker: (),
}

/// Ledger-backed default (append-only, per-tenant) — the existing
/// `CausalLedger` path; no behavior change.
pub struct LedgerStore;

#[cfg(feature = "temporal-rerun")]
pub struct RerunTimeSeries;
#[cfg(feature = "temporal-rerun")]
impl TemporalStore for RerunTimeSeries {
    fn latest_at(&self, _entity: &str, _component: &str, _time_ref: Option<&Path>) -> Option<Observation> {
        // findings: QueryCache::latest_at(query, EntityPath, components)
        todo!("PH-115-rerun-temporal: map entity+component→EntityPath+ComponentDescriptor")
    }
    fn range(&self, _entity: &str, _component: &str, _from: &str, _to: &str) -> Vec<Observation> {
        todo!("PH-115-rerun-temporal: range query via re_query RangeResults")
    }
}

/// View rendering seam.
///
/// Default: the existing ratatui HUD (`src/hud/`, `src/bin/hud.rs`).
/// Optional: a Rerun `ViewClass` registration path. Per the findings (§Rerun
/// visualization, verdict *reuse seams, do not embed the shell*), this does
/// NOT pull in `re_viewer` — it registers a Physis `ViewClass` against
/// `ViewClassRegistry` and renders through `re_renderer` (wgpu, no egui).
/// Gated on `feature = "rerun-graph"`.
pub trait GraphRenderer {
    fn render_worldview(&self, _object: &crate::world_object::WorldObject, _projection: &str);
    fn render_semantic_graph(&self, _roots: &[String], _projection: &str);
}

#[cfg(feature = "rerun-graph")]
pub struct RerunGraphView;
#[cfg(feature = "rerun-graph")]
impl GraphRenderer for RerunGraphView {
    fn render_worldview(&self, _object: &crate::world_object::WorldObject, _projection: &str) {
        // findings: register a ViewClass; re_viewer_context/src/view/
        // view_class.rs:156 ViewClass trait, view_class_registry.rs:199
        // ViewClassRegistry::new_context_collection:471
        // render through re_renderer (wgpu, no egui)
        todo!("PH-116-rerun-hud: register ViewClass, draw Physis WorldObject")
    }
    fn render_semantic_graph(&self, _roots: &[String], _projection: &str) {
        todo!("PH-116-rerun-hud: re_view_graph as a projection seam")
    }
}

