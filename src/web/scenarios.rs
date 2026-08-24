//! Scenario persistence for the decision planner (PLAN 7.3 + 7.4).
//!
//! `/planner/simulate` is stateless and never mutates live state. What it did
//! not answer is "what happened to yesterday's what-if?". This store gives
//! every simulation a durable, versioned home: saving stamps a version,
//! promotion is an explicit human act (one live version per scenario, always
//! reversible by demoting or promoting a different version), and every
//! changed item is kept as a **dated signifier shift** — a hypothesis with a
//! timestamp, never a silent fact.

use crate::planner::PlannerSimulation;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lifecycle of one stored scenario version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioStatus {
    /// Saved, under review. Nothing downstream sees it.
    Draft,
    /// Explicitly promoted by a human; the scenario's live reference version.
    Promoted,
}

/// One changed item, kept dated and hypothetical until a human promotes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignifierShift {
    pub item_id: String,
    pub from: String,
    pub to: String,
    /// When the shift was observed (the save that captured it).
    pub observed_at: DateTime<Utc>,
    /// Always true while in review; promotion flips the *version's* status,
    /// never silently rewrites these into facts.
    pub hypothesis: bool,
}

/// A persisted simulation: one immutable version of a scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioVersion {
    pub scenario_id: String,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub status: ScenarioStatus,
    pub promoted_at: Option<DateTime<Utc>>,
    /// Human-readable provenance: who saved it (`X-Physis-User`).
    pub created_by: String,
    pub summary: SimulationSummary,
    pub shifts: Vec<SignifierShift>,
}

/// The numbers a reviewer scans before opening the full diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSummary {
    pub records_scanned: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub low_confidence: usize,
}

impl SimulationSummary {
    fn of(sim: &PlannerSimulation) -> Self {
        Self {
            records_scanned: sim.records_scanned,
            changed: sim.changed,
            unchanged: sim.unchanged,
            low_confidence: sim.low_confidence,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    #[error("scenario {0} has no version {1}")]
    NotFound(String, u32),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Tenant-scoped scenario persistence over the shared engine sled file.
/// One key per version keeps listing a scan and makes writes race-free.
pub struct ScenarioStore<'a> {
    db: &'a sled::Db,
    uid: String,
}

impl<'a> ScenarioStore<'a> {
    pub fn new(db: &'a sled::Db, uid: &str) -> Self {
        Self {
            db,
            uid: uid.to_string(),
        }
    }

    /// Zero-padded version keeps scan_prefix ordering lexicographic == numeric;
    /// the uid namespaces tenants exactly like every other store.
    fn prefix(&self, scenario_id: &str) -> String {
        format!("planner:{}:{}:", self.uid, scenario_id)
    }

    fn key(&self, scenario_id: &str, version: u32) -> String {
        format!("{}{version:06}", self.prefix(scenario_id))
    }

    /// Persist `sim` as the next version of its scenario. Returns the stored
    /// record including derived signifier shifts.
    pub fn save(
        &self,
        sim: &PlannerSimulation,
        created_by: &str,
    ) -> anyhow::Result<ScenarioVersion> {
        let next = self.next_version(&sim.scenario_id)?;
        let observed_at = Utc::now();
        let record = ScenarioVersion {
            scenario_id: sim.scenario_id.clone(),
            version: next,
            created_at: observed_at,
            status: ScenarioStatus::Draft,
            promoted_at: None,
            created_by: created_by.to_string(),
            summary: SimulationSummary::of(sim),
            shifts: sim
                .transitions
                .iter()
                .flat_map(|t| {
                    sim.items
                        .iter()
                        .filter(|i| i.before.key() == t.from && i.after.key() == t.to)
                        .map(move |i| SignifierShift {
                            item_id: i.id.clone(),
                            from: t.from.clone(),
                            to: t.to.clone(),
                            observed_at,
                            hypothesis: true,
                        })
                })
                .collect(),
        };
        let raw = serde_json::to_vec(&record)?;
        self.db
            .insert(self.key(&record.scenario_id, next), raw)
            .map_err(anyhow::Error::from)?;
        Ok(record)
    }

    /// All versions of a scenario, newest first (metadata only).
    pub fn list(&self, scenario_id: &str) -> anyhow::Result<Vec<ScenarioVersion>> {
        let prefix = self.prefix(scenario_id);
        let mut out: Vec<ScenarioVersion> = Vec::new();
        for item in self.db.scan_prefix(prefix.as_bytes()) {
            let (_, raw) = item?;
            out.push(serde_json::from_slice(&raw)?);
        }
        out.sort_by_key(|v| std::cmp::Reverse(v.version));
        Ok(out)
    }

    /// The currently promoted version, if any. Test-exercised today; the
    /// review UI will read it through the same route as `list`.
    #[allow(dead_code)]
    pub fn live_version(&self, scenario_id: &str) -> anyhow::Result<Option<ScenarioVersion>> {
        Ok(self
            .list(scenario_id)?
            .into_iter()
            .find(|v| v.status == ScenarioStatus::Promoted))
    }

    /// Promote `version`. At most one live version per scenario: whatever else
    /// was promoted drops back to draft. Reversible by construction.
    pub fn promote(
        &self,
        scenario_id: &str,
        version: u32,
    ) -> Result<ScenarioVersion, ScenarioError> {
        let mut record = self.get(scenario_id, version)?;
        for mut other in self.list(scenario_id)? {
            if other.version != version && other.status == ScenarioStatus::Promoted {
                other.status = ScenarioStatus::Draft;
                other.promoted_at = None;
                let raw = serde_json::to_vec(&other).map_err(anyhow::Error::from)?;
                self.db
                    .insert(self.key(scenario_id, other.version), raw)
                    .map_err(anyhow::Error::from)?;
            }
        }
        record.status = ScenarioStatus::Promoted;
        record.promoted_at = Some(Utc::now());
        let raw = serde_json::to_vec(&record).map_err(anyhow::Error::from)?;
        self.db
            .insert(self.key(scenario_id, version), raw)
            .map_err(anyhow::Error::from)?;
        Ok(record)
    }

    pub fn get(&self, scenario_id: &str, version: u32) -> Result<ScenarioVersion, ScenarioError> {
        match self.db.get(self.key(scenario_id, version)) {
            Ok(Some(raw)) => serde_json::from_slice(&raw)
                .map_err(|e| {
                    ScenarioError::NotFound(format!("{scenario_id} (corrupt: {e})"), version)
                }),
            _ => Err(ScenarioError::NotFound(scenario_id.to_string(), version)),
        }
    }

    fn next_version(&self, scenario_id: &str) -> anyhow::Result<u32> {
        Ok(match self.list(scenario_id)?.first() {
            Some(v) => v.version + 1,
            None => 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{CellOutcome, PlannerItem};

    fn sim(id: &str) -> PlannerSimulation {
        let items = vec![
            PlannerItem {
                id: "a".into(),
                source: "s/a".into(),
                before: CellOutcome { domain: "OPERATE".into(), mode: "MONITOR".into(), score: 0.9 },
                after: CellOutcome { domain: "OPERATE".into(), mode: "CORRECT".into(), score: 0.8 },
            },
            PlannerItem {
                id: "b".into(),
                source: "s/b".into(),
                before: CellOutcome { domain: "BOND".into(), mode: "PLAY".into(), score: 0.9 },
                after: CellOutcome { domain: "BOND".into(), mode: "PLAY".into(), score: 0.9 },
            },
        ];
        crate::planner::simulate(id, items, 0.6)
    }

    #[test]
    fn save_versions_shifts_and_promotion_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("t.sled"))
            .open()
            .unwrap();
        let store = ScenarioStore::new(&db, "tenant-a");

        let v1 = store.save(&sim("procurement-v2"), "gio").unwrap();
        assert_eq!(v1.version, 1);
        assert_eq!(v1.status, ScenarioStatus::Draft);
        // Only the changed item becomes a shift; unchanged stays invisible.
        assert_eq!(v1.shifts.len(), 1);
        assert_eq!(v1.shifts[0].item_id, "a");
        assert!(v1.shifts[0].hypothesis);

        let v2 = store.save(&sim("procurement-v2"), "gio").unwrap();
        assert_eq!(v2.version, 2);

        // Promote v2 → it is live; promote v1 → v1 live, v2 back to draft.
        store.promote("procurement-v2", 2).unwrap();
        assert_eq!(store.live_version("procurement-v2").unwrap().unwrap().version, 2);
        store.promote("procurement-v2", 1).unwrap();
        let live = store.live_version("procurement-v2").unwrap().unwrap();
        assert_eq!(live.version, 1);
        assert_eq!(
            store.get("procurement-v2", 2).unwrap().status,
            ScenarioStatus::Draft,
            "promotion must be reversible"
        );

        // Unknown version errors cleanly.
        assert!(store.get("procurement-v2", 99).is_err());
    }

    #[test]
    fn scenarios_are_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("t.sled"))
            .open()
            .unwrap();

        let a = ScenarioStore::new(&db, "tenant-a");
        let b = ScenarioStore::new(&db, "tenant-b");
        a.save(&sim("alpha"), "gio").unwrap();

        assert!(b.list("alpha").unwrap().is_empty(), "tenants must not see each other");
        assert_eq!(a.list("alpha").unwrap().len(), 1);
    }
}
// ── HTTP surface ─────────────────────────────────────────────────────────────

use super::{user_key, SharedState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

fn err(status: StatusCode, msg: impl std::fmt::Display) -> Response {
    (status, Json(serde_json::json!({ "error": msg.to_string() }))).into_response()
}

/// POST /api/v1/planner/scenarios/save
/// Body: `{ "simulation": <PlannerSimulation> }`. Persists as the next draft
/// version for the caller's tenant and returns the stored record, shifts
/// included.
pub async fn scenario_save_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SaveScenarioRequest>,
) -> Response {
    let uid = user_key(&headers);
    let Some(db) = state.db.as_ref() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "persistence unavailable");
    };
    match ScenarioStore::new(db, &uid).save(&req.simulation, &uid) {
        Ok(record) => (StatusCode::CREATED, Json(record)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveScenarioRequest {
    pub simulation: crate::planner::PlannerSimulation,
}

/// GET /api/v1/planner/scenarios/:scenario_id — all versions, newest first.
pub async fn scenario_list_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(scenario_id): Path<String>,
) -> Response {
    let uid = user_key(&headers);
    let Some(db) = state.db.as_ref() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "persistence unavailable");
    };
    match ScenarioStore::new(db, &uid).list(&scenario_id) {
        Ok(versions) => (StatusCode::OK, Json(versions)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Debug, Deserialize)]
pub struct PromoteScenarioRequest {
    pub version: u32,
}

/// POST /api/v1/planner/scenarios/:scenario_id/promote
/// Body: `{ "version": N }`. Explicit + reversible: promoting another version
/// later demotes this one automatically.
pub async fn scenario_promote_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(scenario_id): Path<String>,
    Json(req): Json<PromoteScenarioRequest>,
) -> Response {
    let uid = user_key(&headers);
    let Some(db) = state.db.as_ref() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "persistence unavailable");
    };
    match ScenarioStore::new(db, &uid).promote(&scenario_id, req.version) {
        Ok(record) => Json(record).into_response(),
        Err(ScenarioError::NotFound(_, _)) => {
            err(StatusCode::NOT_FOUND, format!("no version {} of {scenario_id}", req.version))
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}
