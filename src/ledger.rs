//! Causal ledger (PLAN Phase 10 D3) — the append-only record of *why every
//! artifact exists*: what produced it, from what, and when. The file address
//! (see `address.rs`) snapshots the producing event at ingestion; this ledger
//! keeps the whole chain so lineage queries walk history instead of guessing
//! from timestamps.
//!
//! Per-tenant (sled rows keyed by tenant), replay-stable: events carry a
//! monotonic `seq` and project onto the shared `OperationalEvent` pipeline
//! under the `cause` connector, so timelines assemble by construction — the
//! ledger IS the timeline source, not a parallel reporting layer.

use crate::address::Mechanism;
use crate::operational::{
    OperationalEvent, OperationalKind, OperationalProvenance, OperationalStatus,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One production event: `actor` did `mechanism` to `object`, optionally
/// deriving it from `input`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CauseEvent {
    /// Monotonic per-tenant sequence — stable across replays.
    pub seq: u64,
    pub at: DateTime<Utc>,
    pub mechanism: Mechanism,
    pub actor: String,
    /// The artifact this event is about (file path or node id).
    pub object: String,
    /// Parent artifact the object was derived from / received via.
    pub input: Option<String>,
}

/// Append-only causal history for one tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalLedger {
    events: Vec<CauseEvent>,
    next_seq: u64,
    /// object → indices into `events`. Rebuilt on [`Self::load`], appended on
    /// [`Self::record_at`]; every event has an object, so an empty index over
    /// non-empty events can only mean the indexes were skipped (a serde
    /// round-trip) — the walks then fall back to a scan, which is a
    /// performance loss, never a correctness one.
    /// ponytail: two HashMaps, O(V+E) walks; replaced the O(n·depth) lineage
    /// scan and the O(n²) descendants fixpoint.
    #[serde(skip)]
    by_object: std::collections::HashMap<String, Vec<usize>>,
    /// input → indices into `events` — the forward-walk leg.
    #[serde(skip)]
    by_input: std::collections::HashMap<String, Vec<usize>>,
}

impl CausalLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// All events in recording order (seq ascending). Queries filter and
    /// reverse as needed; the ledger itself stays append-only.
    pub fn events(&self) -> &[CauseEvent] {
        &self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn record(
        &mut self,
        mechanism: Mechanism,
        actor: &str,
        object: impl Into<String>,
        input: Option<String>,
    ) -> &CauseEvent {
        self.record_at(mechanism, actor, object, input, Utc::now())
    }

    /// Record with an explicit event time — for artifacts whose native clock
    /// is more truthful than the import moment (a message's Date header, a
    /// commit's author date). Seq stays monotonic; `at` may point backwards.
    pub fn record_at(
        &mut self,
        mechanism: Mechanism,
        actor: &str,
        object: impl Into<String>,
        input: Option<String>,
        at: DateTime<Utc>,
    ) -> &CauseEvent {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(CauseEvent {
            seq,
            at,
            mechanism,
            actor: actor.to_string(),
            object: object.into(),
            input,
        });
        self.events.last().expect("just pushed");
        self.append_index(self.events.len() - 1);
        self.events.last().expect("just pushed")
    }

    /// Add one event to the indexes by position. Append-only: O(1) per record,
    /// never a rebuild (bulk imports record in loops, and a rebuild per record
    /// would be O(n²) on exactly the loads the index exists to speed up).
    fn append_index(&mut self, i: usize) {
        let ev = &self.events[i];
        self.by_object.entry(ev.object.clone()).or_default().push(i);
        if let Some(input) = &ev.input {
            self.by_input.entry(input.clone()).or_default().push(i);
        }
    }

    /// Rebuild both indexes from `events`. Called by [`Self::load`], the only
    /// path events enter by other means than recording.
    fn build_index(&mut self) {
        self.by_object.clear();
        self.by_input.clear();
        for i in 0..self.events.len() {
            self.append_index(i);
        }
    }

    /// Events naming `obj` as their object, by index when fresh, by scan when
    /// the indexes were skipped. Callers still apply their own mechanism and
    /// seq filters, so the index changes nothing but lookup cost.
    fn events_with_object(&self, obj: &str) -> Vec<&CauseEvent> {
        if let Some(idxs) = self.by_object.get(obj) {
            return idxs.iter().map(|&i| &self.events[i]).collect();
        }
        if self.by_object.is_empty() && !self.events.is_empty() {
            return self.events.iter().filter(|e| e.object == obj).collect();
        }
        Vec::new()
    }

    /// Events deriving *from* `obj` (its `input` is `obj`).
    fn events_with_input(&self, obj: &str) -> Vec<&CauseEvent> {
        if let Some(idxs) = self.by_input.get(obj) {
            return idxs.iter().map(|&i| &self.events[i]).collect();
        }
        if self.by_input.is_empty() && !self.events.is_empty() {
            return self
                .events
                .iter()
                .filter(|e| e.input.as_deref() == Some(obj))
                .collect();
        }
        Vec::new()
    }

    /// Upstream chain: the event that produced `object`, then whatever produced
    /// its input, and so on. Cycles are impossible by construction (a derived
    /// artifact is recorded after its parent) but the visited set guards a
    /// hand-edited store anyway.
    pub fn lineage(&self, object: &str) -> Vec<&CauseEvent> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut cursor = Some(object.to_string());
        while let Some(obj) = cursor.take() {
            if !seen.insert(obj.clone()) {
                break;
            }
            let Some(ev) = self
                .events_with_object(&obj)
                .into_iter()
                .filter(|e| e.mechanism != Mechanism::Serves)
                .max_by_key(|e| e.seq)
            else {
                break;
            };
            cursor = ev.input.clone();
            out.push(ev);
        }
        out
    }

    /// Downstream: everything produced *from* `object` (directly or transitively).
    /// Breadth-first over the input index — O(V+E), not the O(n²) fixpoint.
    pub fn descendants(&self, object: &str) -> Vec<&CauseEvent> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::from([object.to_string()]);
        let mut queue = vec![object.to_string()];
        while let Some(obj) = queue.pop() {
            for ev in self
                .events_with_input(&obj)
                .into_iter()
                .filter(|e| e.mechanism != Mechanism::Serves)
            {
                if seen.insert(ev.object.clone()) {
                    out.push(ev);
                    queue.push(ev.object.clone());
                }
            }
        }
        out.sort_by_key(|e| e.seq);
        out
    }

    /// Purpose links (`serves` events) recorded against `object`.
    pub fn purposes(&self, object: &str) -> Vec<&CauseEvent> {
        self.events
            .iter()
            .filter(|e| e.mechanism == Mechanism::Serves && e.object == object)
            .collect()
    }

    /// Project onto the operational timeline (study.rs `to_op_event`
    /// convention). Provenance connector `cause`; never mixes with shop-floor
    /// machine events except through the shared pipeline.
    pub fn to_op_events(&self) -> Vec<OperationalEvent> {
        self.events
            .iter()
            .map(|ev| {
                let status = match ev.mechanism {
                    Mechanism::Generated | Mechanism::Derived => OperationalStatus::Success,
                    Mechanism::Corrected => OperationalStatus::Warning,
                    Mechanism::Received => OperationalStatus::Info,
                    Mechanism::Cited | Mechanism::Serves => OperationalStatus::Inert,
                };
                let mut props = std::collections::HashMap::new();
                props.insert("mechanism".to_string(), ev.mechanism.as_str().to_string());
                props.insert("object".to_string(), ev.object.clone());
                if let Some(input) = &ev.input {
                    props.insert("input".to_string(), input.clone());
                }
                let subject = ev
                    .object
                    .rsplit('/')
                    .next()
                    .unwrap_or(&ev.object)
                    .to_string();
                let mut oev = OperationalEvent::new(
                    OperationalKind::Document,
                    status,
                    ev.at,
                    subject,
                    OperationalProvenance::new("cause", "artifact"),
                );
                oev.actor = Some(ev.actor.clone());
                oev.target = ev.input.clone();
                oev.properties = props;
                oev.event_key = format!("cause:{}:{}", ev.seq, ev.at.timestamp());
                oev
            })
            .collect()
    }

    // ── persistence ────────────────────────────────────────────────────────
    // One sled row per event, keyed `ledger:{uid}:{seq}` (mirrors core.rs
    // node_key scoping): per-tenant isolation falls out of the key prefix, and
    // appending writes only the new row instead of rewriting history.

    const PREFIX: &'static str = "ledger";

    fn key(uid: &str, seq: u64) -> String {
        format!("{}:{}:{:020}", Self::PREFIX, uid, seq)
    }

    pub fn save(&self, db: &sled::Db, uid: &str) -> anyhow::Result<()> {
        for ev in &self.events {
            let k = Self::key(uid, ev.seq);
            if db.contains_key(k.as_bytes())? {
                continue; // append-only: never rewrite a recorded row
            }
            db.insert(k.as_bytes(), serde_json::to_vec(ev)?)?;
        }
        Ok(())
    }

    pub fn load(db: &sled::Db, uid: &str) -> anyhow::Result<Self> {
        let prefix = format!("{}:{}:", Self::PREFIX, uid);
        let mut events = Vec::new();
        for row in db.scan_prefix(prefix.as_bytes()) {
            let (_, v) = row?;
            events.push(serde_json::from_slice::<CauseEvent>(&v)?);
        }
        events.sort_by_key(|e| e.seq);
        let next_seq = events.last().map_or(0, |e| e.seq + 1);
        let mut ledger = Self {
            events,
            next_seq,
            by_object: Default::default(),
            by_input: Default::default(),
        };
        ledger.build_index();
        Ok(ledger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).single().unwrap()
    }

    #[test]
    fn record_appends_with_monotonic_sequence() {
        let mut l = CausalLedger::new();
        l.record(Mechanism::Generated, "watch", "/p/a.md", None);
        l.record(
            Mechanism::Derived,
            "report",
            "/p/report.md",
            Some("/p/a.md".into()),
        );
        assert_eq!(l.len(), 2);
        assert_eq!(l.events[0].seq, 0);
        assert_eq!(l.events[1].seq, 1);
        assert_eq!(l.events[1].at.timestamp(), l.events[1].at.timestamp());
    }

    #[test]
    fn record_at_keeps_seq_monotonic_while_time_points_backwards() {
        let mut l = CausalLedger::new();
        l.record(Mechanism::Generated, "watch", "today.md", None);
        // A month-old letter imported today was received a month ago.
        let old = ts(1_700_000_000);
        l.record_at(Mechanism::Received, "mailbox", "old.eml", None, old);
        assert_eq!(l.events[1].seq, 1, "seq stays monotonic");
        assert_eq!(l.events[1].at, old, "at carries the artifact's own clock");
        // Projections preserve recording order; chronological ordering happens
        // where the timeline is assembled (OperationalTimeline::from_events).
        let evs = l.to_op_events();
        assert_eq!(evs.len(), 2);
        let tl = crate::operational_report::OperationalTimeline::from_events(&evs);
        let times: Vec<i64> = tl
            .entries
            .iter()
            .map(|e| e.observed_at.timestamp())
            .collect();
        let mut sorted = times.clone();
        sorted.sort_unstable();
        assert_eq!(times, sorted, "timeline assembly respects native clocks");
    }

    #[test]
    fn lineage_walks_upstream_through_inputs() {
        let mut l = CausalLedger::new();
        l.record(Mechanism::Received, "vault", "note.md", None);
        l.record(Mechanism::Generated, "watch", "scan.json", None);
        l.record(
            Mechanism::Derived,
            "pack",
            "weekly-pack.md",
            Some("note.md".into()),
        );
        l.record(
            Mechanism::Derived,
            "export",
            "bundle.zip",
            Some("weekly-pack.md".into()),
        );
        let chain: Vec<&str> = l
            .lineage("bundle.zip")
            .iter()
            .map(|e| e.object.as_str())
            .collect();
        assert_eq!(chain, ["bundle.zip", "weekly-pack.md", "note.md"]);
        // Descendants run the other direction.
        let downs: Vec<&str> = l
            .descendants("note.md")
            .iter()
            .map(|e| e.object.as_str())
            .collect();
        assert_eq!(downs, ["weekly-pack.md", "bundle.zip"]);
        // Root artifacts have no upstream beyond themselves.
        assert_eq!(l.lineage("scan.json").len(), 1);
    }

    #[test]
    fn purposes_record_final_cause_without_polluting_lineage() {
        let mut l = CausalLedger::new();
        l.record(Mechanism::Generated, "watch", "spec.md", None);
        l.record(
            Mechanism::Serves,
            "planner",
            "spec.md",
            Some("goal-1".into()),
        );
        assert_eq!(l.purposes("spec.md").len(), 1);
        // A purpose link is not a derivation: lineage stops at the artifact.
        assert_eq!(l.lineage("goal-1").len(), 0);
    }

    #[test]
    fn projection_carries_cause_provenance_and_stable_keys() {
        let mut l = CausalLedger::new();
        l.record(Mechanism::Corrected, "watch", "/plant/line1/log.txt", None);
        let evs = l.to_op_events();
        assert_eq!(evs.len(), 1);
        let e = &evs[0];
        assert_eq!(e.provenance.connector, "cause");
        assert_eq!(e.status, OperationalStatus::Warning);
        assert_eq!(e.subject, "log.txt");
        assert_eq!(e.properties.get("mechanism").unwrap(), "corrected");
        // Replay stability: same seq → same event_key, twice.
        assert_eq!(
            e.event_key,
            format!("cause:{}:{}", 0, e.observed_at.timestamp())
        );
        assert_eq!(l.to_op_events()[0].event_key, e.event_key);
        let _ = ts(0);
    }

    #[test]
    fn sled_round_trip_is_per_tenant_and_replay_stable() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let mut alice = CausalLedger::new();
        alice.record(Mechanism::Generated, "watch", "a.md", None);
        alice.record(Mechanism::Received, "vault", "b.md", None);
        alice.save(&db, "alice").unwrap();

        let mut bob = CausalLedger::new();
        bob.record(Mechanism::Cited, "node", "c.md", None);
        bob.save(&db, "bob").unwrap();

        let a2 = CausalLedger::load(&db, "alice").unwrap();
        assert_eq!(a2.len(), 2, "tenant isolation: alice sees only her rows");
        assert_eq!(a2.events[0].object, "a.md");
        // Appending after load continues the sequence, never restarts it.
        let mut a3 = a2.clone();
        a3.record(Mechanism::Derived, "pack", "d.md", None);
        assert_eq!(a3.events[2].seq, 2);

        let b2 = CausalLedger::load(&db, "bob").unwrap();
        assert_eq!(b2.len(), 1);

        // Unknown tenant → empty, not an error.
        assert!(CausalLedger::load(&db, "carol").unwrap().is_empty());

        // save() twice does not duplicate rows.
        alice.save(&db, "alice").unwrap();
        assert_eq!(CausalLedger::load(&db, "alice").unwrap().len(), 2);
    }

    #[test]
    fn walks_agree_over_index_and_fallback_scan() {
        let mut l = CausalLedger::new();
        l.record(Mechanism::Received, "vault", "note.md", None);
        l.record(Mechanism::Generated, "watch", "scan.json", None);
        l.record(
            Mechanism::Derived,
            "pack",
            "weekly-pack.md",
            Some("note.md".into()),
        );
        l.record(
            Mechanism::Derived,
            "export",
            "bundle.zip",
            Some("weekly-pack.md".into()),
        );
        l.record(
            Mechanism::Serves,
            "planner",
            "bundle.zip",
            Some("goal-9".into()),
        );

        let indexed = |l: &CausalLedger| {
            (
                l.lineage("bundle.zip")
                    .iter()
                    .map(|e| (e.seq, e.object.clone()))
                    .collect::<Vec<_>>(),
                l.descendants("note.md")
                    .iter()
                    .map(|e| (e.seq, e.object.clone()))
                    .collect::<Vec<_>>(),
                l.purposes("bundle.zip").len(),
            )
        };

        // Index is fresh here.
        let fresh = indexed(&l);
        assert_eq!(
            fresh.0,
            [
                (3, "bundle.zip".into()),
                (2, "weekly-pack.md".into()),
                (0, "note.md".into())
            ],
            "lineage walks upstream from the object, serves excluded"
        );
        assert_eq!(
            fresh.1,
            [(2, "weekly-pack.md".into()), (3, "bundle.zip".into())]
        );

        // A serde round-trip drops the skip-ed indexes; the fallback scan must
        // return the identical chains.
        let json = serde_json::to_vec(&l).unwrap();
        let restored: CausalLedger = serde_json::from_slice(&json).unwrap();
        assert_eq!(indexed(&restored), fresh, "fallback scan matches the index");
    }
}
