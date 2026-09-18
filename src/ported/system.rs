// provenance: moved from physis-core src/system.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
//! Shared workspace interface for people, agents, and folder projections.
//!
//! Reuses Physis BM25 and its observation log. Queries read the workspace;
//! `remember` and `run` append observations that `physis-core observed` can read
//! with the same PHYSIS_CORE_DIR. A folder export is a snapshot of references,
//! never a second authoritative filesystem or a command queue.
//!
//! This first interface uses lexical retrieval and explicit program arguments.
//! It does not infer intent, certify commands, or sandbox their effects.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use physis_core::observe::{self, Observation};
use physis_core::rag::{bm25_terms, count_tokens, Bm25Index};

pub const API_VERSION: &str = "physis.system.v1";
const MAX_FILES: usize = 5_000;
const FILE_PREFIX: u64 = 32 * 1024;
const SEARCH_BYTES: usize = 8 * 1024 * 1024;
const HISTORY_WINDOW: usize = 10_000;
/// Lines per ranked window. Small enough that a 2k budget holds several
/// independent regions, large enough that a function usually survives whole.
const PACK_WINDOW_LINES: usize = 40;
/// Files kept by the first stage of `FilesThenWindows`. Wide enough that the
/// right file survives a mediocre query, narrow enough to suppress the
/// one-line-file bias that window-level BM25 has on its own.
const PACK_FILE_SHORTLIST: usize = 12;
const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "vendor",
    "__pycache__",
    "dist",
    "build",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: String,
    pub path: String,
    pub bytes: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub objects: Vec<Object>,
    pub truncated: bool,
    pub skipped_links: usize,
    pub errors: Vec<String>,
}

/// Which ranking produces the candidate windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackStrategy {
    /// Rank every line window directly.
    Windows,
    /// Rank files first, then windows inside the shortlist.
    FilesThenWindows,
}

impl PackStrategy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::FilesThenWindows => "files-then-windows",
        }
    }
}

/// One ranked line window of a file: the unit `pack` selects and prices.
struct Window {
    path: String,
    start_line: usize,
    end_line: usize,
    tokens: usize,
    text: String,
}

#[derive(Debug, Serialize)]
pub struct Hit {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub score: f32,
    pub excerpt: String,
}

/// The root scopes file queries. State is a regular Core observation store.
pub struct Workspace {
    pub root: PathBuf,
    pub state: PathBuf,
    /// Directory names skipped during enumeration. Defaults to [`SKIP_DIRS`];
    /// a caller inspecting a *published* artifact (an npm package is mostly
    /// `dist/`, a vendored crate mostly `vendor/`) needs those back, and
    /// without this the interface reports a confident file count over the
    /// handful of files that are not the thing being inspected.
    pub skip_dirs: Vec<String>,
}

impl Workspace {
    pub fn open(root: &Path, state: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .context("workspace root cannot be opened")?;
        ensure!(root.is_dir(), "workspace root must be a directory");
        let state = if state.is_absolute() {
            state.to_path_buf()
        } else {
            std::env::current_dir()?.join(state)
        };
        Ok(Self {
            root,
            state,
            skip_dirs: SKIP_DIRS.iter().map(|d| d.to_string()).collect(),
        })
    }

    /// Stop skipping these directory names. Unknown names are kept, so a typo
    /// is visible in `capabilities` output rather than silently doing nothing.
    pub fn including(mut self, dirs: &[String]) -> Self {
        self.skip_dirs
            .retain(|d| !dirs.iter().any(|keep| keep == d));
        self
    }

    pub fn capabilities(&self) -> Value {
        json!({
            "api_version": API_VERSION,
            "workspace": self.root,
            "state": self.state,
            "history": self.log_path(),
            "identity": "file IDs address a workspace-relative path; renames change the ID",
            "representation": "BM25 lexical retrieval; no model or inferred intent",
            "operations": [
                {"name":"capabilities", "writes":false, "purpose":"Discover this contract"},
                {"name":"serve", "writes":true, "purpose":"MCP stdio: workspace queries and explicit notes; no command execution"},
                {"name":"inspect", "writes":false, "purpose":"Inventory files and recent observations"},
                {"name":"list", "writes":false, "purpose":"List addressable file objects"},
                {"name":"find", "writes":false, "purpose":"Rank files and remembered outcomes using BM25"},
                {"name":"pack", "writes":false, "purpose":"Assemble a token-budgeted context bundle of line windows"},
                {"name":"predict", "writes":false, "purpose":"Read this workspace's own record of how this kind of action has gone"},
                {"name":"read", "writes":false, "purpose":"Read a path (optionally path:start-end), file ID or prefix, or obs:sequence"},
                {"name":"history", "writes":false, "purpose":"Read this workspace's recorded work"},
                {"name":"runs", "writes":false, "purpose":"Recent command runs with intent, exit status and duration, newest first"},
                {"name":"usage", "writes":false, "purpose":"Per-source and per-kind usage counters over the observation log"},
                {"name":"delegation", "writes":true, "purpose":"Plan/list/show are read-only; create/reserve/release record unstarted tasks; export writes a snapshot; no worker launch"},
                {"name":"remember", "writes":true, "purpose":"Append a note with an explicit outcome"},
                {"name":"run", "writes":true, "purpose":"Run explicit argv and record intent, logs, and exit status"},
                {"name":"export", "writes":true, "purpose":"Create a new folder snapshot of the same references"}
            ],
            "limits": {"max_files":MAX_FILES, "file_prefix_bytes":FILE_PREFIX,
                "pack_window_lines":PACK_WINDOW_LINES, "pack_file_shortlist":PACK_FILE_SHORTLIST,
                "pack_token_counter":"heuristic; budget is in Physis tokens, not the caller's BPE",
                "search_bytes":SEARCH_BYTES, "history_tail_records":HISTORY_WINDOW,
                "hidden_paths":"excluded from enumeration", "excluded_directories":self.skip_dirs,
                "default_excluded_directories":SKIP_DIRS},
            "execution": "ordinary host permissions; workspace is cwd, not a sandbox; no automatic retries"
        })
    }

    fn log_path(&self) -> PathBuf {
        self.state.join("observations.jsonl")
    }

    fn object(&self, path: &Path) -> Result<Object> {
        let relative = path
            .strip_prefix(&self.root)?
            .to_string_lossy()
            .into_owned();
        let metadata = fs::metadata(path)?;
        let id = format!(
            "file:{:x}",
            Sha256::digest(format!("{}\0{relative}", self.root.display()))
        );
        Ok(Object {
            id,
            path: relative,
            bytes: metadata.len(),
            modified_at: metadata
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
        })
    }

    /// Enumerates regular files without following directory symlinks. Limits
    /// and omissions are part of the result, not an implied complete world.
    pub fn inventory(&self) -> Result<Inventory> {
        let mut result = Inventory {
            objects: vec![],
            truncated: false,
            skipped_links: 0,
            errors: vec![],
        };
        self.walk(&self.root, &mut result)?;
        result.objects.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(result)
    }

    fn walk(&self, dir: &Path, result: &mut Inventory) -> Result<()> {
        let mut entries = match fs::read_dir(dir) {
            Ok(entries) => entries.collect::<std::io::Result<Vec<_>>>()?,
            Err(e) => {
                result.errors.push(format!("{}: {e}", dir.display()));
                return Ok(());
            }
        };
        entries.sort_by_key(|a| a.file_name());
        for entry in entries {
            if result.objects.len() >= MAX_FILES {
                result.truncated = true;
                break;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                result.skipped_links += 1;
                continue;
            }
            if kind.is_dir() {
                if !self.skip_dirs.iter().any(|d| d == name.as_ref()) && entry.path() != self.state
                {
                    self.walk(&entry.path(), result)?;
                }
            } else if kind.is_file() {
                match self.object(&entry.path()) {
                    Ok(object) => result.objects.push(object),
                    Err(e) => result
                        .errors
                        .push(format!("{}: {e}", entry.path().display())),
                }
            }
        }
        Ok(())
    }

    fn records(&self) -> Result<Vec<Observation>> {
        Ok(observe::read_tail(&self.log_path(), HISTORY_WINDOW)?
            .into_iter()
            .filter(|o| {
                let payload: Value = serde_json::from_str(&o.body).unwrap_or(Value::Null);
                payload.get("workspace").and_then(Value::as_str) == self.root.to_str()
                    // Existing filesystem observations have a path as subject
                    // and a content hash as body, rather than our JSON payload.
                    // Absolute paths carry scope; relative legacy paths do not.
                    || (o.source == "fs"
                        && Path::new(&o.subject).is_absolute()
                        && Path::new(&o.subject).starts_with(&self.root))
            })
            .collect())
    }

    pub fn history(&self, query: Option<&str>, limit: usize) -> Result<Value> {
        let all = self.records()?;
        let query = query.map(str::to_lowercase);
        let matches: Vec<_> = all
            .iter()
            .filter(|o| {
                query.as_ref().is_none_or(|q| {
                    format!("{} {}", o.subject, o.body)
                        .to_lowercase()
                        .contains(q)
                })
            })
            .collect();
        let records: Vec<_> = matches.iter().rev().take(limit).copied().collect();
        Ok(
            json!({"records": records, "matched_in_window":matches.len(),
            "workspace_records_in_window":all.len(), "history_window":HISTORY_WINDOW,
            "truncated":matches.len() > records.len(), "log":self.log_path()}),
        )
    }

    pub fn inspect(&self) -> Result<Value> {
        let inventory = self.inventory()?;
        let mut by_extension = BTreeMap::<String, usize>::new();
        for o in &inventory.objects {
            let ext = Path::new(&o.path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("(none)");
            *by_extension.entry(ext.into()).or_default() += 1;
        }
        let records = self.records()?;
        let starts: Vec<_> = records
            .iter()
            .filter(|o| o.source == "system.run.start")
            .collect();
        let unfinished: Vec<_> = starts
            .into_iter()
            .filter(|start| {
                !records.iter().any(|o| {
                    let value: Value = serde_json::from_str(&o.body).unwrap_or(Value::Null);
                    o.source == "system.run.finish"
                        && value["start_seq"].as_u64() == Some(start.seq)
                })
            })
            .map(|o| format!("obs:{}", o.seq))
            .collect();
        Ok(
            json!({"files":inventory.objects.len(), "by_extension":by_extension,
            "inventory_truncated":inventory.truncated, "skipped_links":inventory.skipped_links,
            "scan_errors":inventory.errors, "workspace_records_in_window":records.len(),
            "runs_without_recorded_finish_in_window":unfinished,
            "history_window":HISTORY_WINDOW, "capabilities":self.capabilities()}),
        )
    }

    pub fn find(&self, query: &str, limit: usize) -> Result<Value> {
        ensure!(!query.trim().is_empty(), "find needs a non-empty query");
        let inventory = self.inventory()?;
        let mut documents = Vec::new();
        let mut ids = Vec::new();
        let mut used_bytes = 0usize;
        let mut content_truncated = 0;
        for object in &inventory.objects {
            let available = SEARCH_BYTES
                .saturating_sub(used_bytes)
                .min(FILE_PREFIX as usize);
            let content = if available > 0 {
                read_prefix(&self.root.join(&object.path), available as u64).unwrap_or_default()
            } else {
                String::new()
            };
            used_bytes += content.len();
            if object.bytes > content.len() as u64 {
                content_truncated += 1;
            }
            documents.push(format!("{}\n{content}", object.path));
            ids.push((object.id.clone(), "file".to_string(), object.path.clone()));
        }
        for record in self.records()? {
            documents.push(format!("{}\n{}", record.subject, record.body));
            ids.push((
                format!("obs:{}", record.seq),
                "observation".into(),
                record.subject,
            ));
        }
        let index = Bm25Index::build(&documents);
        let terms = bm25_terms(query);
        let hits: Vec<_> = index
            .rank(&terms)
            .into_iter()
            .filter(|(_, score)| *score > 0.0)
            .take(limit)
            .map(|(i, score)| {
                let (id, kind, label) = &ids[i];
                let line = documents[i]
                    .lines()
                    .find(|line| {
                        let lower = line.to_lowercase();
                        terms.iter().any(|t| lower.contains(t))
                    })
                    .unwrap_or(&documents[i]);
                Hit {
                    id: id.clone(),
                    kind: kind.clone(),
                    label: label.clone(),
                    score,
                    excerpt: line.chars().take(280).collect(),
                }
            })
            .collect();
        Ok(
            json!({"query":query, "method":"physis_core::rag::Bm25Index",
            "representation":"lexical", "hits":hits, "documents_ranked":documents.len(),
            "content_bytes":used_bytes, "files_without_complete_text":content_truncated,
            "inventory_truncated":inventory.truncated, "scan_errors":inventory.errors,
            "history_window":HISTORY_WINDOW}),
        )
    }

    /// Pack a token-budgeted context bundle for `query`.
    ///
    /// `find` ranks whole files and prints one line, which leaves the caller to
    /// read the file itself; the read is where the tokens go. This ranks
    /// *windows* of lines, then greedily packs the highest-scoring windows
    /// until the next one would exceed `budget`, so the answer is the context
    /// rather than a pointer to it, and its cost is bounded before the call.
    ///
    /// Lexical BM25 over the same file prefixes `find` uses. `per_file` caps
    /// how many windows one file may contribute, because a single long file
    /// otherwise fills the budget with one region's neighbours.
    pub fn pack(&self, query: &str, budget: usize, per_file: usize) -> Result<Value> {
        self.pack_with(
            query,
            budget,
            per_file,
            PackStrategy::FilesThenWindows,
            true,
        )
    }

    /// `pack`, with the ranking stage named so the two can be measured against
    /// each other rather than chosen by argument.
    pub fn pack_with(
        &self,
        query: &str,
        budget: usize,
        per_file: usize,
        strategy: PackStrategy,
        bridge_gaps: bool,
    ) -> Result<Value> {
        self.pack_selected(query, budget, per_file, strategy, bridge_gaps, None)
    }

    /// Pack only the supplied workspace-relative paths (files or directories).
    /// Filtering happens before content is read or ranked.
    pub fn pack_paths(&self, query: &str, budget: usize, paths: &[PathBuf]) -> Result<Value> {
        ensure!(!paths.is_empty(), "scoped packing requires paths");
        self.pack_selected(
            query,
            budget,
            3,
            PackStrategy::FilesThenWindows,
            true,
            Some(paths),
        )
    }

    /// One side of the P3 staged-diff gate: a deterministic, engine-independent
    /// preview of a proposed change vs the *current* working tree, before any
    /// patch is applied. `baselines` maps workspace-relative paths to the content
    /// the change was authored against; each path is read from the workspace and
    /// diffed with the same `crate::diff::diff_lines` the Studio DIFF pane uses,
    /// so the review surface and the apply path share one hunk algorithm.
    ///
    /// Hunks are returned only for paths present in both `baselines` and the
    /// workspace — missing or untracked paths surface as an error, never a
    /// silent skip, because a patch over a file that does not exist in the
    /// working tree is a stale-base condition the caller must resolve.
    pub fn stage_diff(&self, baselines: &[(String, String)]) -> Result<Vec<(String, Vec<crate::diff::Hunk>)>> {
        let mut out = Vec::new();
        for (rel, base) in baselines {
            let here = self.root.join(rel);
            let current = std::fs::read_to_string(&here)
                .with_context(|| format!("stale-base: {rel} not found in workspace; re-baseline against HEAD"))?;
            let hunks = crate::diff::diff_lines(base, &current);
            out.push((rel.clone(), hunks));
        }
        Ok(out)
    }

    fn pack_selected(
        &self,
        query: &str,
        budget: usize,
        per_file: usize,
        strategy: PackStrategy,
        bridge_gaps: bool,
        paths: Option<&[PathBuf]>,
    ) -> Result<Value> {
        ensure!(!query.trim().is_empty(), "pack needs a non-empty query");
        ensure!(budget > 0, "pack needs a positive token budget");
        let inventory = self.inventory()?;
        let mut windows: Vec<Window> = Vec::new();
        let mut file_texts: Vec<(String, String)> = Vec::new();
        let mut documents: Vec<String> = Vec::new();
        let mut used_bytes = 0usize;
        let mut content_truncated = 0;
        for object in &inventory.objects {
            if paths
                .is_some_and(|paths| !paths.iter().any(|p| Path::new(&object.path).starts_with(p)))
            {
                continue;
            }
            let available = SEARCH_BYTES
                .saturating_sub(used_bytes)
                .min(FILE_PREFIX as usize);
            if available == 0 {
                continue;
            }
            let content =
                read_prefix(&self.root.join(&object.path), available as u64).unwrap_or_default();
            used_bytes += content.len();
            if object.bytes > content.len() as u64 {
                content_truncated += 1;
            }
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() {
                continue;
            }
            file_texts.push((object.path.clone(), content.clone()));
            for (index, chunk) in lines.chunks(PACK_WINDOW_LINES).enumerate() {
                let text = chunk.join("\n");
                if text.trim().is_empty() {
                    continue;
                }
                let start = index * PACK_WINDOW_LINES + 1;
                // The path rides in the indexed document so a query naming a
                // file still reaches that file's windows, exactly as in `find`.
                documents.push(format!("{}\n{text}", object.path));
                windows.push(Window {
                    path: object.path.clone(),
                    start_line: start,
                    end_line: start + chunk.len() - 1,
                    tokens: count_tokens(&text),
                    text,
                });
            }
        }
        // Stage one, when asked for it: rank whole files the way `find` does and
        // keep the windows of the best ones. A 40-line window is a short
        // document, and BM25 rewards short documents — one-line files outrank a
        // function that answers the question. File-level scores are computed
        // over the whole file, where that bias does not apply, and the window
        // stage then only has to choose *where* in a plausible file to look.
        let allowed: Option<std::collections::BTreeSet<String>> = match strategy {
            PackStrategy::Windows => None,
            PackStrategy::FilesThenWindows => {
                let file_docs: Vec<String> = file_texts
                    .iter()
                    .map(|(path, text)| format!("{path}\n{text}"))
                    .collect();
                let file_index = Bm25Index::build(&file_docs);
                let file_terms = bm25_terms(query);
                Some(
                    file_index
                        .rank(&file_terms)
                        .into_iter()
                        .filter(|(_, score)| *score > 0.0)
                        .take(PACK_FILE_SHORTLIST)
                        .map(|(i, _)| file_texts[i].0.clone())
                        .collect(),
                )
            }
        };
        let ranked_total = documents.len();
        ensure!(
            ranked_total > 0,
            "0 windows over {} files: nothing to pack",
            inventory.objects.len()
        );
        let index = Bm25Index::build(&documents);
        let terms = bm25_terms(query);
        let mut per_file_count: BTreeMap<String, usize> = BTreeMap::new();
        let mut selected: Vec<(usize, f32)> = Vec::new();
        let mut bridged = 0usize;
        let mut used_tokens = 0usize;
        let mut skipped_over_budget = 0usize;
        for (i, score) in index.rank(&terms) {
            if score <= 0.0 {
                break;
            }
            let window = &windows[i];
            if let Some(allowed) = &allowed {
                if !allowed.contains(&window.path) {
                    continue;
                }
            }
            let count = per_file_count.entry(window.path.clone()).or_insert(0);
            if *count >= per_file {
                continue;
            }
            if used_tokens + window.tokens > budget {
                skipped_over_budget += 1;
                // Keep scanning: a smaller window further down still fits.
                continue;
            }
            *count += 1;
            used_tokens += window.tokens;
            selected.push((i, score));
            if !bridge_gaps {
                continue;
            }
            // Close a one-window gap the moment it forms, ahead of the next
            // ranked candidate. Windows are cut at fixed line boundaries, so
            // the lines that answer a query routinely straddle them: measured
            // on a Kotlin login screen, 1-40 and 81-120 were selected and the
            // answer sat at line 43. Deferring this to a pass after the budget
            // is spent never gets to run, which is how it was first written.
            let chosen: Vec<usize> = selected.iter().map(|(j, _)| *j).collect();
            for candidate in [i.wrapping_sub(1), i + 1] {
                if candidate >= windows.len() || chosen.contains(&candidate) {
                    continue;
                }
                let far = if candidate > i {
                    candidate + 1
                } else {
                    candidate.wrapping_sub(1)
                };
                let bridges_a_gap = far < windows.len()
                    && chosen.contains(&far)
                    && windows[far].path == windows[i].path
                    && windows[candidate].path == windows[i].path;
                if !bridges_a_gap {
                    continue;
                }
                if used_tokens + windows[candidate].tokens <= budget {
                    used_tokens += windows[candidate].tokens;
                    bridged += 1;
                    // Score 0.0: it was not ranked in, it was read around into.
                    selected.push((candidate, 0.0));
                }
            }
        }
        // Reading order, not rank order: same-file regions arrive adjacent and
        // in line order, which is how the caller would have read them.
        selected.sort_by(|a, b| {
            let (x, y) = (&windows[a.0], &windows[b.0]);
            (&x.path, x.start_line).cmp(&(&y.path, y.start_line))
        });
        let chunks: Vec<Value> = selected
            .iter()
            .map(|(i, score)| {
                let w = &windows[*i];
                json!({
                    "path": w.path,
                    "start_line": w.start_line,
                    "end_line": w.end_line,
                    "tokens": w.tokens,
                    "score": score,
                    "text": w.text,
                })
            })
            .collect();
        Ok(json!({
            "query": query,
            "method": "physis_core::rag::Bm25Index over line windows",
            "representation": "lexical",
            "budget_tokens": budget,
            "used_tokens": used_tokens,
            "window_lines": PACK_WINDOW_LINES,
            "per_file_limit": per_file,
            "strategy": strategy.name(),
            "file_shortlist": allowed.as_ref().map(|a| a.len()),
            "chunks": chunks,
            "windows_ranked": ranked_total,
            "windows_over_budget": skipped_over_budget,
            "windows_bridged": bridged,
            "files_scanned": inventory.objects.len(),
            "content_bytes": used_bytes,
            "files_without_complete_text": content_truncated,
            "inventory_truncated": inventory.truncated,
            "scan_errors": inventory.errors,
            "token_counter": "physis_core::rag::count_tokens (heuristic, not the caller's BPE)",
        }))
    }

    /// What this workspace's own record says is likely to happen if you run
    /// this.
    ///
    /// The log already holds every `run` this interface executed, with the exit
    /// status reality gave it. Nothing read those back at the moment a decision
    /// was made, which is a write with no read one level up from the symbols
    /// rule 1 checks. This is the read.
    ///
    /// Measured on 9466 recorded actions from agent transcripts (E71,
    /// `research/E71_ANTICIPATING_OUTCOMES.md`): the failure rate of *this kind
    /// of action*, accumulated across sessions, carries essentially all of the
    /// available signal — AUC 0.620 against 0.500 for a base rate, and online
    /// within-session adaptation did not improve on it. So this returns the
    /// prior and the counts behind it, and claims nothing else.
    pub fn predict(&self, argv: &[String]) -> Result<Value> {
        self.predict_with(argv, None)
    }

    /// `predict`, optionally backing off to another store's record.
    ///
    /// A kind this workspace has never run is exactly the case where its own
    /// log says nothing, and E73 measured what to do about it: a prior from
    /// another machine transferred as well as the target's own history when the
    /// kinds overlapped, and not at all when they did not. So a borrowed prior
    /// is used only above [`PRIOR_COVERAGE_GATE`], or when the local log is
    /// empty (the cold start, where borrowed beat own at 0.635 against 0.494),
    /// and the coverage that decided it is in the answer either way.
    pub fn predict_with(&self, argv: &[String], shared: Option<&Path>) -> Result<Value> {
        ensure!(!argv.is_empty(), "predict needs the argv it would run");
        let kind = action_kind(argv);
        let local = Tally::from_runs(&self.records()?, Some(&kind));
        let shared_tally = match shared {
            None => None,
            Some(path) => {
                let log = if path.is_dir() {
                    path.join("observations.jsonl")
                } else {
                    path.to_path_buf()
                };
                ensure!(
                    log.exists(),
                    "shared prior log not found: {}",
                    log.display()
                );
                let records = observe::read(&log)?;
                Some((log, Tally::from_runs(&records, Some(&kind))))
            }
        };
        let coverage = shared_tally
            .as_ref()
            .and_then(|(_, other)| local.coverage_by(other));
        let local_runs_of_kind = local.per_kind.get(&kind).copied().unwrap_or((0, 0)).0;

        // Order: this workspace's own record for this kind, then a borrowed one
        // if the gate allows, then this workspace's own global rate.
        let borrowed_allowed = shared_tally.as_ref().is_some_and(|(_, other)| {
            other.per_kind.contains_key(&kind)
                && (local.runs == 0 || coverage.is_some_and(|c| c >= PRIOR_COVERAGE_GATE))
        });
        let (probability, source) = if local_runs_of_kind > 0 {
            (local.smoothed(&kind), "workspace")
        } else if borrowed_allowed {
            let (_, other) = shared_tally.as_ref().unwrap();
            (other.smoothed(&kind), "shared")
        } else if local.runs > 0 {
            (local.smoothed(&kind), "workspace base rate")
        } else {
            return Ok(json!({
                "kind": kind,
                "status": "NOT MEASURED",
                "reason": if shared.is_some() {
                    "neither this workspace's log nor the shared store has a completed run of this kind"
                } else {
                    "this workspace's log holds no completed run, so there is no prior to read"
                },
                "runs_of_this_kind": 0, "runs_total": 0,
                "shared_coverage": coverage, "log": self.log_path(),
            }));
        };

        let shared_view = shared_tally.as_ref().map(|(log, other)| {
            let (runs, failures) = other.per_kind.get(&kind).copied().unwrap_or((0, 0));
            json!({
                "log": log,
                "runs_of_this_kind": runs,
                "failures_of_this_kind": failures,
                "runs_total": other.runs,
                "failure_rate": other.rate(),
                "coverage_of_this_workspace": coverage,
                "coverage_gate": PRIOR_COVERAGE_GATE,
                "used": source == "shared",
            })
        });
        let (kind_runs, kind_failures) = local.per_kind.get(&kind).copied().unwrap_or((0, 0));
        let mut recent = local.recent.clone();
        recent.reverse();
        recent.truncate(5);
        Ok(json!({
            "kind": kind,
            "argv": argv,
            "failure_probability": probability,
            "source": source,
            "runs_of_this_kind": kind_runs,
            "failures_of_this_kind": kind_failures,
            "runs_total": local.runs,
            "failures_total": local.failures,
            "workspace_failure_rate": local.rate(),
            "shared": shared_view,
            "recent": recent,
            "method": "Laplace-smoothed failure rate of this action kind, from the log named in `source`",
            "evidence": "E71: the per-kind prior carried the signal (AUC 0.620 vs 0.500). \
        E73: it transfers between machines (0.644 vs an in-corpus 0.642) when coverage is high, \
        and not at 0.24-0.64 coverage",
            "not_claimed": "process exit only; a command can exit 0 and still not do what was wanted",
            "history_window": HISTORY_WINDOW,
        }))
    }

    pub fn read(&self, target: &str, max_bytes: usize) -> Result<Value> {
        if let Some(seq) = target.strip_prefix("obs:") {
            let seq: u64 = seq
                .parse()
                .context("observation ID must be obs:<integer>")?;
            let record = self
                .records()?
                .into_iter()
                .find(|o| o.seq == seq)
                .context("observation not found for this workspace in the history window")?;
            return Ok(json!({"id":target, "record":record, "log":self.log_path()}));
        }
        // `path:120-180` reads one region. Pack and find report line ranges, and
        // re-reading the whole file to see forty of its lines is where an
        // interface stops being cheaper than `sed -n`.
        let (target, lines) = split_line_range(target);
        let path = if target.starts_with("file:") {
            let objects = self.inventory()?.objects;
            let object = objects
                .iter()
                .find(|o| o.id == target)
                .or_else(|| {
                    // A full SHA-256 costs 43 tokens to print and to repeat, so
                    // displays abbreviate it. Any unambiguous prefix resolves;
                    // an ambiguous one is an error, never a silent first match.
                    let mut matches = objects.iter().filter(|o| o.id.starts_with(target));
                    let first = matches.next();
                    match (first, matches.next()) {
                        (Some(o), None) => Some(o),
                        _ => None,
                    }
                })
                .context("file ID not found, or the prefix matches more than one file")?;
            self.root.join(&object.path)
        } else {
            self.root.join(target)
        };
        let path = path.canonicalize().context("file cannot be opened")?;
        ensure!(
            path.starts_with(&self.root),
            "path is outside this workspace"
        );
        ensure!(path.is_file(), "read expects a regular file");
        let object = self.object(&path)?;
        let text = read_prefix(&path, max_bytes as u64)?;
        let whole_bytes = text.len() as u64;
        let (text, range) = match lines {
            None => (text, Value::Null),
            Some((from, to)) => {
                ensure!(
                    from >= 1 && to >= from,
                    "line range must be start-end, 1-indexed"
                );
                let all: Vec<&str> = text.lines().collect();
                ensure!(
                    from <= all.len(),
                    "line {from} is past the end of the readable prefix ({} lines)",
                    all.len()
                );
                let end = to.min(all.len());
                (
                    all[from - 1..end].join("\n"),
                    json!({"start_line": from, "end_line": end, "file_lines": all.len()}),
                )
            }
        };
        Ok(json!({"object":object, "text":text, "lines":range,
            "truncated":object.bytes > whole_bytes,
            "read_at":chrono::Utc::now(), "provenance":"current workspace file; content is not a historical snapshot"}))
    }

    pub(crate) fn record(
        &self,
        source: &str,
        subject: &str,
        mut payload: Value,
    ) -> Result<Observation> {
        fs::create_dir_all(&self.state)?;
        // Cooperative lock for interface writers. Other existing observation
        // writers do not yet acquire this lock; do not claim global transactions.
        let _guard = WriteLock::acquire(self.state.join("system-write.lock"))?;
        payload["workspace"] = json!(self.root);
        let mut records = [Observation::new(source, subject)
            .with_body(serde_json::to_string(&payload)?)
            .by("physis.system.v1")];
        observe::append(&self.log_path(), &mut records)?;
        Ok(records[0].clone())
    }

    pub fn remember(&self, text: &str, outcome: &str, actor: &str) -> Result<Value> {
        ensure!(!text.trim().is_empty(), "remember needs a non-empty note");
        ensure!(text.len() <= 16 * 1024, "note exceeds 16 KiB");
        ensure!(
            ["unverified", "success", "failure", "inconclusive"].contains(&outcome),
            "unknown outcome"
        );
        let record = self.record(
            "system.note",
            text,
            json!({"text":text, "outcome":outcome, "actor":actor}),
        )?;
        Ok(json!({"id":format!("obs:{}", record.seq), "record":record,
            "meaning":"operator-reported development outcome, not classification quality feedback"}))
    }

    /// Recent command runs with intent, exit status and duration — the usage
    /// record `predict` reads, newest first. Read-only; a run that exited 0
    /// may still not have done what was wanted (see `predict`'s not_claimed).
    pub fn runs(&self, limit: usize) -> Result<Value> {
        let mut rows: Vec<Value> = Vec::new();
        for o in self.records()?.into_iter().rev() {
            if o.source != "system.run.finish" {
                continue;
            }
            let payload: Value = serde_json::from_str(&o.body).unwrap_or(Value::Null);
            rows.push(json!({
                "id": format!("obs:{}", o.seq),
                "at": o.at,
                "intent": payload["intent"],
                "actor": payload["actor"],
                "argv": payload["argv"],
                "process_success": payload["process_success"],
                "exit_code": payload["exit_code"],
                "duration_ms": payload["duration_ms"],
            }));
            if rows.len() >= limit.max(1) {
                break;
            }
        }
        let tally = Tally::from_runs(&self.records()?, None);
        Ok(json!({"runs": rows, "count": rows.len(),
            "runs_total": tally.runs, "failures_total": tally.failures,
            "workspace_failure_rate": tally.rate(),
            "history_window": HISTORY_WINDOW, "log": self.log_path()}))
    }

    /// Tool/call usage counters over the workspace observation log: per-source
    /// counts plus per-kind run tallies with failure rates. Read-only rollup
    /// for adoption questions ("what do agents actually call, what fails").
    /// Sources count what is *recorded* — read-only MCP calls leave no
    /// observation, so this is a lower bound on reads, exact on writes/runs.
    pub fn usage(&self) -> Result<Value> {
        use std::collections::BTreeMap;
        let records = self.records()?;
        let mut by_source = BTreeMap::<String, usize>::new();
        for o in &records {
            *by_source.entry(o.source.clone()).or_default() += 1;
        }
        // MCP call audit, when `PHYSIS_MCP_AUDIT=1` was set on the server. Keyed
        // by tool so "what do agents actually call, and what errors" is one step
        // from the raw log. Absent entirely when no audit row exists.
        let mut mcp_tools: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        for o in &records {
            if o.source != "system.mcp.call" {
                continue;
            }
            let payload: Value = serde_json::from_str(&o.body).unwrap_or(Value::Null);
            let tool = payload["tool"].as_str().unwrap_or(&o.subject).to_string();
            let ok = payload["ok"].as_bool().unwrap_or(false);
            let entry = mcp_tools.entry(tool).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += u32::from(!ok);
        }
        let mcp_calls: Vec<Value> = mcp_tools
            .iter()
            .map(|(tool, (calls, errors))| json!({"tool": tool, "calls": calls, "errors": errors}))
            .collect();
        let tally = Tally::from_runs(&records, None);
        let mut kinds: Vec<Value> = tally
            .per_kind
            .iter()
            .map(|(kind, (runs, failures))| {
                let rate = if *runs == 0 {
                    0.0
                } else {
                    *failures as f64 / *runs as f64
                };
                json!({"kind": kind, "runs": runs, "failures": failures,
                    "failure_rate": rate})
            })
            .collect();
        kinds.sort_by(|a, b| {
            b["runs"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&b["runs"].as_u64().unwrap_or(0))
                .then_with(|| {
                    a["kind"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["kind"].as_str().unwrap_or(""))
                })
        });
        Ok(json!({"workspace_records_in_window": records.len(),
            "history_window": HISTORY_WINDOW,
            "by_source": by_source,
            "run_kinds": kinds,
            "runs_total": tally.runs, "failures_total": tally.failures,
            "workspace_failure_rate": tally.rate(),
            "mcp_calls": mcp_calls,
            "mcp_calls_total": mcp_calls.iter().map(|r| r["calls"].as_u64().unwrap_or(0)).sum::<u64>(),
            "audit_env": "PHYSIS_MCP_AUDIT=1",
            "coverage_note": "read-only MCP/CLI queries are not recorded unless the server runs with the audit env; counts are exact for notes/runs, lower-bound for reads",
            "log": self.log_path()}))
    }

    /// Executes the exact program/arguments requested. No shell is inserted.
    /// Logs are files so an arbitrarily verbose child does not fill memory or
    /// corrupt JSON stdout. An interrupted parent leaves a start observation.
    pub fn run(&self, argv: &[String], intent: &str, actor: &str) -> Result<Value> {
        ensure!(!argv.is_empty(), "run requires a program after --");
        ensure!(!intent.trim().is_empty(), "run requires an explicit intent");
        let id = uuid::Uuid::new_v4().to_string();
        let directory = self.state.join("runs").join(&id);
        fs::create_dir_all(&directory)?;
        let stdout = directory.join("stdout.log");
        let stderr = directory.join("stderr.log");
        let out = File::create(&stdout)?;
        let err = File::create(&stderr)?;
        let start = self.record(
            "system.run.start",
            intent,
            json!({"run_id":id,
            "argv":argv, "intent":intent, "actor":actor, "stdout":stdout, "stderr":stderr}),
        )?;
        // What the record said before it ran. Kept in the result so a run and
        // the prior it defied are one record rather than two, and so the loop
        // that writes outcomes is the same loop that reads them.
        let prior = self.predict(argv).ok();
        let timer = std::time::Instant::now();
        let status = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .status();
        let (success, exit_code, error) = match status {
            Ok(status) => (status.success(), status.code(), None),
            Err(error) => (false, None, Some(error.to_string())),
        };
        let payload = json!({"run_id":id, "start_seq":start.seq, "argv":argv,
            "intent":intent, "actor":actor, "process_success":success, "exit_code":exit_code,
            "spawn_error":error, "duration_ms":timer.elapsed().as_millis() as u64,
            "stdout":stdout, "stderr":stderr,
            "assessment":"process exit only; task correctness is not inferred"});
        let finish = self.record("system.run.finish", intent, payload.clone())?;
        Ok(
            json!({"id":id, "prior":prior, "start_id":format!("obs:{}",start.seq),
            "finish_id":format!("obs:{}",finish.seq), "result":payload,
            "stdout_tail":read_tail_text(&stdout, 8192)?, "stderr_tail":read_tail_text(&stderr, 8192)?}),
        )
    }

    /// Creates a NEW ordinary directory. Refuses overwrite. Entries contain
    /// addresses and provenance; opening/editing them does not execute actions.
    pub fn export(&self, destination: &Path) -> Result<Value> {
        let inventory = self.inventory()?;
        let records = self.records()?;
        fs::create_dir(destination)
            .context("export requires a new directory with an existing parent")?;
        let objects = destination.join("objects");
        let history = destination.join("history");
        fs::create_dir(&objects)?;
        fs::create_dir(&history)?;
        let mut index = String::from("# Workspace files\n\nReferences in this dated snapshot; source files remain in the workspace.\n\n");
        for object in &inventory.objects {
            let filename = format!("{}.json", object.id.replace(':', "-"));
            write_json(&objects.join(&filename), object)?;
            let label = object
                .path
                .replace('\\', "\\\\")
                .replace('[', "\\[")
                .replace(']', "\\]")
                .replace(['\n', '\r'], " ");
            index.push_str(&format!("- [{label}](objects/{filename})\n"));
        }
        fs::write(destination.join("INDEX.md"), index)?;
        for record in &records {
            write_json(
                &history.join(format!("obs-{}.json", record.seq)),
                &json!({"id":format!("obs:{}",record.seq), "record":record, "log":self.log_path()}),
            )?;
        }
        let manifest = json!({"api_version":API_VERSION, "kind":"reference-snapshot",
            "created_at":chrono::Utc::now(), "workspace":self.root, "state":self.state,
            "files":inventory.objects.len(), "observations":records.len(),
            "inventory_truncated":inventory.truncated, "scan_errors":inventory.errors,
            "history_window":HISTORY_WINDOW, "live":false});
        write_json(&destination.join("capabilities.json"), &self.capabilities())?;
        fs::write(destination.join("README.md"),
            "# Physis workspace snapshot\n\nOpen [INDEX.md](INDEX.md) to browse files by path. objects/ contains file references; history/ contains observations.\n\nUse physis system read <id> against the original workspace to inspect the current object.\nThis directory is a dated snapshot, not a live mount. Editing it does not change source files or execute commands.\nSee manifest.json for scope, limits, and time.\n")?;
        // Publish the manifest last: its presence marks a completed projection.
        write_json(&destination.join("manifest.json"), &manifest)?;
        Ok(json!({"directory":destination, "manifest":manifest}))
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    writeln!(file)?;
    Ok(())
}

/// Failure counts per action kind, read out of any observation log.
///
/// Kept separate from [`Workspace`] because the point of E73 is that this can
/// come from *another* workspace: a prior fitted on one machine predicted
/// another machine's failures as well as that machine's own history did
/// (0.644 against an in-corpus 0.642), provided the kinds overlapped.
#[derive(Debug, Default)]
struct Tally {
    per_kind: BTreeMap<String, (u32, u32)>, // kind -> (runs, failures)
    runs: u32,
    failures: u32,
    recent: Vec<Value>,
}

impl Tally {
    fn from_runs(records: &[Observation], kind_wanted: Option<&str>) -> Self {
        let mut tally = Tally::default();
        for record in records {
            if record.source != "system.run.finish" {
                continue;
            }
            let payload: Value = serde_json::from_str(&record.body).unwrap_or(Value::Null);
            let Some(argv) = payload.get("argv").and_then(Value::as_array) else {
                continue;
            };
            let argv: Vec<String> = argv
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if argv.is_empty() {
                continue;
            }
            // A missing success flag is not evidence of failure, so it is
            // skipped rather than assumed either way.
            let Some(success) = payload.get("process_success").and_then(Value::as_bool) else {
                continue;
            };
            let kind = action_kind(&argv);
            tally.runs += 1;
            tally.failures += u32::from(!success);
            let entry = tally.per_kind.entry(kind.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += u32::from(!success);
            if kind_wanted == Some(kind.as_str()) {
                tally.recent.push(json!({
                    "seq": record.seq, "argv": argv, "failed": !success,
                    "exit_code": payload.get("exit_code").cloned().unwrap_or(Value::Null),
                }));
            }
        }
        tally
    }

    fn rate(&self) -> f64 {
        if self.runs == 0 {
            0.0
        } else {
            f64::from(self.failures) / f64::from(self.runs)
        }
    }

    /// Laplace toward the store's own global rate, so one unlucky first run of
    /// a kind does not read as a 100% failure rate.
    fn smoothed(&self, kind: &str) -> f64 {
        let (runs, failures) = self.per_kind.get(kind).copied().unwrap_or((0, 0));
        (f64::from(failures) + 2.0 * self.rate()) / (f64::from(runs) + 2.0)
    }

    /// E73's gate: the share of *this* store's runs whose kind the other store
    /// has also seen. Every transferring pair measured 0.93-0.96; every failing
    /// pair 0.24-0.64. Computable before trusting a borrowed prior.
    fn coverage_by(&self, other: &Tally) -> Option<f64> {
        if self.runs == 0 {
            return None;
        }
        let covered: u32 = self
            .per_kind
            .iter()
            .filter(|(kind, _)| other.per_kind.contains_key(*kind))
            .map(|(_, (runs, _))| *runs)
            .sum();
        Some(f64::from(covered) / f64::from(self.runs))
    }
}

/// Below this share of local actions in kinds the other store has seen, a
/// borrowed prior is reported but not used. E73: transfer held at 0.93-0.96 and
/// failed at 0.24-0.64; 0.90 is the round number between them, and it is a
/// threshold rather than a law.
const PRIOR_COVERAGE_GATE: f64 = 0.90;

/// The coarse action type a prior is kept for: the program being run, without
/// its path, or the interface operation. E71 found this granularity is where
/// the predictive signal lives — finer (the exact argv) is too sparse to
/// accumulate, coarser (just "a run") is the base rate.
fn action_kind(argv: &[String]) -> String {
    let Some(program) = argv.first() else {
        return "?".to_string();
    };
    let base = Path::new(program)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| program.clone());
    // `cargo test` and `cargo build` fail for entirely different reasons, and
    // the subcommand is free to read.
    match argv.get(1) {
        Some(sub) if !sub.starts_with('-') && base_takes_subcommand(&base) => {
            format!("{base} {sub}")
        }
        _ => base,
    }
}

fn base_takes_subcommand(base: &str) -> bool {
    matches!(
        base,
        "cargo" | "git" | "npm" | "pnpm" | "yarn" | "just" | "docker" | "python3" | "python"
    )
}

/// Split a trailing `:start-end` line range off a read target. A path
/// containing a colon that is not a line range is returned untouched.
fn split_line_range(target: &str) -> (&str, Option<(usize, usize)>) {
    let Some((head, tail)) = target.rsplit_once(':') else {
        return (target, None);
    };
    let Some((from, to)) = tail.split_once('-') else {
        return (target, None);
    };
    match (from.parse::<usize>(), to.parse::<usize>()) {
        (Ok(from), Ok(to)) if !head.is_empty() => (head, Some((from, to))),
        _ => (target, None),
    }
}

fn read_prefix(path: &Path, max: u64) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?.take(max).read_to_end(&mut bytes)?;
    ensure!(!bytes.contains(&0), "binary file; text read is unavailable");
    // Truncation may split the final UTF-8 character; lossy decoding only
    // affects presentation, never the source file.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_tail_text(path: &Path, max: u64) -> Result<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max)))?;
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub(crate) struct WriteLock(PathBuf);
impl WriteLock {
    pub(crate) fn acquire(path: PathBuf) -> Result<Self> {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let guard = Self(path);
                writeln!(file, "pid={}", std::process::id())?;
                Ok(guard)
            }
            Err(error) => bail!("cannot acquire {}: {error}; another writer may be active; inspect owner before removing a stale lock", path.display()),
        }
    }
}
impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_diff_reports_changes_and_stale_base() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let file = root.join("src/a.txt");
        std::fs::write(&file, "line one\nline two\n").unwrap();

        let ws = Workspace::open(&root, &dir.path().join("state")).unwrap();

        // Unchanged vs baseline → empty hunks (apply_hunks is the round-trip).
        let baseline_same = vec![("src/a.txt".to_string(), "line one\nline two\n".to_string())];
        let preview = ws.stage_diff(&baseline_same).unwrap();
        assert_eq!(preview.len(), 1);
        assert!(preview[0].1.is_empty(), "no diff against identical baseline");

        // Changed → hunks reconstruct to the new text.
        let baseline_new = vec![("src/a.txt".to_string(), "line one\nline TWO\n".to_string())];
        let preview = ws.stage_diff(&baseline_new).unwrap();
        let hunks = &preview[0].1;
        assert!(!hunks.is_empty(), "changed baseline must produce hunks");
        let rebuilt = crate::diff::apply_hunks(&baseline_new[0].1, hunks);
        let current = std::fs::read_to_string(&file).unwrap();
        assert_eq!(rebuilt, current, "hunks must reconstruct the working-tree file");

        // Stale base: file removed from the working tree → error, not silent skip.
        std::fs::remove_file(&file).unwrap();
        let preview = ws.stage_diff(&baseline_new);
        assert!(
            matches!(preview, Err(ref e) if e.to_string().contains("stale-base")),
            "missing file must surface stale-base, got: {preview:?}"
        );
    }
}
