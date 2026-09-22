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
#[derive(Default)]
pub struct PhysisNativeStore {
    /// Ordered observations by entity, used by `latest_at` and `range`.
    records: std::collections::BTreeMap<String, Vec<Observation>>,
}

impl PhysisNativeStore {
    /// Construct a fresh in-memory temporal store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl TemporalStore for PhysisNativeStore {
    fn latest_at(&self, entity: &str, component: &str, _time_ref: Option<&Path>) -> Option<Observation> {
        self.records
            .get(entity)?
            .iter()
            .rev()
            .find(|o| o.component == component)
            .cloned()
    }
    fn range(&self, entity: &str, component: &str, _from: &str, _to: &str) -> Vec<Observation> {
        self.records
            .get(entity)
            .map(|v| v.iter().filter(|o| o.component == component).cloned().collect())
            .unwrap_or_default()
    }
    fn register(&mut self, entity: &str, _component: &str, obs: Observation) {
        self.records.entry(entity.to_string()).or_default().push(obs);
    }
}

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
/// `Observation` is physis's typed temporal record: the rerun
/// `LatestAtResults`/`RangeResults` project onto it (PH-112). Field names
/// mirror the ledger's `CauseEvent` vocabulary so a temporal record reads the
/// same shape whether it came from `CausalLedger` or `RerunTimeSeries`:
/// `seq` (row/ingest order), `at` (wall-time), `actor` (producer), `object`
/// (the entity), `component` (the recorded slot). `at` is `Option<DateTime>`
/// — rerun's `TimeInt` is i64 nanos on `log_time`; we surface it as UTC only
/// when the timeline carries a real time.
pub trait TemporalStore {
    /// Latest record of `component` for `entity` at `time_ref` (None = now).
    fn latest_at(&self, entity: &str, component: &str, time_ref: Option<&Path>) -> Option<Observation>;
    /// Range over a span; returns provenance-ordered records.
    fn range(&self, entity: &str, component: &str, from: &str, to: &str) -> Vec<Observation>;
    /// Append a typed `Observation` for `entity`, stamped with `seq`
    /// (monotonic ingest order). This is the write seam used by the lens and
    /// HUD: every ranked `ProjectionRow` flows through here so the temporal
    /// store sees exactly the projection the renderer shows. Default impl is
    /// a no-op (read-only backends); `PhysisNativeStore` and `RerunTimeSeries`
    /// override.
    fn register(&mut self, _entity: &str, _component: &str, _obs: Observation) {}
}

/// Typed temporal record (PH-112). Mirrors `ledger::CauseEvent`'s vocabulary
/// so temporal projections are backend-agnostic: `seq` is the monotonic
/// ingest/row order, `at` the wall-time, `actor` the producer, `object` the
/// entity path, `component` the recorded slot. `TimeInt`/`RowId` from rerun
/// become `seq`/ingest position; `DateTime<Utc>` only when a real timeline
/// is available.
#[derive(Clone)]
pub struct Observation {

    /// Monotonic ingest/row order (rerun `RowId` → physis seq).
    pub seq: u64,
    /// Wall-time, when the timeline carries one (`log_time` → UTC).
    pub at: Option<chrono::DateTime<chrono::Utc>>,
    /// Producing actor/tool: `store-rerun` surfaces the embedder kind.
    pub actor: String,
    /// Physis identity this record attaches to.
    pub object: String,
    /// The component/slot recorded (rerun `ComponentIdentifier`).
    pub component: String,
    /// Raw row position for stable ordering; rerun `RowId` is u128 → u64.
    pub row: u64,
    /// The cell the entity resolved to, surfaced as `DOMAIN/MODE`.
    pub cell: Option<String>,
}

impl Observation {
    /// Construct from a seq + entity + component, wall-time unknown.
    pub fn at(seq: u64, object: impl Into<String>, component: impl Into<String>, actor: impl Into<String>) -> Self {
        Self {
            seq,
            at: None,
            actor: actor.into(),
            object: object.into(),
            component: component.into(),
            row: seq,
            cell: None,
        }
    }
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
    /// identity → resolved `DOMAIN/MODE` cell label.
    cells: std::collections::HashMap<String, String>,
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
            cells: std::collections::HashMap::new(),
        }
    }

    /// Register the locator + the resolved DOMAIN/MODE cell for a Physis
    /// identity. `cell` is the semiotic cell (`DOMAIN/MODE`) the entity
    /// resolves to — surfaced on `Observation` so temporal records carry the
    /// provenance the quality tracker and HUD need (never anonymous).
    pub fn register(&mut self, identity: &str, path: &str, cell: Option<&str>) {
        self.index.insert(identity.to_string(),
            re_log_types::EntityPath::from_single_string(path));
        if let Some(c) = cell {
            self.cells.insert(identity.to_string(), c.to_string());
        }
    }

    fn entity_path(&self, identity: &str) -> Option<&re_log_types::EntityPath> {
        self.index.get(identity)
    }
}

#[cfg(feature = "temporal-rerun")]
impl TemporalStore for RerunTimeSeries {
    fn latest_at(&self, entity: &str, component: &str, _time_ref: Option<&Path>) -> Option<Observation> {
        let path = self.entity_path(entity)?;
        // Static-time latest-at (read-only marker query; no timeline):
        // findings QueryCache::latest_at (latest_at.rs:43).
        let query = re_chunk::LatestAtQuery::new_static();
        let results = self.store.latest_at(
            re_chunk_store::ChunkTrackingMode::Report,
            &query,
            path,
            std::iter::empty::<re_chunk::ComponentIdentifier>(),
        );
        // PH-112 projection: min_index → (TimeInt, RowId). RowId.as_u128
        // is the monotonic ingest order; TimeInt.as_i64 is nanos on log_time.
        let (time_int, row_id) = results.min_index;
        let seq = row_id.as_u128() as u64;
        let at = time_int_to_utc(time_int);
        Some(Observation {
            seq,
            at,
            actor: "store-rerun".to_string(),
            object: entity.to_string(),
            component: component.to_string(),
            row: seq,
            cell: self.cells.get(entity).cloned(),
        })
    }
    fn range(&self, entity: &str, _component: &str, from: &str, to: &str) -> Vec<Observation> {
        let path = match self.entity_path(entity) {
            Some(p) => p,
            None => return Vec::new(),
        };
        // PH-112: parse ISO 8601 `from`/`to` into AbsoluteTimeRange via
        // TimeReal::from_secs(unix). `RangeResults` is chunked per entity;
        // we walk `per_chunk` units → rows. findings: QueryCache::range
        // (range.rs:25).
        let timeline = re_log_types::TimelineName::log_time();
        let range = iso_to_range(from, to);
        let query = re_chunk::RangeQuery {
            timeline,
            range,
            options: re_chunk::RangeQueryOptions {
                keep_extra_timelines: false,
                keep_extra_components: false,
                ..Default::default()
            },
        };
        let results = self.store.range(
            re_chunk_store::ChunkTrackingMode::Report,
            &query,
            path,
            std::iter::empty::<re_chunk::ComponentIdentifier>(),
        );
        // Project each row: RangeResults.components → Chunk → row_ids/times.
        // Walk every chunk's rows on `log_time`, build one Observation per row,
        // seq-ordered by RowId (rerun's monotonic ingest order).
        let cell = self.cells.get(entity).cloned();
        let mut out: Vec<Observation> = Vec::new();
        let log_time = re_log_types::TimelineName::log_time();
        for chunks in results.components.values() {
            for chunk in chunks {
                // Per-rerun: `times()` lives on `TimeColumn`, reached via
                // `Chunk::timelines()` keyed by TimelineName. `row_ids()`
                // parallels the time column row-wise.
                let tc = match chunk.timelines().get(&log_time) {
                    Some(tc) => tc,
                    None => continue,
                };
                let times: Vec<_> = tc.times().collect();
                let row_ids: Vec<_> = chunk.row_ids().collect();
                let n = times.len().min(row_ids.len());
                for i in 0..n {
                    out.push(Observation {
                        seq: row_ids[i].as_u128() as u64,
                        at: time_int_to_utc(times[i]),
                        actor: "store-rerun".to_string(),
                        object: entity.to_string(),
                        component: _component.to_string(),
                        row: row_ids[i].as_u128() as u64,
                        cell: cell.clone(),
                    });
                }
            }
        }
        out.sort_by_key(|o| o.seq);
        out
    }
}

/// PH-112 helper: rerun `TimeInt` (nanos on `log_time`, or STATIC) →
/// `Option<DateTime<Utc>>`. `STATIC` → `None` (no wall-time); MAX/sentinel →
/// `None` so a placeholder record never smuggles a bogus timestamp; otherwise
/// nanos-since-epoch → UTC via `DateTime::from_timestamp_opt(secs,nanos)`.
#[cfg(feature = "temporal-rerun")]
fn time_int_to_utc(t: re_chunk_store::TimeInt) -> Option<chrono::DateTime<chrono::Utc>> {
    if t.is_static() {
        return None;
    }
    let nanos = t.as_i64();
    if nanos < 0 || nanos == i64::MAX {
        return None;
    }
    let secs = nanos / 1_000_000_000;
    let nsecs = (nanos % 1_000_000_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsecs)
}

/// PH-112 helper: parse ISO 8601 `from`/`to` bounds into an
/// `AbsoluteTimeRange` (discrete `TimeInt`). Empty/unparseable bound widens
/// to `MAX`/`MIN` so a one-sided query still compiles. `AbsoluteTimeRange` is
/// constructed from the floored `AbsoluteTimeRangeF` (TimeReal→TimeInt).
#[cfg(feature = "temporal-rerun")]
fn iso_to_range(from: &str, to: &str) -> re_log_types::AbsoluteTimeRange {
    let parse = |s: &str| -> Option<i64> {
        if s.trim().is_empty() {
            return None;
        }
        chrono::DateTime::parse_from_rfc3339(s).map(|dt| dt.timestamp()).ok()
    };
    let lo = parse(from);
    let hi = parse(to);
    let lo_tr = lo.map_or(re_log_types::TimeReal::MIN, |s| re_log_types::TimeReal::from_secs(s as f64));
    let hi_tr = hi.map_or(re_log_types::TimeReal::MAX, |s| re_log_types::TimeReal::from_secs(s as f64));
    // AbsoluteTimeRange is (TimeInt, TimeInt); floored from TimeReal bounds.
    re_log_types::AbsoluteTimeRange { min: lo_tr.floor(), max: hi_tr.floor() }
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
        // PH-115/PH-112 gate: an identity not registered has no locator →
        // latest_at returns None (no phantom entities). Registering one
        // yields a typed Observation (PH-112 shape) with the right identity.
        let mut s = RerunTimeSeries::new("physis:temporal-H6");
        assert!(s.latest_at("core:node:missing", "semantics", None).is_none());
        // register now takes the optional cell label.
        s.register("core:node:0", "world/object/0", Some("BOND/COMMUNICATION"));
        let obs = s.latest_at("core:node:0", "semantics", None).expect("obs surfaced");
        assert_eq!(obs.object, "core:node:0");
        assert_eq!(obs.component, "semantics");
        assert_eq!(obs.cell.as_deref(), Some("BOND/COMMUNICATION"));
        // range over an unregistered identity is empty (not panicking).
        assert!(s.range("core:node:missing", "semantics", "", "").is_empty());
        // range over a registered-but-empty entity yields no rows, no panic.
        let r = s.range("core:node:0", "semantics", "", "");
        assert!(r.is_empty(), "empty store yields no observations");
    }
}

