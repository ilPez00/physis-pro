//! The model, and the settings that decide whether there is one.
//!
//! The editor starts with no model at all. Whether it ever gets one is the
//! operator's call, because the cost is theirs: building the session takes
//! seconds and hundreds of megabytes, and on a machine without a GPU that is a
//! decision rather than a detail.
//!
//! [`Settings::model`] is that decision:
//!
//! - [`ModelPolicy::Off`] — no model, ever. Identity, the diff, the causal
//!   ledger, the graph, the files pane and the terminal all keep working. No
//!   vectors are stored, so the corpus has nothing to relate.
//! - [`ModelPolicy::Lazy`] — the default. A model is loaded when a diff
//!   produces something the exact evidence could not account for, and not
//!   otherwise.
//! - [`ModelPolicy::Eager`] — warm on open and read the whole document, so the
//!   corpus sees it even if nothing in it was ever ambiguous.
//!
//! [`Settings::embedder`] is the second half of the hardware decision: leaving
//! it empty lets the engine pick its best candidate (ONNX, semantics), and
//! `random-projection` selects the offline model-free embedder — instant,
//! deterministic, and coarser. That one never touches a model file at all.
//!
//! When a model is used, it is used from one resident thread. The session and
//! the classifier are built once and reused for every later job, because
//! rebuilding them per request was the cost that made the earlier one-shot
//! design slow under load.
//!
//! Nothing here is allowed to influence identity. It runs after a transition
//! has already been recorded, and what it returns is attached to the record.

use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};

use serde::{Deserialize, Serialize};

use physis_pro::adaptive_document::observation::{
    round_vector, Observation, NEIGHBOUR_FLOOR, NEIGHBOURS,
};
use physis_pro::adaptive_document::BlockId;
use physis_pro::config::{OntologyLoader, PhysisProConfig};
use physis_pro::embed::{select_embedder, RandomProjectionEmbedder, VectorEmbed};

/// How much a model is allowed to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPolicy {
    /// Never load one.
    Off,
    /// Load one only where the exact evidence ran out. The default, and the
    /// only one that costs nothing until it is needed.
    #[default]
    Lazy,
    /// Warm on open, and read the whole document so the corpus sees it.
    Eager,
}

impl ModelPolicy {
    pub const ALL: [ModelPolicy; 3] = [ModelPolicy::Off, ModelPolicy::Lazy, ModelPolicy::Eager];

    pub fn label(self) -> &'static str {
        match self {
            ModelPolicy::Off => "off — never load one",
            ModelPolicy::Lazy => "lazy — only where the diff ran out of evidence",
            ModelPolicy::Eager => "eager — warm on open and read the whole document",
        }
    }

    pub fn next(self) -> ModelPolicy {
        let i = ModelPolicy::ALL.iter().position(|p| *p == self).unwrap_or(0);
        ModelPolicy::ALL[(i + 1) % ModelPolicy::ALL.len()]
    }
}

/// Operator decisions, persisted next to the stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub model: ModelPolicy,
    /// Empty lets the engine pick. `random-projection` is the offline,
    /// model-free embedder: instant and deterministic, coarser semantics.
    #[serde(default)]
    pub embedder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            model: ModelPolicy::Lazy,
            embedder: String::new(),
        }
    }
}

impl Settings {
    /// The embedder choices, as values. Empty is the engine's own pick.
    pub const EMBEDDERS: [&'static str; 3] = ["", "random-projection", "bge-base"];

    pub fn load(path: &Path) -> Settings {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        physis_pro::atomic::write_json_durable(path, self)
    }

    pub fn embedder_label(&self) -> &str {
        if self.embedder.is_empty() {
            "engine default"
        } else {
            &self.embedder
        }
    }

    /// Whether a model may be built at all.
    pub fn may_load(&self) -> bool {
        self.model != ModelPolicy::Off
    }
}

/// A block the caller wants read, with the text to read.
pub struct Pending {
    pub block_id: BlockId,
    pub content_hash: u64,
    pub text: String,
}

pub enum Reply {
    /// Observations for the blocks that were sent, in the order they arrived.
    Ready(Vec<Observation>),
    /// Corpus entries ranked against a query, best first.
    Ranked(Vec<(usize, f32)>),
    /// The model is built and resident. Sent once, before the first real reply.
    Warm(String),
    /// The model could not be built. The document is unaffected; this is why
    /// nothing was said about it.
    Failed(String),
}

/// One unit of work for the resident thread.
enum Job {
    Annotate { pending: Vec<Pending>, revision: u64 },
    Search { query: String, entries: Vec<(usize, Vec<f32>)> },
    /// Build the model and report its name, so the first real job is instant.
    Warm,
}

/// The model, built once and kept by the thread that owns it.
struct Resident {
    embedder: Box<dyn VectorEmbed>,
    kind: String,
    classifier: physis_pro::classify::CellClassifier,
}

/// What the worker is doing, for a status line.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// The settings forbid a model.
    Off,
    /// No job in flight and no session built.
    Idle,
    Working,
    Ready(String),
    Failed(String),
}

pub struct Worker {
    jobs: Option<Sender<Job>>,
    replies: Option<Receiver<Reply>>,
    pub settings: Settings,
    pub label: String,
    /// The last search query: a reply carries results, not the words that
    /// produced them.
    pub query: Option<String>,
    /// Replies still expected. A warm-up and its job both report, so a boolean
    /// would clear too early.
    in_flight: usize,
    pub state: State,
}

impl Worker {
    pub fn new(settings: Settings) -> Worker {
        let may_load = settings.may_load();
        let state = if may_load { State::Idle } else { State::Off };
        Worker {
            jobs: None,
            replies: None,
            settings,
            label: if may_load {
                "no model loaded".into()
            } else {
                "model off (settings: ,)".into()
            },
            query: None,
            in_flight: 0,
            state,
        }
    }

    /// Apply a settings change.
    ///
    /// The resident thread captures the settings when it starts, so a change to
    /// the *embedder* has to drop it: the session and the classifier were built
    /// with the old one and there is no way to swap an embedder underneath
    /// them. Turning the model off drops it too, so the memory goes back rather
    /// than sitting there unused. Dropping the job sender is what ends the
    /// thread.
    pub fn set_settings(&mut self, settings: Settings) {
        let was = self.settings.may_load();
        let embedder_changed = self.settings.embedder != settings.embedder;
        self.settings = settings;
        if !self.settings.may_load() || embedder_changed {
            self.jobs = None;
            self.replies = None;
            self.in_flight = 0;
            self.query = None;
            self.state = if self.settings.may_load() {
                State::Idle
            } else {
                State::Off
            };
            self.label = if self.settings.may_load() {
                format!(
                    "model reset — will build [{}] on the next job",
                    self.settings.embedder_label()
                )
            } else {
                "model off (settings: ,)".into()
            };
        } else if !was {
            self.state = State::Idle;
            self.label = "no model loaded".into();
        }
    }

    pub fn busy(&self) -> bool {
        self.in_flight > 0
    }

    fn ensure_thread(&mut self) -> bool {
        if self.jobs.is_some() {
            return true;
        }
        if !self.settings.may_load() {
            return false;
        }
        let (job_tx, job_rx) = channel::<Job>();
        let (reply_tx, reply_rx) = channel::<Reply>();
        let settings = self.settings.clone();
        std::thread::spawn(move || run(job_rx, reply_tx, settings));
        self.jobs = Some(job_tx);
        self.replies = Some(reply_rx);
        true
    }

    fn send(&mut self, job: Job) -> bool {
        if !self.ensure_thread() {
            return false;
        }
        let Some(tx) = self.jobs.as_ref() else { return false };
        if tx.send(job).is_err() {
            self.state = State::Failed("the model thread stopped".into());
            return false;
        }
        self.in_flight += 1;
        self.state = State::Working;
        true
    }

    /// Build the session now, so the first real job is instant.
    pub fn warm(&mut self) {
        if self.busy() {
            return;
        }
        if self.send(Job::Warm) {
            self.label = format!(
                "warming the model [{}]",
                self.settings.embedder_label()
            );
        }
    }

    /// Send work to the resident thread.
    pub fn request(&mut self, pending: Vec<Pending>, revision: u64, label: String) {
        if pending.is_empty() {
            return;
        }
        if self.send(Job::Annotate { pending, revision }) {
            self.label = label;
        }
    }

    pub fn request_search(&mut self, query: String, entries: Vec<(usize, Vec<f32>)>, label: String) {
        if entries.is_empty() {
            return;
        }
        if self.send(Job::Search {
            query: query.clone(),
            entries,
        }) {
            self.label = label;
            self.query = Some(query);
        }
    }

    /// Non-blocking poll. `None` means nothing has come back yet.
    pub fn poll(&mut self) -> Option<Reply> {
        let rx = self.replies.as_ref()?;
        match rx.try_recv() {
            Ok(reply) => {
                self.in_flight = self.in_flight.saturating_sub(1);
                match &reply {
                    Reply::Warm(kind) => {
                        self.state = State::Ready(kind.clone());
                        if self.in_flight == 0 {
                            self.label = format!("model ready [{kind}]");
                        }
                    }
                    Reply::Failed(e) => {
                        self.state = State::Failed(e.clone());
                        self.label = format!("model unavailable: {e}");
                    }
                    _ => {
                        if self.in_flight == 0 {
                            self.state = State::Idle;
                        }
                    }
                }
                Some(reply)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.in_flight = 0;
                self.state = State::Failed("the model thread stopped".into());
                Some(Reply::Failed("the model thread stopped".into()))
            }
        }
    }
}

/// The resident thread: build the model on the first job, then serve jobs with
/// it. This is the whole reason the session is not rebuilt per request.
fn run(jobs: Receiver<Job>, replies: Sender<Reply>, settings: Settings) {
    let mut resident: Option<Resident> = None;
    while let Ok(job) = jobs.recv() {
        if resident.is_none() {
            match build(&settings) {
                Ok(r) => {
                    let _ = replies.send(Reply::Warm(r.kind.clone()));
                    resident = Some(r);
                }
                Err(e) => {
                    let _ = replies.send(Reply::Failed(e));
                    continue;
                }
            }
        }
        let Some(r) = resident.as_ref() else { continue };
        let reply = match job {
            Job::Warm => None,
            Job::Annotate { pending, revision } => {
                Some(Reply::Ready(annotate(r, &pending, revision)))
            }
            Job::Search { query, entries } => Some(Reply::Ranked(rank(r, &query, entries))),
        };
        if let Some(reply) = reply {
            let _ = replies.send(reply);
        }
    }
}

fn build(settings: &Settings) -> Result<Resident, String> {
    let config = PhysisProConfig::default();
    // The embedder choice is honoured explicitly rather than through the
    // environment: `PHYSIS_EMBEDDER` is process-wide and this is one pane.
    let (embedder, kind): (Box<dyn VectorEmbed>, String) =
        if settings.embedder == "random-projection" {
            (
                Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim)),
                "random-projection".to_string(),
            )
        } else {
            let (e, k) = select_embedder(&config.physis);
            (e, k.to_string())
        };
    let ontology = OntologyLoader::load_all(&config.physis);
    let mut cache = physis_pro::pack::EmbedCache::load(&config.physis.data_dir);
    let classifier = physis_pro::classify::CellClassifier::build_cached(
        &ontology,
        embedder.as_ref(),
        &kind,
        &mut cache,
    );
    cache.save(&config.physis.data_dir);
    Ok(Resident {
        embedder,
        kind,
        classifier,
    })
}

fn annotate(r: &Resident, pending: &[Pending], revision: u64) -> Vec<Observation> {
    // Embed every block once: the neighbours are a statement about this
    // document, so they need the document, not just the new block.
    let mut vectors: Vec<(BlockId, Vec<f32>)> = Vec::with_capacity(pending.len());
    for p in pending {
        vectors.push((p.block_id.clone(), r.embedder.embed(&p.text)));
    }
    let mut observations = Vec::with_capacity(pending.len());
    for (i, p) in pending.iter().enumerate() {
        let embedding = &vectors[i].1;
        let scores = r.classifier.classify(embedding);
        let (cell, score, entries) = match scores.first() {
            Some(c) => (
                Some((c.domain.clone(), c.mode.clone())),
                c.score,
                c.entries.iter().take(4).cloned().collect(),
            ),
            None => (None, 0.0, Vec::new()),
        };
        let mut neighbours: Vec<(BlockId, f32)> = vectors
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, (id, v))| (id.clone(), cosine(embedding, v)))
            .collect();
        neighbours.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        neighbours.truncate(NEIGHBOURS);
        let isolated = !neighbours.iter().any(|(_, s)| *s >= NEIGHBOUR_FLOOR);
        observations.push(Observation {
            block_id: p.block_id.clone(),
            revision,
            content_hash: p.content_hash,
            embedder: r.kind.clone(),
            cell,
            score,
            entries,
            neighbours,
            isolated,
            vector: round_vector(embedding),
        });
    }
    observations
}

fn rank(r: &Resident, query: &str, entries: Vec<(usize, Vec<f32>)>) -> Vec<(usize, f32)> {
    let q = r.embedder.embed(query);
    let mut scored: Vec<(usize, f32)> = entries
        .into_iter()
        .map(|(i, v)| (i, cosine(&q, &v)))
        .filter(|(_, s)| *s > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(10);
    scored
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_is_well_behaved_on_degenerate_input() {
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0, "no panic on zero vectors");
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((cosine(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 1.0]), 1.0, "mismatched lengths do not panic");
    }

    #[test]
    fn settings_round_trip_and_default_to_lazy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        assert_eq!(Settings::load(&path).model, ModelPolicy::Lazy);
        let s = Settings {
            model: ModelPolicy::Off,
            embedder: "random-projection".into(),
        };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        assert!(!Settings::load(&path).may_load());
        // A file written by an older version, or a hand-edited one, still loads.
        std::fs::write(&path, b"{\"model\":\"eager\"}").unwrap();
        let back = Settings::load(&path);
        assert_eq!(back.model, ModelPolicy::Eager);
        assert!(back.embedder.is_empty());
    }

    #[test]
    fn the_policy_cycles_and_covers_every_state() {
        assert_eq!(ModelPolicy::default(), ModelPolicy::Lazy);
        let mut p = ModelPolicy::Lazy;
        let mut seen = vec![p];
        for _ in 0..2 {
            p = p.next();
            seen.push(p);
        }
        assert_eq!(seen, vec![ModelPolicy::Lazy, ModelPolicy::Eager, ModelPolicy::Off]);
        assert_eq!(p.next(), ModelPolicy::Lazy, "the cycle closes");
        for policy in ModelPolicy::ALL {
            assert!(!policy.label().is_empty());
        }
    }

    #[test]
    fn the_model_is_off_until_asked_and_off_means_off() {
        let mut w = Worker::new(Settings::default());
        assert!(!w.busy(), "nothing in flight at rest");
        assert_eq!(w.state, State::Idle);
        assert!(w.jobs.is_none(), "no thread until there is work");

        // A settings change to Off drops the channel and refuses work.
        w.set_settings(Settings {
            model: ModelPolicy::Off,
            embedder: String::new(),
        });
        assert_eq!(w.state, State::Off);
        assert!(!w.send(Job::Warm), "off means off");
        assert!(!w.busy());

        // And back on again is allowed.
        w.set_settings(Settings::default());
        assert_eq!(w.state, State::Idle);
        assert!(w.ensure_thread());
    }

    #[test]
    fn changing_the_embedder_drops_the_session_and_a_policy_change_alone_does_not() {
        // The thread captures the settings it started with, so a different
        // embedder cannot reuse the session built for the old one.
        let mut w = Worker::new(Settings::default());
        assert!(w.ensure_thread());
        assert!(w.jobs.is_some());

        // Same embedder, different policy: the session is still valid.
        w.set_settings(Settings {
            model: ModelPolicy::Eager,
            embedder: String::new(),
        });
        assert!(w.jobs.is_some(), "a policy change keeps the session");
        assert_eq!(w.state, State::Idle);

        // A different embedder: the session must go.
        w.set_settings(Settings {
            model: ModelPolicy::Eager,
            embedder: "random-projection".into(),
        });
        assert!(w.jobs.is_none(), "a new embedder needs a new session");
        assert_eq!(w.state, State::Idle);
        assert!(w.label.contains("random-projection"), "{}", w.label);

        // And off drops it as well.
        w.set_settings(Settings::default());
        assert!(w.ensure_thread());
        w.set_settings(Settings {
            model: ModelPolicy::Off,
            embedder: String::new(),
        });
        assert!(w.jobs.is_none());
        assert_eq!(w.state, State::Off);
    }

    #[test]
    fn an_empty_job_is_not_sent_to_the_model() {
        // Nothing to read is not a reason to build a session.
        let mut w = Worker::new(Settings::default());
        w.request(Vec::new(), 0, "nothing".into());
        assert!(!w.busy());
        assert!(w.jobs.is_none(), "no thread for an empty request");
        w.request_search("q".into(), Vec::new(), "nothing".into());
        assert!(w.jobs.is_none());
    }

    #[test]
    fn a_worker_reports_busy_then_yields() {
        // The resident thread round-trips a real job. Warm builds a session, so
        // this is the one test here that touches a model; it uses the offline
        // embedder so it needs no model file.
        let mut w = Worker::new(Settings {
            model: ModelPolicy::Lazy,
            embedder: "random-projection".into(),
        });
        assert!(!w.busy());
        w.warm();
        assert!(w.busy(), "a warm-up is in flight");
        let mut kinds = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while std::time::Instant::now() < deadline {
            if let Some(reply) = w.poll() {
                if let Reply::Warm(kind) = reply {
                    kinds.push(kind);
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(kinds, vec!["random-projection".to_string()]);
        assert!(!w.busy(), "the worker is idle again");
        assert!(matches!(w.state, State::Ready(ref k) if k == "random-projection"));
        assert!(w.query.is_none(), "a warm-up sets no query");
    }
}
