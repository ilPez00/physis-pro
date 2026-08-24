//! Accumulated interpretant evidence: which signs actually earned their keep.
//!
//! An ontology entry starts as an assertion by whoever authored it. Salience is
//! what the entry has since been worth: every observed outcome moves it by the
//! governing [`Interpretant`]'s `delta`, and time moves it back toward zero by
//! that interpretant's half-life. An entry nobody's work confirms fades; one
//! that keeps being confirmed rises.
//!
//! Two decisions shape this module.
//!
//! **The signal is file retention and editing**, not a self-report. Asking the
//! user to grade each classification would produce a trickle of judgements
//! biased toward the moments they felt like grading. Editing a file that was
//! classified into a cell is evidence the frame was useful, and it is emitted
//! by work the user was doing anyway. Deleting it is evidence the frame was
//! not. That is the difference between a taxonomy the user maintains and one
//! that maintains itself.
//!
//! **Salience biases the score**, it does not merely break ties. A
//! well-confirmed entry can therefore beat a semantically closer one. That is
//! real adaptation, and it is also how a system entrenches its own past, so the
//! shift is bounded: see [`BIAS_STRENGTH`].
//!
//! Decay is applied lazily, on read, from `updated_at`. Eager decay would need
//! a sweep over every entry on a timer, and the engine has no scheduler.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::models::Interpretant;

/// Seconds in a day, as f32, for decay arithmetic.
const DAY: f32 = 86_400.0;

/// Maximum cosine shift salience may apply, at full weight and saturated score.
///
/// Cosine similarities across this corpus sit around 0.55–0.75 and cells are
/// separated by far less than that — the measured gap between a coherent cell
/// and a random pair is about 0.15. A bias of 0.05 is therefore enough to
/// reorder genuinely close calls and too small to overturn a clear semantic
/// win. Raising it past ~0.10 would let history outvote meaning, which is the
/// failure mode of every system that learns from its own output.
pub const BIAS_STRENGTH: f32 = 0.05;

/// Observations saturate: `tanh(score / SATURATION)`. Ten net confirmations
/// gets an entry ~76% of the available bias; a hundred does not get it ten
/// times more. Without saturation one heavily-edited file would pin a cell
/// permanently.
const SATURATION: f32 = 8.0;

/// What was observed about a classified item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// The file was edited after being classified here. The frame was useful.
    Edited,
    /// The file is still present at a later scan. Weak, but real, evidence.
    Retained,
    /// The file was deleted. The frame did not survive contact.
    Deleted,
}

impl Signal {
    /// Map a file event onto the outcome vocabulary an [`Interpretant`] speaks.
    ///
    /// `Retained` maps to `inert` deliberately: mere survival is not success.
    /// Whether it nonetheless confirms is the ontology's call — a machine
    /// ontology says an inspection that found nothing still confirms the
    /// routine, a software ontology says a change that did nothing does not.
    pub fn outcome(self) -> &'static str {
        match self {
            Signal::Edited => "success",
            Signal::Retained => "inert",
            Signal::Deleted => "failure",
        }
    }
}

/// One entry's accumulated evidence, before decay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accrued {
    /// Net delta as of `updated_at`. Decay is applied when read.
    pub score: f32,
    /// Unix seconds of the last observation folded into `score`.
    pub updated_at: i64,
    /// Total observations, for the studio. Never decays — it is a count of
    /// what happened, not a measure of how much it currently matters.
    pub observations: u32,
}

/// Persisted salience, keyed by context and entry.
///
/// Keying by context is what stops one client's evidence from biasing another
/// client's classification: the same entry name means different things in
/// different contexts, so what confirmed it in one is not evidence about the
/// other.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SalienceStore {
    /// `"<context label>\u{0}<entry name>" -> accrued`.
    entries: HashMap<String, Accrued>,
}

impl SalienceStore {
    /// Key by tenant **and** context.
    ///
    /// The context alone is not enough. The server is multi-tenant via
    /// `X-Physis-User`, while the ambient context comes from `PHYSIS_CONTEXT` —
    /// one process-wide value. Keying on context only would let one customer's
    /// editing habits bias another customer's classification, and the effect is
    /// observable to them as a changed ranking. Evidence is only evidence about
    /// the tenant who produced it.
    fn key(tenant: &str, ctx: &Context, entry: &str) -> String {
        format!("{tenant}\u{0}{}\u{0}{entry}", ctx.label())
    }

    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("salience.json")
    }

    pub fn load(data_dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(data_dir))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Atomic write, for the reason documented on `pack::EmbedCache::save`: a
    /// truncating write that fails leaves an empty file and silently discards
    /// every observation ever recorded.
    pub fn save(&self, data_dir: &Path) {
        if let Err(e) = self.try_save(data_dir) {
            eprintln!(
                "physis: could not write salience to {} — {e}. Observations from \
                 this run are lost; earlier ones are intact.",
                Self::path(data_dir).display()
            );
        }
    }

    fn try_save(&self, data_dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Accumulated observations cannot be recomputed from anything, so this
        // one pays for the directory sync.
        crate::atomic::write_durable(&Self::path(data_dir), &bytes)
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// Fold one observation in, decaying what was already there first.
    ///
    /// Decaying before adding is what makes the accumulator order-independent
    /// in time: adding today's `+1` to a year-old `+10` undecayed would treat
    /// stale evidence as current.
    pub fn record(
        &mut self,
        tenant: &str,
        ctx: &Context,
        entry: &str,
        signal: Signal,
        interp: &Interpretant,
    ) {
        let delta = interp.delta(signal.outcome());
        let now = Self::now();
        let e = self
            .entries
            .entry(Self::key(tenant, ctx, entry))
            .or_insert(Accrued {
                score: 0.0,
                updated_at: now,
                observations: 0,
            });
        let days = ((now - e.updated_at).max(0) as f32) / DAY;
        e.score = e.score * interp.decay(days) + delta;
        e.updated_at = now;
        e.observations = e.observations.saturating_add(1);
    }

    /// Decayed salience for one entry, as of now. `0.0` when never observed.
    pub fn score(&self, tenant: &str, ctx: &Context, entry: &str, interp: &Interpretant) -> f32 {
        let Some(e) = self.entries.get(&Self::key(tenant, ctx, entry)) else {
            return 0.0;
        };
        let days = ((Self::now() - e.updated_at).max(0) as f32) / DAY;
        e.score * interp.decay(days)
    }

    pub fn observations(&self, tenant: &str, ctx: &Context, entry: &str) -> u32 {
        self.entries
            .get(&Self::key(tenant, ctx, entry))
            .map(|e| e.observations)
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bounded score adjustment for one entry.
    ///
    /// `weight` comes from the interpretant, so an ontology can record outcomes
    /// without letting them influence ranking by declaring `weight: 0`.
    pub fn bias(&self, tenant: &str, ctx: &Context, entry: &str, interp: &Interpretant) -> f32 {
        if interp.weight <= 0.0 {
            return 0.0;
        }
        let s = self.score(tenant, ctx, entry, interp);
        BIAS_STRENGTH * interp.weight * (s / SATURATION).tanh()
    }

    /// Apply salience to a cell's score.
    ///
    /// A cell is scored by its *best* member, so the bias is the mean over the
    /// members that actually carry evidence rather than over all of them.
    /// Averaging across every member would let a large cell dilute a strong
    /// signal to nothing — and the large cells are already the incoherent ones.
    pub fn cell_bias(
        &self,
        tenant: &str,
        ctx: &Context,
        entries: &[String],
        interp: &Interpretant,
    ) -> f32 {
        let biases: Vec<f32> = entries
            .iter()
            .map(|e| self.bias(tenant, ctx, e, interp))
            .filter(|b| *b != 0.0)
            .collect();
        if biases.is_empty() {
            return 0.0;
        }
        biases.iter().sum::<f32>() / biases.len() as f32
    }

    /// Every observed entry in a context, strongest first, for the studio.
    pub fn ranked(
        &self,
        tenant: &str,
        ctx: &Context,
        interp: &Interpretant,
    ) -> Vec<(String, f32, u32)> {
        let prefix = format!("{tenant}\u{0}{}\u{0}", ctx.label());
        let mut out: Vec<(String, f32, u32)> = self
            .entries
            .iter()
            .filter_map(|(k, v)| {
                let name = k.strip_prefix(&prefix)?;
                let days = ((Self::now() - v.updated_at).max(0) as f32) / DAY;
                Some((
                    name.to_string(),
                    v.score * interp.decay(days),
                    v.observations,
                ))
            })
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tenant used by every test that is not about tenant isolation.
    const T: &str = "tenant-a";

    fn ctx() -> Context {
        Context::parse("client:evotec")
    }

    #[test]
    fn editing_confirms_and_deleting_refutes() {
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        s.record(T, &ctx(), "Debugging", Signal::Edited, &i);
        assert!(s.score(T, &ctx(), "Debugging", &i) > 0.0);

        s.record(T, &ctx(), "Refactoring", Signal::Deleted, &i);
        assert!(s.score(T, &ctx(), "Refactoring", &i) < 0.0);
    }

    #[test]
    fn what_retention_means_is_the_ontologys_call() {
        // The same Retained signal, read two ways. This is the whole point of
        // per-ontology interpretants: survival confirms a maintenance routine
        // and says nothing about a code change.
        let plant = Interpretant {
            confirms: vec!["success".into(), "inert".into()],
            refutes: vec!["failure".into()],
            ..Default::default()
        };
        let code = Interpretant::default(); // inert is neither

        let mut a = SalienceStore::default();
        a.record(T, &ctx(), "Inspection", Signal::Retained, &plant);
        assert!(a.score(T, &ctx(), "Inspection", &plant) > 0.0);

        let mut b = SalienceStore::default();
        b.record(T, &ctx(), "Refactoring", Signal::Retained, &code);
        assert_eq!(b.score(T, &ctx(), "Refactoring", &code), 0.0);
        // Still counted as an observation — it happened, it just did not move
        // the score.
        assert_eq!(b.observations(T, &ctx(), "Refactoring"), 1);
    }

    #[test]
    fn salience_is_scoped_to_its_context() {
        // Evidence from one client must not bias another. Same entry name,
        // different universe of discourse.
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        let evotec = Context::parse("client:evotec");
        let publisher = Context::parse("client:publisher");

        s.record(T, &evotec, "Margin", Signal::Edited, &i);
        assert!(s.score(T, &evotec, "Margin", &i) > 0.0);
        assert_eq!(s.score(T, &publisher, "Margin", &i), 0.0);
    }

    #[test]
    fn one_tenants_evidence_never_reaches_another() {
        // The server is multi-tenant via X-Physis-User while the context comes
        // from one process-wide env var, so context alone would let a
        // customer's editing habits move another customer's ranking — visible
        // to them as a changed result.
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        let c = ctx();

        for _ in 0..30 {
            s.record("tenant-a", &c, "Debugging", Signal::Edited, &i);
        }
        assert!(s.score("tenant-a", &c, "Debugging", &i) > 0.0);
        assert_eq!(s.score("tenant-b", &c, "Debugging", &i), 0.0);
        assert_eq!(s.bias("tenant-b", &c, "Debugging", &i), 0.0);
        assert_eq!(s.observations("tenant-b", &c, "Debugging"), 0);
        assert!(s.ranked("tenant-b", &c, &i).is_empty());
    }

    #[test]
    fn the_default_tenant_is_not_a_wildcard() {
        // "_default" is the shared global tenant. It must be an ordinary key,
        // not one that sees or is seen by named tenants.
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        let c = ctx();
        s.record("_default", &c, "Debugging", Signal::Edited, &i);
        assert!(s.score("_default", &c, "Debugging", &i) > 0.0);
        assert_eq!(s.score("tenant-a", &c, "Debugging", &i), 0.0);
    }

    #[test]
    fn bias_is_bounded_and_saturates() {
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        for _ in 0..500 {
            s.record(T, &ctx(), "Debugging", Signal::Edited, &i);
        }
        let b = s.bias(T, &ctx(), "Debugging", &i);
        assert!(b > 0.0);
        assert!(
            b <= BIAS_STRENGTH + 1e-6,
            "500 confirmations must not exceed the cap: {b}"
        );
    }

    #[test]
    fn a_zero_weight_interpretant_records_but_never_biases() {
        let mut s = SalienceStore::default();
        let i = Interpretant {
            weight: 0.0,
            ..Default::default()
        };
        s.record(T, &ctx(), "Debugging", Signal::Edited, &i);
        assert!(s.score(T, &ctx(), "Debugging", &i) > 0.0, "still recorded");
        assert_eq!(
            s.bias(T, &ctx(), "Debugging", &i),
            0.0,
            "but cannot influence ranking"
        );
    }

    #[test]
    fn stale_evidence_decays_toward_zero() {
        let mut s = SalienceStore::default();
        let i = Interpretant {
            half_life_days: 30.0,
            ..Default::default()
        };
        s.record(T, &ctx(), "Debugging", Signal::Edited, &i);

        // Backdate the observation by two half-lives.
        let k = SalienceStore::key(T, &ctx(), "Debugging");
        s.entries.get_mut(&k).unwrap().updated_at -= (60.0 * DAY) as i64;

        let decayed = s.score(T, &ctx(), "Debugging", &i);
        assert!(
            (decayed - 0.25).abs() < 1e-3,
            "expected ~0.25, got {decayed}"
        );
    }

    #[test]
    fn an_ontology_that_never_decays_keeps_its_evidence() {
        let mut s = SalienceStore::default();
        let i = Interpretant {
            half_life_days: 0.0,
            ..Default::default()
        };
        s.record(T, &ctx(), "Inspection", Signal::Edited, &i);
        let k = SalienceStore::key(T, &ctx(), "Inspection");
        s.entries.get_mut(&k).unwrap().updated_at -= (3650.0 * DAY) as i64;
        assert_eq!(
            s.score(T, &ctx(), "Inspection", &i),
            1.0,
            "ten years, no decay"
        );
    }

    #[test]
    fn cell_bias_averages_only_the_members_carrying_evidence() {
        // A large cell must not dilute a real signal to nothing — the large
        // cells are already the least coherent ones.
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        for _ in 0..20 {
            s.record(T, &ctx(), "Debugging", Signal::Edited, &i);
        }
        let members: Vec<String> = std::iter::once("Debugging".to_string())
            .chain((0..40).map(|n| format!("Unobserved{n}")))
            .collect();

        let solo = s.cell_bias(T, &ctx(), &["Debugging".to_string()], &i);
        let diluted = s.cell_bias(T, &ctx(), &members, &i);
        assert!(
            (solo - diluted).abs() < 1e-6,
            "40 silent members must not dilute"
        );
    }

    #[test]
    fn a_failed_save_leaves_earlier_observations_intact() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        s.record(T, &ctx(), "Debugging", Signal::Edited, &i);
        s.save(dir.path());
        let before = std::fs::read(SalienceStore::path(dir.path())).unwrap();
        assert!(!before.is_empty());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
            let mut more = s.clone();
            more.record(T, &ctx(), "Refactoring", Signal::Edited, &i);
            more.save(dir.path()); // reports on stderr, must not panic
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        assert_eq!(
            std::fs::read(SalienceStore::path(dir.path())).unwrap(),
            before
        );
        assert_eq!(
            SalienceStore::load(dir.path()).observations(T, &ctx(), "Debugging"),
            1
        );
    }

    #[test]
    fn ranked_lists_a_contexts_entries_strongest_first() {
        let mut s = SalienceStore::default();
        let i = Interpretant::default();
        for _ in 0..3 {
            s.record(T, &ctx(), "Debugging", Signal::Edited, &i);
        }
        s.record(T, &ctx(), "Profiling", Signal::Edited, &i);
        s.record(T, &ctx(), "Vendoring", Signal::Deleted, &i);
        s.record(
            T,
            &Context::parse("client:other"),
            "Elsewhere",
            Signal::Edited,
            &i,
        );

        let r = s.ranked(T, &ctx(), &i);
        assert_eq!(r.len(), 3, "only this context's entries");
        assert_eq!(r[0].0, "Debugging");
        assert_eq!(r[0].2, 3, "observation count survives");
        assert_eq!(r.last().unwrap().0, "Vendoring");
        assert!(r.last().unwrap().1 < 0.0);
    }
}
