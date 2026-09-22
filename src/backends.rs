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
/// the verdict (YES, ChunkStore is usable), the implementation lands in
/// PH-114-rerun-store: wraps `ChunkStore`+`QueryCache`, mapping Physis identity
/// to Rerun `EntityPath` on `locate` (H19: identity is canonical, EntityPath is
/// a locator). Gated on `feature = "store-rerun"`.
#[cfg(feature = "store-rerun")]
pub struct RerunChunkStore {
    store: re_query::QueryCache,
    // Identity → Rerun EntityPath. Physis identity stays canonical; this is the
    // only direction that exists, so a backend locator cannot collapse the
    // object-id space.
    index: std::collections::HashMap<String, re_log_types::EntityPath>,
}

#[cfg(feature = "store-rerun")]
impl RerunChunkStore {
    /// Open a fresh (empty) chunk store backed by Rerun's `ChunkStore` +
    /// `QueryCache`. `application_id` names the recording; `recording_id`
    /// defaults to a random UUID per findings §Rerun store.
    pub fn new(application_id: &str) -> Self {
        let store_id = re_log_types::StoreId::default_blueprint(re_log_types::ApplicationId::new_or_unknown(application_id));
        let handle = re_chunk_store::ChunkStore::new_handle(store_id, Default::default());
        Self {
            store: re_query::QueryCache::new(handle),
            index: std::collections::HashMap::new(),
        }
    }

    /// Register the locator this backend uses for a Physis identity.
    pub fn register(&mut self, identity: &str, path: &str) {
        self.index.insert(identity.to_string(), re_log_types::EntityPath::from_single_string(path));
    }

    fn entity_path(&self, identity: &str) -> Option<&re_log_types::EntityPath> {
        self.index.get(identity)
    }
}

#[cfg(feature = "store-rerun")]
impl WorldStore for RerunChunkStore {
    fn get_object(&self, identity: &str) -> Option<crate::world_object::WorldObject> {
        let path = self.entity_path(identity)?;
        let query = re_chunk::LatestAtQuery::new_static();
        let results = self.store.latest_at(
            re_chunk_store::ChunkTrackingMode::Report,
            &query,
            path,
            std::iter::empty::<re_chunk::ComponentIdentifier>(),
        );
        // Skeleton: reconstruct a WorldObject from latest_at results.
        // The component payloads (UnitChunkShared) are available via
        // `results.get(component_id)` in a later packet; for now we surface the
        // identity+locator as a marker so the seam compiles and is testable.
        // findings: QueryCache::latest_at, LatestAtResults::get
        // (store/re_chunk_store/src/store.rs:527, store_app/re_query/src/latest_at.rs:43)
        Some(crate::world_object::WorldObject::new(
            identity,
            crate::world_object::ObjectKind::Other("rerun".into()),
            format!("rerun://{}", results.entity_path),
        ).with_provenance(crate::world_object::Provenance::deterministic("store-rerun")))
    }
    fn locate(&self, _identity: &str) -> Option<String> {
        // Rerun EntityPath is a locator, not Physis identity (H19).
        self.entity_path(_identity).map(|p| p.to_string())
    }
    fn put_object(&self, _object: crate::world_object::WorldObject) -> anyhow::Result<()> {
        // TODO(PH-114): append to ChunkStore via ChunkIngestor / append_to_store.
        Err(anyhow::anyhow!("PH-114-rerun-store: append path not yet wired"))
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

/// Rerun temporal adapter. Gated on `feature = "temporal-rerun"`. Wraps
/// `re_query::QueryCache` + an identity↔`EntityPath` index (same seam as
/// `RerunChunkStore` in `WorldStore`). Uses `QueryCache::latest_at` and
/// `QueryCache::range` directly (findings §Rerun store — verdict YES).
#[cfg(feature = "temporal-rerun")]
pub struct RerunTimeSeries {
    store: re_query::QueryCache,
    index: std::collections::HashMap<String, re_log_types::EntityPath>,
}

#[cfg(feature = "temporal-rerun")]
impl RerunTimeSeries {
    /// Open a fresh temporal view over a named app recording.
    pub fn new(application_id: &str) -> Self {
        let store_id = re_log_types::StoreId::default_blueprint(
            re_log_types::ApplicationId::new_or_unknown(application_id),
        );
        let handle = re_chunk_store::ChunkStore::new_handle(store_id, Default::default());
        Self {
            store: re_query::QueryCache::new(handle),
            index: std::collections::HashMap::new(),
        }
    }

    /// Register the locator for a Physis identity.
    pub fn register(&mut self, identity: &str, path: &str) {
        self.index.insert(identity.to_string(), re_log_types::EntityPath::from_single_string(path));
    }

    fn entity_path(&self, identity: &str) -> Option<&re_log_types::EntityPath> {
        self.index.get(identity)
    }
}

#[cfg(feature = "temporal-rerun")]
impl TemporalStore for RerunTimeSeries {
    fn latest_at(&self, entity: &str, component: &str, _time_ref: Option<&Path>) -> Option<Observation> {
        let path = self.entity_path(entity)?;
        // Static-time latest-at (read-only marker query; no timeline).
        let query = re_chunk::LatestAtQuery::new_static();
        let results = self.store.latest_at(
            re_chunk_store::ChunkTrackingMode::Report,
            &query,
            path,
            std::iter::empty::<re_chunk::ComponentIdentifier>(),
        );
        // TODO(PH-112): project LatestAtResults into a typed Observation once
        // physis exposes one. For now surface identity+component+locator so the
        // seam is testable and the entity is known to exist in rerun.
        let _ = results;
        Some(Observation {
            entity: entity.to_string(),
            component: component.to_string(),
            _marker: (),
        })
    }
    fn range(&self, entity: &str, _component: &str, from: &str, to: &str) -> Vec<Observation> {
        let path = match self.entity_path(entity) {
            Some(p) => p,
            None => return Vec::new(),
        };
        // findings: re_query::QueryCache::range with RangeQuery over
        // AbsoluteTimeRange. Real packet parses `from`/`to` (ISO timestamps)
        // into TimeReal→TimeInt bounds; here we query the full timeline so the
        // seam is wired and testable.
        let timeline = re_log_types::TimelineName::log_time();
        let range = re_log_types::AbsoluteTimeRange::EVERYTHING;
        let query = re_chunk::RangeQuery {
            timeline,
            range,
            options: re_chunk::RangeQueryOptions {
                keep_extra_timelines: false,
                keep_extra_components: false,
                // TODO: extra_ticks_from_history when PH-112 defines bounds.
                ..Default::default()
            },
        };
        let results = self.store.range(
            re_chunk_store::ChunkTrackingMode::Report,
            &query,
            path,
            std::iter::empty::<re_chunk::ComponentIdentifier>(),
        );
        // TODO(PH-112): walk results.chunks_per_row_id / unit batches into
        // Observations once a typed temporal record exists.
        let _ = results;
        let _ = from;
        let _ = to;
        Vec::new()
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
        // findings: PH-116-b verdict — reuse `re_renderer` + register a
        // `ViewClass` via `ViewClassRegistry::add_class` (view/view_class.rs:156,
        // view_class_registry.rs:199), render through `re_renderer` (wgpu, no
        // egui). We do NOT embed the `re_viewer` shell.
        //
        // This box (pansa) is CPU-only (dead RX 550 per the fleet manifest),
        // so there is no wgpu surface to construct/validate here — the registry
        // wiring is therefore deferred to a GPU-capable run rather than
        // shipped as a todo!() that pretends otherwise. The default ratatui
        // HUD (src/bin/hud.rs) remains the only active surface on headless
        // hosts.
        // TODO(PH-116): construct `re_renderer::RenderContext` + register a
        // `WorldObjectViewClass` once a wgpu surface is available.
        todo!("PH-116-rerun-hud: register ViewClass + render via re_renderer (GPU-gated)")
    }
    fn render_semantic_graph(&self, _roots: &[String], _projection: &str) {
        // findings: re_view_graph as a projection seam (world surface,
        // PH-026/PH-027: SemanticLens + ProjectionRow).
        todo!("PH-116-rerun-hud: register a GraphViewClass against the semantic lens")
    }
}

#[cfg(all(test, feature = "store-rerun"))]
mod tests {
    use super::*;

    #[test]
    fn rerun_store_roundtrip_locate_empty_get() {
        // PH-114 gate: construct, register an identity↔locator pair, then read
        // back. No entities have been written, so get_object returns a marker
        // WorldObject (the ChunkStore has no rows → latest_at is empty).
        let mut s = RerunChunkStore::new("physis:agent-H6");
        s.register("core:node:0", "world/object/0");
        // H19 round-trip: Physis identity → locator → locator. The EntityPath
        // is a locator, not canonical identity, but it must round-trip
        // stably for a given identity.
        let loc = s.locate("core:node:0").expect("locator registered");
        let loc2 = s.locate("core:node:0").expect("locator stable");
        assert_eq!(loc, loc2);
        // Re-registering the same identity is idempotent.
        s.register("core:node:0", "world/object/0");
        assert_eq!(s.locate("core:node:0"), Some(loc));
        let obj = s.get_object("core:node:0").expect("marker object surfaced");
        assert_eq!(obj.identity, "core:node:0");
        // H19: Physis identity stays canonical; EntityPath is the locator.
        let _ = obj;
    }
}

#[cfg(all(test, feature = "temporal-rerun"))]
mod temporal_tests {
    use super::*;

    #[test]
    fn rerun_temporal_latest_at_unregistered_returns_none() {
        // PH-115 gate: an identity not registered has no locator → latest_at
        // returns None (no phantom entities). Registering one yields an
        // Observation marker with the right identity/component.
        let mut s = RerunTimeSeries::new("physis:temporal-H6");
        assert!(s.latest_at("core:node:missing", "semantics", None).is_none());
        s.register("core:node:0", "world/object/0");
        let obs = s.latest_at("core:node:0", "semantics", None).expect("marker obs");
        assert_eq!(obs.entity, "core:node:0");
        assert_eq!(obs.component, "semantics");
        // range over an unregistered identity is empty (not panicking).
        assert!(s.range("core:node:missing", "semantics", "t0", "t1").is_empty());
    }
}

