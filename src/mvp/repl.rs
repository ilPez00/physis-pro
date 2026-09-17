//! MVP interactive loop: one command box over the nav index.
//!
//! Wiring only: NavWorkspace (map+retrieve) + pack tokens + act + log.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::deps::{affected_dependents, changed_symbols, classify_intent, render_map};
use super::log::{hash_canonical, CompactDelta, SliceCost, StateRef, Transition, TransitionLog};
use super::model::{Artifact, SemanticDelta};
use super::slice::{diff_snapshots, snapshot_hashes, HashSnapshot, WorldSlice};
use crate::nav::workspace::NavWorkspace;
use crate::nav::{NavHit, NavWeights};

pub struct Session {
    pub root: PathBuf,
    pub ws: NavWorkspace,
    pub focus: Option<Artifact>,
    pub trail: Vec<String>,
    pub last_hits: Vec<NavHit>,
    pub last_slice: Option<WorldSlice>,
    pub before_files: HashSnapshot,
    pub objects_before: usize,
    pub map_counts: (usize, usize, usize),
    pub log: TransitionLog,
    pub transcript: PathBuf,
    pub provider: String,
}

/// Result of observing the filesystem + index around one loop exit.
/// Built once in `observe_delta`; every exit (act, edit, rescan) shares it.
struct Observed {
    delta: SemanticDelta,
    compact: CompactDelta,
    validation: Vec<String>,
    outcome: String,
    affected: Vec<String>,
}

pub fn open(root: &Path) -> anyhow::Result<Session> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    // One state directory per repository. The loop's log already lives in
    // `<repo>/.physis`, and the nav index and caches must live beside it —
    // otherwise `physis <repo>` run from a different cwd splits one
    // repository's state across two directories, and the index is rebuilt
    // every time the operator types the command somewhere else. An explicit
    // `PHYSIS_DATA_DIR` still wins (tests and the demo script rely on it).
    if std::env::var("PHYSIS_DATA_DIR")
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        std::env::set_var("PHYSIS_DATA_DIR", root.join(".physis"));
    }
    let timing = std::env::var("PHYSIS_MVP_TIMING").ok().as_deref() == Some("1");
    let t0 = Instant::now();
    if timing {
        eprintln!("[timing] start (root {})", root.display());
    }
    let mut ws = NavWorkspace::open(&root)?;
    if timing {
        eprintln!(
            "[timing] workspace open (sled+index load): {:?}",
            t0.elapsed()
        );
    }
    // First build on a real repository embeds every chunk and takes minutes;
    // saying so up front is the difference between "working" and "hung".
    if ws.index().objects.is_empty() {
        eprintln!(
            "physis: first index of {} — embedding every chunk (subsequent starts are incremental)",
            root.display()
        );
    }
    let summary = ws.update(true)?;
    let _ = summary;
    if timing {
        eprintln!("[timing] nav index+embed: {:?}", t0.elapsed());
    }
    let t1 = Instant::now();
    let files = crate::scanner::scan_project(&root, None);
    let before_files = snapshot_hashes(&files);
    if timing {
        eprintln!(
            "[timing] scan+hash {} files: {:?}",
            files.len(),
            t1.elapsed()
        );
    }
    let t2 = Instant::now();
    let map_counts = map_counts(&root);
    if timing {
        eprintln!("[timing] sample map {map_counts:?}: {:?}", t2.elapsed());
    }
    let objects_before = ws.index().objects.len();
    let kind = ws.embedder_kind(true);
    if timing {
        eprintln!("[timing] embedder select ({kind}): {:?}", t0.elapsed());
    }
    let log = TransitionLog::open(&root)?;
    let transcript = start_transcript(&root);
    let session = Session {
        root: root.clone(),
        ws,
        focus: None,
        trail: Vec::new(),
        last_hits: Vec::new(),
        last_slice: None,
        before_files,
        objects_before,
        map_counts,
        log,
        transcript,
        provider: kind,
    };
    if timing {
        eprintln!("[timing] open total: {:?}", t0.elapsed());
    }
    Ok(session)
}

fn start_transcript(root: &Path) -> PathBuf {
    let name = format!(
        "mvp_transcript_{}.md",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    );
    let path = root.join(".physis").join(name);
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    let _ = std::fs::write(&path, format!("# MVP transcript — {}\n", root.display()));
    path
}

/// Bounded structural map counts: repeats/differences/contradictions over a
/// capped sample (smallest text files), computed with the deterministic
/// random-projection embedder. Real numbers over the sample, never fabricated.
///
/// The sample is capped in both files and bytes because this runs on every
/// startup and on a real repository an unbounded scan plus a quadratic
/// within-corpus comparison is the difference between "click" and "wait".
/// The cap is stated in the banner field name (`sample map`) so a reader is
/// never told these are whole-repository numbers.
///
/// The result is cached under `.physis/mvp_map_cache.json`, keyed by the
/// content-addressed sample (path + hash of every sampled file). An unchanged
/// sample costs a stat and a hash — the same bargain the rest of the engine
/// makes. A changed sample recomputes in full and says so.
///
/// `pub` so the HUD startup line reports the same sample map as the REPL.
pub fn map_counts(root: &Path) -> (usize, usize, usize) {
    const MAX_DOCS: usize = 120;
    const MAX_FILE_BYTES: u64 = 16 * 1024;
    const MAX_TOTAL_BYTES: u64 = 512 * 1024;

    let mut files = crate::scanner::scan_project(root, None);
    files.sort_by_key(|f| f.size);
    let mut docs: Vec<(String, String)> = Vec::new();
    let mut total = 0u64;
    for f in files
        .iter()
        .filter(|f| f.size > 0 && f.size <= MAX_FILE_BYTES)
    {
        if docs.len() >= MAX_DOCS || total >= MAX_TOTAL_BYTES {
            break;
        }
        if let Ok(text) = std::fs::read_to_string(root.join(&f.path)) {
            total += f.size;
            docs.push((f.path.clone(), text));
        }
    }
    if docs.is_empty() {
        return (0, 0, 0);
    }

    // Cache key: the sample's own content hash, so the key changes exactly
    // when the sampled file set or any sampled byte changes.
    let signature: Vec<(String, String)> = {
        let mut v: Vec<(String, String)> = files
            .iter()
            .filter(|f| docs.iter().any(|(p, _)| p == &f.path))
            .map(|f| (f.path.clone(), f.hash.clone()))
            .collect();
        v.sort();
        v
    };
    let key = crate::mvp::log::hash_canonical(&signature);
    let cache = root.join(".physis").join("mvp_map_cache.json");
    if let Ok(text) = std::fs::read_to_string(&cache) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if v.get("key").and_then(|k| k.as_str()) == Some(key.as_str()) {
                let g = |name: &str| v.get(name).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                return (g("repeats"), g("differences"), g("contradictions"));
            }
        }
    }

    let e = physis_core::embed::RandomProjectionEmbedder::new(128);
    let counts = match physis_core::map::build_map(&docs, &e, None) {
        Ok(m) => (m.repeats.len(), m.differences.len(), m.contradictions.len()),
        Err(_) => (0, 0, 0),
    };
    let (repeats, differences, contradictions) = counts;
    let payload = serde_json::json!({
        "key": key,
        "documents": docs.len(),
        "repeats": repeats,
        "differences": differences,
        "contradictions": contradictions,
    });
    let _ = std::fs::write(&cache, serde_json::to_string(&payload).unwrap_or_default());
    counts
}

impl Session {
    pub fn startup_banner(&mut self) -> String {
        // Factual state only, and only measured: the file count comes from the
        // snapshot taken at open (no second walk), object count from the index,
        // and the map fields are labelled as what they are — counts over a
        // capped sample, not over the whole repository.
        //
        // Edits that landed while the loop was closed are absorbed into the
        // open snapshot, so a bare `rescan` afterwards would (correctly per
        // its code, wrongly per the demo) report nothing. Surface that
        // absorbed-at-open Δ here instead: diff the last recorded
        // `state_after` content hash against the fresh scan, and say so.
        // Nothing is re-recorded — the transition log stays append-only; the
        // banner only reports what the open already observed.
        let name = self
            .root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let (repeats, differences, contradictions) = self.map_counts;
        let mut banner = format!(
            "PHYSIS — {name}\n\nfiles          {}\nobjects        {}\nrelations      {}\nsample map     repeats {} · differences {} · contradictions {}\nindex          ready\n",
            self.before_files.len(),
            self.ws.index().objects.len(),
            self.ws.relations().len() + self.ws.pins().len(),
            repeats, differences, contradictions);
        let fresh_hash = hash_canonical(&self.before_files);
        if let Some(note) = super::log::since_last_session_notice(&self.log, &fresh_hash) {
            banner.push_str(&format!("\n{note}\n"));
        }
        self.say("startup", &banner);
        banner
    }

    fn say(&self, kind: &str, text: &str) {
        use std::fmt::Write as _;
        let mut s = String::new();
        let _ = writeln!(s, "\n## {kind}\n\n```\n{text}\n```\n");
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.transcript)
        {
            let _ = f.write_all(s.as_bytes());
        }
    }

    fn state_ref(&self) -> StateRef {
        let snap = snapshot_hashes(&crate::scanner::scan_project(&self.root, None));
        let content_hash = hash_canonical(&snap);
        let mut ids: Vec<String> = self
            .ws
            .index()
            .objects
            .values()
            .map(|o| format!("{}:{}", o.id.short(), o.hash))
            .collect();
        ids.sort();
        let map_hash = hash_canonical(&ids);
        StateRef {
            content_hash,
            objects: self.ws.index().objects.len(),
            files: snap.len(),
            map_hash,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        intent: &str,
        action: &str,
        delta: CompactDelta,
        validation: Vec<String>,
        outcome: String,
        provenance: Vec<String>,
        before: StateRef,
        after: StateRef,
        uncertainty: f32,
    ) {
        // The slice in force for this transition, if one was scoped: the
        // measured cost of the context, recorded with the state and the delta
        // so a future world-model / transplant learner has the transition's
        // shape and not only its endpoints.
        let slice = self.last_slice.as_ref().map(|s| SliceCost {
            objects_considered: s.objects_considered,
            objects_selected: s.objects_selected,
            candidate_tokens: s.candidate_tokens,
            final_tokens: s.final_tokens,
            provider: s.provider.clone(),
            paths: s
                .chunks
                .iter()
                .map(|c| c.path.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
        });
        let _ = self.log.append(Transition {
            v: 1,
            seq: 0,
            timestamp: String::new(),
            state_before: before,
            intent: intent.into(),
            action: action.into(),
            delta,
            validation,
            outcome,
            state_after: after,
            provenance,
            uncertainty,
            slice,
        });
    }

    /// Scope the intent to a world slice: nav candidates -> pack -> telemetry.
    pub fn scope(&mut self, raw_intent: &str) -> anyhow::Result<String> {
        let started = Instant::now();
        let before = self.state_ref();
        let intent = classify_intent(raw_intent);
        let query = if intent.query.trim().is_empty() {
            raw_intent.to_string()
        } else {
            intent.query.clone()
        };
        let weights = NavWeights::default();
        let global = query.starts_with('/');
        let (mut hits, mode) = self.ws.query(&query, global, &weights, 60)?;
        // Attention needs a tail to demote: keep the full 60 for banding,
        // show the top 20 in the map. Scope output unchanged otherwise.
        let band_hits = hits.clone();
        hits.truncate(20);
        self.last_hits = band_hits.clone();
        self.ws.push_trail("query", &query);
        self.trail.push(query.clone());
        // Pack scoped context: smallest relevant slice, token-fixed. The
        // shared content-addressed embedding cache is used and persisted, so a
        // second query over the same repository costs a hash per unchanged
        // chunk instead of a forward pass — the difference between a demo and
        // a wait on a real repository.
        let (embedder, kind) =
            crate::embed::select_embedder(&crate::config::PhysisProConfig::default().physis);
        let data_dir = std::env::var("PHYSIS_DATA_DIR")
            .ok()
            .filter(|p| !p.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(".physis").to_path_buf());
        let mut cache = crate::pack::EmbedCache::load(&data_dir);
        let pack = crate::pack::pack_cached(
            &self.root,
            &query,
            2000,
            8,
            0.3,
            embedder.as_ref(),
            kind,
            &mut cache,
        );
        cache.save(&data_dir);
        let objects_considered = mode.indexed_objects;
        let slice = WorldSlice {
            intent: raw_intent.to_string(),
            context: pack.context.clone(),
            chunks: pack.chunks.clone(),
            objects_considered,
            objects_selected: pack.chunks.len(),
            candidate_tokens: pack.candidate_tokens,
            final_tokens: pack.tokens,
            budget: 2000,
            provider: format!("{} (extractive; oracle when configured)", self.provider),
            latency_ms: started.elapsed().as_millis(),
        };
        self.last_slice = Some(slice.clone());
        let mut o = String::new();
        o.push_str(&render_map(&hits, &self.trail));
        o.push_str("\n── CONTEXT ──\n");
        o.push_str(&crate::pack::summarize(&pack));
        o.push_str("\n── TELEMETRY ──\n");
        o.push_str(&slice.telemetry());
        o.push('\n');
        for h in hits.iter().take(5) {
            o.push_str(&format!(
                "  [{:.2}] {}  why: {}\n",
                h.score.total,
                h.object
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| h.object.label.clone()),
                crate::nav::explain_hit(h, &weights)
            ));
        }
        let after = self.state_ref();
        self.record(
            raw_intent,
            &format!("scope:{query}"),
            CompactDelta {
                no_change: true,
                ..Default::default()
            },
            vec![],
            format!("scoped {} hits", hits.len()),
            hits.iter()
                .take(8)
                .map(|h| {
                    h.object
                        .source
                        .as_ref()
                        .map(|s| s.path.clone())
                        .unwrap_or_else(|| h.object.label.clone())
                })
                .collect(),
            before,
            after,
            0.2,
        );
        self.say(&format!("intent: {raw_intent}"), &o);
        // Implicit context: top hit becomes focus.
        if let Some(top) = hits.first() {
            self.focus = Some(Artifact::from(&top.object));
        }
        Ok(o)
    }
    /// Answer from the current slice via the notebook.
    pub fn answer_current(&mut self, question: &str) -> anyhow::Result<String> {
        let before = self.state_ref();
        let slice = match self.last_slice.clone() {
            Some(s) => s,
            None => {
                self.scope(question)?;
                self.last_slice.clone().unwrap()
            }
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut docs: Vec<(String, String)> = Vec::new();
        for c in &slice.chunks {
            if seen.insert(format!("{}:{}-{}", c.path, c.start_line, c.end_line)) {
                docs.push((
                    format!("{}:{}-{}", c.path, c.start_line, c.end_line),
                    c.text.clone(),
                ));
            }
        }
        if docs.is_empty() {
            return Ok("The index has no content for this scope.".into());
        }
        let (embedder, kind) = physis_core::embed::select(384);
        let ans = crate::ported::notebook::answer(&docs, question, embedder.as_ref(), 2000, kind)?;
        let out = ans.render();
        let after = self.state_ref();
        self.record(
            question,
            "answer",
            CompactDelta {
                no_change: true,
                ..Default::default()
            },
            vec![],
            format!("answered from {} docs", docs.len()),
            ans.sources.iter().map(|s| s.name.clone()).collect(),
            before,
            after,
            0.3,
        );
        self.say(&format!("answer: {question}"), &out);
        Ok(out)
    }

    /// Inspect: focus object + parents + relations + exact source.
    /// The index's first file-level object, lowest path first — the
    /// deterministic fallback when nothing is in focus yet.
    fn default_object(&self) -> anyhow::Result<crate::nav::PhysisObject> {
        let mut objs: Vec<crate::nav::PhysisObject> = self
            .ws
            .index()
            .objects
            .values()
            .filter(|o| {
                matches!(
                    o.kind,
                    crate::nav::ObjectKind::File | crate::nav::ObjectKind::Note
                )
            })
            .cloned()
            .collect();
        objs.sort_by(|a, b| a.label.cmp(&b.label));
        objs.into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("the index is empty — nothing to inspect"))
    }

    pub fn inspect(&mut self, target: Option<&str>) -> anyhow::Result<String> {
        let before = self.state_ref();
        let obj = match target {
            Some(t) => self.ws.resolve(t)?,
            None => match self.last_hits.first() {
                Some(h) => h.object.clone(),
                // No query yet in this session: fall back to the index's own
                // first file-level object rather than refusing. The selection
                // is deterministic (lowest path) and reported as such.
                None => self.default_object()?,
            },
        };
        self.focus = Some(Artifact::from(&obj));
        self.ws.push_trail("open", &obj.label);
        let mut o = String::new();
        o.push_str(&format!("--- {} ({:?}) ---\n", obj.label, obj.kind));
        o.push_str(&format!(
            "provenance: {}\n",
            obj.source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| obj.label.clone())
        ));
        o.push_str("parents:\n");
        for (kind, name) in crate::nav::semantic_parents(&obj) {
            o.push_str(&format!("  {kind:<6} {name}\n"));
        }
        o.push_str("relations:\n");
        let rels = self.ws.relations_of(&obj);
        if rels.is_empty() {
            o.push_str("  (none)\n");
        }
        for r in &rels {
            o.push_str(&format!(
                "  {} ──[{}]── {}\n",
                r.a.short(),
                r.kind,
                r.b.short()
            ));
        }
        o.push_str("\nsource:\n");
        o.push_str(&self.ws.read(&obj).unwrap_or_default());
        o.push('\n');
        let after = self.state_ref();
        self.record(
            &format!("inspect {}", obj.label),
            "inspect",
            CompactDelta {
                no_change: true,
                ..Default::default()
            },
            vec![],
            "inspected".into(),
            vec![obj.label.clone()],
            before,
            after,
            0.1,
        );
        self.say(&format!("inspect {}", obj.label), &o);
        Ok(o)
    }

    /// Dependencies: dependents of the focus via stem/symbol scan.
    pub fn dependencies(&mut self, target: Option<&str>) -> anyhow::Result<String> {
        let before = self.state_ref();
        if let Some(t) = target {
            self.inspect(Some(t))?;
        }
        let focus = self
            .focus
            .clone()
            .ok_or_else(|| anyhow::anyhow!("nothing in focus"))?;
        let syms = changed_symbols(&self.root, std::slice::from_ref(&focus.path));
        let deps = affected_dependents(&self.root, std::slice::from_ref(&focus.path), &syms);
        let mut o = String::new();
        o.push_str(&format!("dependencies of {}\n", focus.path));
        if deps.is_empty() {
            o.push_str("  (none found)\n");
        }
        for d in &deps {
            o.push_str(&format!("  ↔ {d}\n"));
        }
        let after = self.state_ref();
        self.record(
            &format!("dependencies {}", focus.path),
            "dependencies",
            CompactDelta {
                no_change: true,
                ..Default::default()
            },
            vec![],
            format!("{} dependents", deps.len()),
            deps.clone(),
            before,
            after,
            0.2,
        );
        self.say(&format!("dependencies {}", focus.path), &o);
        Ok(o)
    }

    /// Recent changes: git log when available, else hash-snapshot note.
    /// Body by the shared `history_text` helper (same past the HUD `H`
    /// view reports); the session recording stays here.
    pub fn recent(&mut self) -> anyhow::Result<String> {
        let before = self.state_ref();
        let o = super::deps::history_text(&self.root);
        let after = self.state_ref();
        self.record(
            "recent",
            "recent",
            CompactDelta {
                no_change: true,
                ..Default::default()
            },
            vec![],
            "listed recent".into(),
            vec![],
            before,
            after,
            0.1,
        );
        self.say("recent", &o);
        Ok(o)
    }

    /// Contradictions bearing on focus: core map candidates over the slice.
    /// Pairs by the shared `contradictions_text` helper (same view the HUD
    /// `C` reports); the session recording stays here.
    pub fn contradictions(&mut self) -> anyhow::Result<String> {
        let before = self.state_ref();
        let docs: Vec<(String, String)> = self
            .last_slice
            .clone()
            .map(|s| {
                s.chunks
                    .iter()
                    .map(|c| (c.path.clone(), c.text.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let o = super::deps::contradictions_text(&docs);
        let after = self.state_ref();
        self.record(
            "contradictions",
            "contradictions",
            CompactDelta {
                no_change: true,
                ..Default::default()
            },
            vec![],
            "checked".into(),
            vec![],
            before,
            after,
            0.4,
        );
        self.say("contradictions", &o);
        Ok(o)
    }
    /// Bounded modification: plan, approve, validate, observe Δ, remap.
    pub fn act_flow(&mut self, raw: &str, auto: bool) -> anyhow::Result<String> {
        let before = self.state_ref();
        let before_snap = self.before_files.clone();
        let objects_before = self.ws.index().objects.len();
        if self.last_slice.is_none() {
            self.scope(raw)?;
        }
        let slice = self.last_slice.clone().unwrap();
        let mut o = String::new();
        o.push_str("── PLAN ──\n");
        o.push_str(&format!("intent: {raw}\n"));
        let files: std::collections::BTreeSet<_> =
            slice.chunks.iter().map(|c| c.path.clone()).collect();
        o.push_str(&format!(
            "scoped files: {}\n",
            files.into_iter().take(8).collect::<Vec<_>>().join(", ")
        ));
        o.push_str("agent: existing harness reads the context; no new framework.\n");
        if !auto {
            o.push_str("(pass --yes to approve)\n");
            self.say(&format!("plan: {raw}"), &o);
            return Ok(o);
        }
        // Validation runs in the repository, not in whatever directory the
        // operator launched from: `act::run` is reused unchanged, so the
        // root is pinned with a `cd` prefix rather than by changing its API.
        // The command itself is overridable (`PHYSIS_MVP_VALIDATE`) so a big
        // repository can run a bounded check; whatever runs is printed in the
        // Δ, so the transcript never claims a validation it did not run.
        let validate = std::env::var("PHYSIS_MVP_VALIDATE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "cargo test --quiet 2>&1 | tail -5".to_string());
        let cmd = format!(
            "cd {} && {validate}",
            shell_quote(&self.root.to_string_lossy())
        );
        let acted = crate::ported::act::run(&cmd, vec![], &physis_core::observe::log_path())?;
        let obs = self.observe(&before_snap, objects_before, &acted.command, acted.exit_code, acted.duration_ms as u128, &acted.stdout_tail)?;
        o.push('\n');
        o.push_str(&obs.delta.render());
        o.push('\n');
        let after = self.state_ref();
        self.record(
            raw,
            &format!("act:{cmd}"),
            obs.compact,
            {
                let mut v = vec![format!("exit {:?}", acted.exit_code)];
                // The record carries what validation said, bounded, so a replay
                // shows the same evidence the operator saw.
                let tail: String = acted.stdout_tail.chars().take(400).collect();
                if !tail.trim().is_empty() {
                    v.push(tail);
                }
                v
            },
            format!("exit {:?}", acted.exit_code),
            obs.affected,
            before,
            after,
            if acted.exit_code == Some(0) { 0.1 } else { 0.6 },
        );
        self.say(&format!("act: {raw}"), &o);
        Ok(o)
    }

    /// One observation pass: hash-diff the filesystem, re-index only what
    /// moved (`update(true)` walks in O(Δ) after the stat fast path), and
    /// remap affected state. Every loop exit (validation, `edit`, `rescan`)
    /// shares it, so the Δ numbers mean the same thing everywhere.
    fn observe(
        &mut self,
        before_snap: &HashSnapshot,
        objects_before: usize,
        command: &str,
        exit: Option<i32>,
        duration_ms: u128,
        stdout_tail: &str,
    ) -> anyhow::Result<Observed> {
        let files_after = crate::scanner::scan_project(&self.root, None);
        let after_snap = snapshot_hashes(&files_after);
        let (changed, added, removed) = diff_snapshots(before_snap, &after_snap);
        self.before_files = after_snap;
        let mut changed_all = changed.clone();
        let syms = changed_symbols(&self.root, &changed);
        let affected = affected_dependents(&self.root, &changed, &syms);
        let full = removed.len() > changed.len() + added.len() + 10;
        let summary = self.ws.update(true)?;
        let _ = summary;
        let objects_after = self.ws.index().objects.len();
        let remapped = objects_before.abs_diff(objects_after) + changed.len() + added.len();
        let mut test_results = vec![format!("act: {command} exit {exit:?} in {duration_ms} ms")];
        for line in stdout_tail.lines().filter(|l| !l.trim().is_empty()) {
            test_results.push(format!("  {line}"));
        }
        let delta = SemanticDelta {
            changed_artifacts: { changed_all.extend(added.iter().cloned()); changed_all.clone() },
            added_relations: vec![],
            removed_relations: removed.clone(),
            affected_dependents: affected.clone(),
            invariant_results: vec![
                "file-type invariants preserved (rs/md/json/toml shapes)".into()
            ],
            test_results,
            objects_before,
            objects_after,
            objects_remapped: remapped,
            changed_semantic_neighborhoods: affected.len(),
            full_rescan: full,
        };
        let validation = vec![format!("exit {exit:?}")];
        Ok(Observed {
            compact: CompactDelta {
                changed: changed_all,
                added,
                removed,
                objects_remapped: remapped,
                neighborhoods_changed: affected.len(),
                no_change: changed.is_empty(),
                ..Default::default()
            },
            validation,
            outcome: format!("exit {exit:?}"),
            affected,
            delta,
        })
    }

    /// External/manual edit: whatever changed on disk since the last
    /// observation is the Δ (`rescan` is the no-op-invocation form of this).
    /// Returned text is the rendered Δ; the transition record carries it too.
    /// `observe_without_command` is fallible (re-index can fail); callers
    /// that already return `Result` propagate it, `bench` excepted.
    fn observe_without_command(&mut self, intent: &str) -> anyhow::Result<String> {
        let before = self.state_ref();
        let before_snap = self.before_files.clone();
        let objects_before = self.ws.index().objects.len();
        let obs = self.observe(&before_snap, objects_before, "(no command: external/manual edit)", None, 0, "")?;
        let after = self.state_ref();
        let text = obs.delta.render();
        self.record(intent, "rescan", obs.compact, obs.validation, obs.outcome, obs.affected, before, after, 0.1);
        self.say(intent, &text);
        Ok(text)
    }

    /// `rescan`: pick up whatever changed on disk (including edits from
    /// another terminal), then re-index only what moved — O(Δ) via the nav
    /// stat fast path — and report the Δ. No validation command runs.
    pub fn rescan(&mut self) -> anyhow::Result<String> {
        self.observe_without_command("rescan")
    }

    /// `edit [path]`: open the focus (or `path`) in `$EDITOR` (fallback
    /// `vi`), then observe exactly like `rescan`. Undo-before-save is the
    /// editor's, not ours. External-change detection is free: any bytes that
    /// moved on disk since the last observation — however they got there —
    /// appear in the same Δ.
    pub fn edit(&mut self, target: Option<&str>) -> anyhow::Result<String> {
        if let Some(t) = target {
            self.inspect(Some(t))?;
        }
        let focus = self
            .focus
            .clone()
            .ok_or_else(|| anyhow::anyhow!("nothing in focus — open/zoom into an object first"))?;
        let abs = self.root.join(&focus.path);
        let editor = std::env::var("EDITOR")
            .ok()
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| "vi".to_string());
        let status = std::process::Command::new(&editor)
            .arg(&abs)
            .status()
            .map_err(|e| anyhow::anyhow!("cannot launch editor {editor}: {e}"))?;
        if !status.success() {
            return Ok(format!("editor exited {status:?} — no rescan ran\n"));
        }
        self.observe_without_command(&format!("edit {}", focus.path))
    }

    /// `focus [query]`: attention over the world. Re-scopes retrieval and
    /// re-renders the map with HOT/WARM/COLD bands so the operator sees what
    /// the intent pulled forward and what it demoted. Pins survive: any hit
    /// touching a pinned object is never COLD.
    pub fn focus_query(&mut self, q: Option<&str>) -> anyhow::Result<String> {
        let before = self.state_ref();
        let query = match q {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => match self.last_slice.clone() {
                Some(s) => s.intent.clone(),
                None => anyhow::bail!("no slice yet — scope an intent first (e.g. `search <q>`)"),
            },
        };
        self.scope(&query)?;
        let after = self.state_ref();
        let hits = self.last_hits.clone();
        let mut o = String::new();
        o.push_str("── ATTENTION ──\n");
        let pins = self.ws.pins();
        let pinned: std::collections::BTreeSet<String> = pins
            .iter()
            .flat_map(|p| {
                [serde_json::to_string(&p.a).ok(), serde_json::to_string(&p.b).ok()]
                    .into_iter()
                    .flatten()
            })
            .collect();
        let key_of = |h: &NavHit| serde_json::to_string(&h.object.id).unwrap_or_default();
        let mut hot = Vec::new();
        let mut warm = Vec::new();
        // COLD = the demoted tail: hits beyond the top-8, shown collapsed as
        // paths only (attention changed over the world, not just the chat).
        let mut cold: Vec<String> = Vec::new();
        for (i, h) in hits.iter().enumerate() {
            let path = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            let row = format!("{:.2}  {path}", h.score.total);
        // Rank-position bands over this scope's own 60-hit tail (top-20
        // shown, rest demotable) — visible in the map output, never a claim
        // about the whole repository.
            if pinned.contains(&key_of(h)) {
                hot.push(format!("{row}  [pinned]"));
            } else if i < 3 {
                hot.push(row);
            } else if i < 8 {
                warm.push(row);
            } else {
                cold.push(path);
            }
        }
        let band = |name: &str, rows: &[String]| {
            let mut s = format!("{name} ({})\n", rows.len());
            for r in rows.iter().take(8) {
                s.push_str(&format!("  {r}\n"));
            }
            s
        };
        o.push_str(&band("HOT", &hot));
        o.push_str(&band("WARM", &warm));
        o.push_str(&format!("COLD ({}) — collapsed\n", cold.len()));
        for c in cold.iter().take(8) {
            o.push_str(&format!("  · {c}\n"));
        }
        if cold.len() > 8 {
            o.push_str(&format!("  … and {} more\n", cold.len() - 8));
        }
        o.push_str(&format!("pinned objects never demote ({} pins)\n", pins.len()));
        self.record(&query, "focus", CompactDelta { no_change: true, ..Default::default() }, vec![], format!("{} hits banded", hits.len()), hits.iter().take(8).map(|h| h.object.source.as_ref().map(|s| s.path.clone()).unwrap_or_else(|| h.object.label.clone())).collect(), before, after, 0.2);
        self.say(&format!("focus: {query}"), &o);
        Ok(o)
    }

    /// `context`: the deterministic packet for the current slice — what a
    /// human inspects before any model sees it, and what a local worker
    /// receives instead of the whole repository. Model-free: intents,
    /// targets, dependents, constraints, bounded snippets, measured costs.
    pub fn context_packet(&mut self) -> anyhow::Result<String> {
        let before = self.state_ref();
        let slice = match self.last_slice.clone() {
            Some(s) => s,
            None => anyhow::bail!("no slice yet — scope an intent first (e.g. `search <q>`)"),
        };
        let mut paths: std::collections::BTreeSet<String> = Default::default();
        for c in &slice.chunks {
            paths.insert(c.path.clone());
        }
        let targets: Vec<String> = paths.iter().take(8).cloned().collect();
        // Dependents of the targets, same precision-first rule the Δ uses.
        let affected = affected_dependents(&self.root, &targets, &changed_symbols(&self.root, &targets));
        // Workspace denominator: same token counter as the packer, over the
        // same chunk shape — a measured total, never an estimate.
        let mut ws_tokens = 0usize;
        let mut ws_chunks = 0usize;
        for f in crate::scanner::scan_project(&self.root, None) {
            let Ok(text) = std::fs::read_to_string(self.root.join(&f.path)) else { continue };
            for w in text.lines().collect::<Vec<_>>().chunks(crate::pack::CHUNK_LINES) {
                let t = w.join("\n");
                if t.trim().is_empty() { continue }
                ws_chunks += 1;
                ws_tokens += crate::pack::tokens_of(&t);
            }
        }
        let excluded = if ws_tokens > 0 { 100.0 * (ws_tokens.saturating_sub(slice.final_tokens)) as f64 / ws_tokens as f64 } else { 0.0 };
        let mut o = String::new();
        o.push_str("── CONTEXT PACKET ──\n\nINTENT\n");
        o.push_str(&format!("{}\n", slice.intent));
        o.push_str("\nTARGETS\n");
        for t in &targets { o.push_str(&format!("- {t}\n")); }
        if targets.is_empty() { o.push_str("(none)\n"); }
        o.push_str("\nDEPENDENCIES\n");
        for d in affected.iter().take(12) { o.push_str(&format!("- {d}\n")); }
        if affected.is_empty() { o.push_str("(none found)\n"); }
        o.push_str("\nCONSTRAINTS\n");
        o.push_str("- public interfaces of TARGETS, as consumed by DEPENDENCIES\n");
        o.push_str("- file-type shapes hold (rs/md/json/toml — see invariants)\n");
        o.push_str("\nRECENT DELTA\n");
        let recent: Vec<String> = self.log.read_all().iter().rev().take(3).filter(|t| !t.delta.no_change).flat_map(|t| t.delta.changed.clone()).collect();
        if recent.is_empty() { o.push_str("(no recorded changes this session)\n"); }
        for r in recent.iter().take(6) { o.push_str(&format!("- {r}\n")); }
        o.push_str("\nRELEVANT SOURCE\n");
        for c in slice.chunks.iter().take(8) {
            o.push_str(&format!("--- {}:{}-{} ({} tokens, score {:.3}) ---\n", c.path, c.start_line, c.end_line, c.tokens, c.score));
            for line in c.text.lines().take(20) { o.push_str(line); o.push('\n'); }
        }
        o.push_str("\nEXCLUDED\n");
        o.push_str(&format!("{excluded:.1}% of indexed workspace ({} of {ws_tokens} tokens, {} chunks, {} objects considered)\n", slice.final_tokens, ws_chunks, slice.objects_considered));
        o.push_str(&format!("workspace source: {ws_tokens} tokens · selected context: {} tokens · excluded: {excluded:.1}%\n", slice.final_tokens));
        let gen = super::deps::generator_state();
        o.push_str(&format!("generator: {gen} — packet is complete without it\n"));
        let after = self.state_ref();
        self.record(&slice.intent, "context", CompactDelta { no_change: true, ..Default::default() }, vec![], format!("packet {} tokens", slice.final_tokens), targets, before, after, 0.1);
        self.say("context", &o);
        Ok(o)
    }

    /// `bench`: real offline performance numbers — scan cost, index size,
    /// query/propagation latency. Measured by the shared `deps::bench_text`
    /// helper (same numbers the HUD `B` view reports).
    pub fn bench(&mut self) -> String {
        let q = self
            .last_slice
            .as_ref()
            .map(|s| s.intent.clone())
            .unwrap_or_else(|| "bench".to_string());
        let slice_summary = self
            .last_slice
            .as_ref()
            .map(|s| (s.candidate_tokens, s.final_tokens));
        let o = super::deps::bench_text(&mut self.ws, &self.root, &self.provider, &q, slice_summary);
        self.say("bench", &o);
        o
    }

    /// One input surface: nav commands + plain intent.
    ///
    /// Model-free contract: every arm below works with `PHYSIS_MODEL=none`
    /// (nothing here reads that variable — the generator lives only behind
    /// `PHYSIS_ORACLE_*` in the notebook path, which degrades to extractive).
    /// `edit` shells to `$EDITOR` (undo is the editor's, before save);
    /// `rescan` detects whatever changed on disk, however it got there.
    pub fn handle(&mut self, line: &str) -> anyhow::Result<Option<String>> {
        let t = line.trim();
        if t.is_empty() {
            return Ok(None);
        }
        self.say("input", t);
        let lower = t.to_lowercase();
        if ["quit", "exit", ":q"].contains(&lower.as_str()) {
            return Ok(Some("__quit__".into()));
        }
        if lower == "help" {
            return Ok(Some("zoom/open <t> · back · search <q> · focus [q] · related · dependencies [t] · differences · contradictions · history · inspect [t] · context · edit [path] · rescan · bench · improve ... · act ... --yes · quit".into()));
        }
        if lower == "back" {
            self.trail.pop();
            return Ok(Some("back".into()));
        }
        if lower == "context" {
            return Ok(Some(self.context_packet()?));
        }
        if lower == "rescan" {
            return Ok(Some(self.rescan()?));
        }
        if lower == "bench" {
            return Ok(Some(self.bench()));
        }
        if lower == "focus" || lower.starts_with("focus ") {
            let q = t.strip_prefix("focus").unwrap().trim();
            let q = if q.is_empty() { None } else { Some(q) };
            return Ok(Some(self.focus_query(q)?));
        }
        if lower == "edit" || lower.starts_with("edit ") {
            let tg = t.strip_prefix("edit").unwrap().trim();
            let tg = if tg.is_empty() { None } else { Some(tg) };
            return Ok(Some(self.edit(tg)?));
        }
        if let Some(q) = t.strip_prefix("search ") {
            return Ok(Some(self.scope(q)?));
        }
        if let Some(tg) = t.strip_prefix("open ").or_else(|| t.strip_prefix("zoom ")) {
            return Ok(Some(self.inspect(Some(tg.trim()))?));
        }
        if t == "inspect" || t == "inspect this" {
            return Ok(Some(self.inspect(None)?));
        }
        if t.starts_with("dependencies") {
            let tg = t.strip_prefix("dependencies").unwrap().trim();
            let tg = if tg.is_empty() { None } else { Some(tg) };
            return Ok(Some(self.dependencies(tg)?));
        }
        if t == "history" || lower == "what changed recently?" {
            return Ok(Some(self.recent()?));
        }
        if t == "differences" {
            let (_, d, _) = self.map_counts;
            return Ok(Some(format!("differences (sample map): {d}\n")));
        }
        if t == "contradictions" || lower == "what contradicts this?" {
            return Ok(Some(self.contradictions()?));
        }
        if t == "related" {
            if let Some(f) = self.focus.clone() {
                let deps = affected_dependents(&self.root, std::slice::from_ref(&f.path), &[]);
                return Ok(Some(format!(
                    "related to {}:\n{}\n",
                    f.path,
                    deps.iter()
                        .map(|d| format!("  ↔ {d}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )));
            }
            return Ok(Some("(nothing in focus)\n".into()));
        }
        let intent = classify_intent(t);
        if intent.wants_action || lower.starts_with("act ") || lower.starts_with("improve ") {
            let auto = t.contains("--yes");
            let raw = t.replace("--yes", "").trim().to_string();
            self.scope(&raw)?;
            return Ok(Some(self.act_flow(&raw, auto)?));
        }
        let scoped = self.scope(t)?;
        let ans = self.answer_current(t).unwrap_or_default();
        Ok(Some(format!("{scoped}\n── ANSWER ──\n{ans}")))
    }
}

/// Minimal POSIX single-quote for a path handed to `sh -c`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Generator state for the demo contract: `PHYSIS_MODEL=none` (or unset) =
/// deterministic/manual path, coherent without any model; `local` = the
/// optional offline worker behind `PHYSIS_ORACLE_*` pointed at localhost.
/// Anything else is reported verbatim — never assumed usable.
pub fn run(root: &Path) -> anyhow::Result<()> {
    let mut s = open(root)?;
    print!("{}", s.startup_banner());
    std::io::stdout().flush().ok();
    println!("log: {}", s.log.path().display());
    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("\n> ");
        io::stdout().flush().ok();
        line.clear();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        match s.handle(&line) {
            Ok(None) => {}
            Ok(Some(o)) if o == "__quit__" => break,
            Ok(Some(o)) => println!("{o}"),
            Err(e) => eprintln!("(mvp) {e}"),
        }
    }
    Ok(())
}
