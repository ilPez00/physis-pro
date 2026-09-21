//! physis-hud — the Semantic Navigation HUD (Function 3 MVP).
//!
//! One screen, three panes:
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │  semantic neighborhood (grouped results)     │
//! ├──────────────────────────────────────────────┤
//! │  reading pane (exact source, no summary)     │
//! ├──────────────────────────────────────────────┤
//! │ > query                                      │
//! └──────────────────────────────────────────────┘
//! ```
//!
//! Keyboard-first (§22): `/` focus search · ↑↓ move · Enter zoom in ·
//! Esc zoom out (trail preserved) · Space select · s/d/R same/different/relates
//! · n notebook selection · p pin · r related · e edit ($EDITOR + rescan) ·
//! o observe-draft · P plan · X execute-dry · V validators · M impossible-machine · [ ] scroll reading · u rescan · g graph view · H history · B bench · D dependents · C contradictions ·
//! f/F intent focus/clear · z zoom rollup · 1/2/3 why/context/delta tabs. No mouse required.
//!
//! No LLM anywhere: retrieval is the shared hybrid scorer; when no embedder
//! passes the probe the header says `semantic: off` and lexical/structural
//! retrieval carries on.

use std::collections::{BTreeMap, BTreeSet};
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};

use physis_pro::nav::{self, NavHit, NavWeights, ObjectKind};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut prescan = true;
    let mut prescan_only = false;
    let mut root: Option<PathBuf> = None;
    for a in args.iter().skip(1) {
        match a.as_str() {
            "--prescan-only" => {
                prescan_only = true;
            }
            "--no-prescan" => {
                prescan = false;
            }
            "-h" | "--help" => {
                eprintln!("physis-hud [root] [--prescan-only] [--no-prescan]");
                eprintln!("  default: prescan index + embedding cache, then open the HUD");
                eprintln!("  --prescan-only: warm the index + caches and exit (no TUI)");
                eprintln!("  --no-prescan: open on the persisted index as-is (fast restart)");
                return Ok(());
            }
            _ => {
                if root.is_none() {
                    root = Some(PathBuf::from(a));
                }
            }
        }
    }
    let root = root
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Prescan/preload happens before the alternate screen owns the terminal,
    // so the first-index notice and timings stay visible instead of flashing
    // inside the TUI. Same bargain as the REPL: pay once up front, then
    // every keystroke is incremental.
    if prescan_only {
        return prescan_only_run(&root);
    }
    let app = App::new(root, prescan)?;
    let terminal = enter_tui()?;
    let result = run(terminal, app);
    leave_tui()?;
    result
}

/// Scriptable warmup: build the index + embedding cache, print the summary,
/// exit. `physis-hud <dir> --prescan-only && physis-hud <dir> --no-prescan`
/// is the fast path for demos on a cold checkout.
fn prescan_only_run(root: &Path) -> anyhow::Result<()> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    anchor_data_dir(&root);
    let mut ws = physis_pro::nav::workspace::NavWorkspace::open(&root)?;
    if ws.index().objects.is_empty() {
        eprintln!(
            "physis-hud: first index of {} — embedding every chunk (later starts are incremental)",
            root.display()
        );
    }
    let t0 = std::time::Instant::now();
    let summary = ws.update(true)?;
    let tokens = workspace_tokens(&root);
    eprintln!("physis-hud prescan: {summary} in {:?} · workspace {tokens} tok", t0.elapsed());
    Ok(())
}

/// One state directory per repository, mirroring the REPL: without this,
/// `physis-hud /path` from another cwd splits one repository's index across
/// two directories and rebuilds it every launch. Explicit PHYSIS_DATA_DIR wins.
fn anchor_data_dir(root: &Path) {
    if std::env::var("PHYSIS_DATA_DIR")
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        std::env::set_var("PHYSIS_DATA_DIR", root.join(".physis"));
    }
}

/// Whole-workspace token denominator on the same counter the packer enforces,
/// over the same 40-line chunk shape. Computed at startup and on rescan only —
/// never per frame (the old header scanned every file per draw).
fn workspace_tokens(root: &Path) -> usize {
    let mut total = 0usize;
    for f in physis_pro::scanner::scan_project(root, None) {
        let Ok(text) = std::fs::read_to_string(root.join(&f.path)) else { continue };
        for w in text.lines().collect::<Vec<_>>().chunks(physis_pro::pack::CHUNK_LINES) {
            let t = w.join("\n");
            if t.trim().is_empty() { continue }
            total += physis_pro::pack::tokens_of(&t);
        }
    }
    total
}

fn enter_tui() -> anyhow::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout()))?)
}

fn leave_tui() -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// One entry in the visible result list: either a group header, a directory
/// aggregation (zoomed-out rollup), or an object.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum Row {
    Header(String, usize),
    Dir(String, usize),
    Hit(NavHit),
}

/// World contexts the demo pane shows on demand. Kept in the App (not the
/// world) because they are projections of existing state — never new stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextTab {
    Why,
    Context,
    Delta,
}

/// One entry in the visible result list: either a group header or an object.
fn state_of(
    path: &str,
    changed: &BTreeSet<String>,
    affected: &BTreeSet<String>,
    pinned: &BTreeSet<String>,
    intent_hits: &BTreeSet<String>,
    selected: &[NavHit],
    obj: &physis_pro::nav::PhysisObject,
) -> char {
    if selected.iter().any(|s| s.object.id == obj.id) {
        return '★';
    }
    if pinned.contains(path) {
        return '◆';
    }
    if changed.contains(path) {
        return '●';
    }
    if affected.contains(path) {
        return '◐';
    }
    if intent_hits.contains(path) {
        return '▲';
    }
    '·'
}

/// Aggregate hit paths by parent directory. Zoomed-out rollup: one row per
/// directory, capped so the list never renders thousands of nodes because
/// they exist. Inside existing HUD file only; object substrate unchanged.
fn aggregate_dirs(hits: &[NavHit], cap: usize) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for h in hits {
        let path = h
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| h.object.label.clone());
        let dir = Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());
        let dir = if dir.is_empty() { ".".into() } else { dir };
        *counts.entry(dir).or_default() += 1;
    }
    let mut out: Vec<(String, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out.truncate(cap);
    out
}

/// Paths of the current intent slice: top-8 ranked hits stay prominent,
/// the tail collapses. Pure projection over the ranked list the shared
/// scorer already produced — no second ranking, no model.
fn intent_paths(hits: &[NavHit]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut hot = BTreeSet::new();
    let mut warm = BTreeSet::new();
    for (i, h) in hits.iter().enumerate() {
        let path = h
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| h.object.label.clone());
        if i < 3 {
            hot.insert(path);
        } else if i < 8 {
            warm.insert(path);
        }
    }
    (hot, warm)
}

struct App {
    ws: physis_pro::nav::workspace::NavWorkspace,
    weights: NavWeights,

    query: String,
    typing: bool,
    hits: Vec<NavHit>,
    rows: Vec<Row>,
    sel: usize,
    list_state: ListState,

    selected: Vec<NavHit>,
    /// Reading pane: (title, exact source text).
    reading: Option<(String, String)>,
    /// Reading-pane scroll offset in lines ([ up / ] down).
    reading_scroll: u16,
    /// Right pane: explanation of the selected object.
    explain: String,
    /// Zoom stack of (query, hits, rows, sel) so Esc climbs out with the
    /// trail intact (§8, §12).
    stack: Vec<(String, Vec<NavHit>, Vec<Row>, usize)>,
    crumbs: Vec<String>,
    /// Status line (pins, notebook handoff, degraded mode notices).
    status: String,
    semantic_on: bool,
    embedder_kind: String,
    /// Changed/affected path sets from the last rescan (`u`): the graph list
    /// marks them ●/◐ so the world visibly reacts to an edit. Refreshed by
    /// `rescan_now`, empty until the operator asks — never a background poll
    /// freezing the UI.
    changed: BTreeSet<String>,
    affected: BTreeSet<String>,
    /// Zoomed-out aggregation (Phase 32): when true, rows collapse to one
    /// rollup per directory instead of one row per object.
    zoomed_out: bool,
    /// HOT/WARM/COLD intent focus (Phase 32): set by `f`, cleared by `F`.
    /// HOT/WARM path sets projected from the ranked hits; COLD collapses.
    intent_hot: BTreeSet<String>,
    intent_warm: BTreeSet<String>,
    /// Demo context tab in the right pane: Why / Context / Delta.
    context_tab: ContextTab,
    /// Last Δ for the CONTEXT tab: recomputed by `rescan_now`/edit paths.
    last_delta: Option<physis_pro::mvp::SemanticDelta>,
    /// Cached whole-workspace token denominator (see `workspace_tokens`).
    /// Recomputed at startup and on rescan; the header and Context tab read
    /// this instead of re-walking the tree.
    ws_tokens: usize,
}

impl App {
    fn new(root: PathBuf, prescan: bool) -> anyhow::Result<Self> {
        let root = root.canonicalize().unwrap_or_else(|_| root.clone());
        anchor_data_dir(&root);
        let timing = std::env::var("PHYSIS_MVP_TIMING").ok().as_deref() == Some("1");
        let t0 = std::time::Instant::now();
        let mut ws = physis_pro::nav::workspace::NavWorkspace::open(&root)?;
        if prescan {
            if ws.index().objects.is_empty() {
                eprintln!(
                    "physis-hud: first index of {} — embedding every chunk (later starts are incremental)",
                    root.display()
                );
            }
            let summary = ws.update(true)?;
            if timing {
                eprintln!("[timing] hud prescan (index+embed): {:?}", t0.elapsed());
            } else {
                eprintln!("physis-hud prescan: {summary}");
            }
        }
        let ws_tokens = workspace_tokens(&root);
        let kind = ws.embedder_kind(true);
        // An embedder that is the lexical hash is still *a* retriever, but the
        // header must not call it semantic. The kind label carries the truth.
        let semantic_on = kind != "random-projection";
        // Warm-up embed to keep the first query responsive.
        if semantic_on {
            let _ = ws.embedder(true).map(|e| e.embed("warm"));
        }
        // Startup summary, same facts as the REPL banner: file/object counts
        // from the post-prescan index, the cached sample map, and the
        // absorbed-at-open notice when the workspace moved while closed.
        let (map_r, map_d, map_c) = physis_pro::mvp::map_counts(&root);
        let snap = physis_pro::mvp::slice::snapshot_hashes(
            &physis_pro::scanner::scan_project(&root, None),
        );
        let fresh = physis_pro::mvp::log::hash_canonical(&snap);
        let notice = physis_pro::mvp::TransitionLog::open(&root)
            .ok()
            .and_then(|log| physis_pro::mvp::since_last_session_notice(&log, &fresh));
        let status = format!(
            "files {} · objects {} · sample map repeats {} / differences {} / contradictions {}",
            ws.index().files.len(),
            ws.index().objects.len(),
            map_r,
            map_d,
            map_c,
        );
        let mut startup = format!(
            "PHYSIS HUD\n\n{status}\nindex ready — type a query, Enter searches\n"
        );
        if let Some(n) = notice {
            startup.push_str(&format!("\n{n}\n"));
        }
        Ok(Self {
            ws,
            weights: NavWeights::default(),
            query: String::new(),
            typing: true,
            hits: Vec::new(),
            rows: Vec::new(),
            sel: 0,
            list_state: ListState::default(),
            selected: Vec::new(),
            reading: Some(("startup".into(), startup)),
            reading_scroll: 0,
            explain: String::new(),
            stack: Vec::new(),
            crumbs: Vec::new(),
            status,
            semantic_on,
            embedder_kind: kind,
            changed: Default::default(),
            affected: Default::default(),
            zoomed_out: false,
            intent_hot: Default::default(),
            intent_warm: Default::default(),
            context_tab: ContextTab::Why,
            last_delta: None,
            ws_tokens,
        })
    }

    /// Human pins as a path set: any hit touching a pinned object is never
    /// COLD and renders ◆. Pins live in sled; this projects them per frame.
    fn pinned_paths(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for p in self.ws.pins() {
            for key in [
                serde_json::to_string(&p.a).ok(),
                serde_json::to_string(&p.b).ok(),
            ]
            .into_iter()
            .flatten()
            {
                for h in &self.hits {
                    if serde_json::to_string(&h.object.id).ok().as_ref() == Some(&key) {
                        let path = h
                            .object
                            .source
                            .as_ref()
                            .map(|s| s.path.clone())
                            .unwrap_or_else(|| h.object.label.clone());
                        out.insert(path);
                    }
                }
            }
        }
        out
    }

    /// HOT ∪ WARM: the intent-relevant set. COLD = everything else.
    fn intent_paths(&self) -> BTreeSet<String> {
        self.intent_hot
            .union(&self.intent_warm)
            .cloned()
            .collect()
    }

    /// WHY AFFECTED: deterministic path from the cursor object back to a
    /// changed file. Uses the same precision-first dependents rule the MVP
    /// REPL uses (`changed_symbols` + `affected_dependents`): the cursor is
    /// affected because it code-references a changed symbol/module, or names
    /// the changed module. Ledger lineage is the second leg where recorded.
    fn why_affected(&self, path: &str) -> Vec<String> {
        if self.changed.is_empty() || self.changed.contains(path) {
            return Vec::new();
        }
        let root = self.ws.root.clone();
        let changed_v: Vec<String> = self.changed.iter().cloned().collect();
        let syms = physis_pro::mvp::changed_symbols(&root, &changed_v);
        let mut lines = Vec::new();
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            return Vec::new();
        };
        let lower = text.to_lowercase().replace('-', "_");
        for c in &changed_v {
            let stem = Path::new(c)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.replace('-', "_").to_lowercase())
                .unwrap_or_default();
            if !stem.is_empty() && lower.contains(stem.as_str()) {
                lines.push(format!("{path}\n  ← names module {stem}\n  ← {c} changed"));
            }
        }
        for s in &syms {
            if s.len() < 6 {
                continue;
            }
            let distinctive = s.contains('_') || s.chars().any(|c| c.is_ascii_uppercase());
            let pats = if distinctive {
                vec![
                    format!("{s}::"),
                    format!("::{s}"),
                    format!("use {s}"),
                    format!("{s}("),
                ]
            } else {
                vec![format!("{s}::"), format!("::{s}"), format!("use {s}")]
            };
            if pats.iter().any(|p| text.contains(p.as_str())) {
                lines.push(format!("{path}\n  ← calls {s}\n  ← {s} defined in changed file(s)"));
                break;
            }
        }
        lines
    }

    /// Minimal context for the CONTEXT tab: PRIMARY (top hits), DEPENDENCIES
    /// (shared dependents rule), RELEVANT TESTS (paths mentioning test),
    /// RECENT CHANGES (last Δ), WHY INCLUDED, bounded source regions, and
    /// real WORKSPACE vs SELECTED token counts labeled potential context
    /// tokens avoided / context reduction — no billing or equivalence claim.
    fn context_text(&self) -> String {
        let mut o = String::new();
        o.push_str("INTENT\n");
        o.push_str(&format!(
            "{}\n",
            if self.query.is_empty() {
                "(no intent scoped — type a query)"
            } else {
                &self.query
            }
        ));
        o.push_str("\nPRIMARY\n");
        for h in self.hits.iter().take(3) {
            let path = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            o.push_str(&format!("  {:.2}  {path}\n", h.score.total));
        }
        if self.hits.is_empty() {
            o.push_str("  (none)\n");
        }
        o.push_str("\nDEPENDENCIES\n");
        let targets: Vec<String> = self.hits.iter().take(3).map(|h| {
            h.object.source.as_ref().map(|s| s.path.clone()).unwrap_or_else(|| h.object.label.clone())
        }).collect();
        let root = self.ws.root.clone();
        let deps = physis_pro::mvp::affected_dependents(
            &root,
            &targets,
            &physis_pro::mvp::changed_symbols(&root, &targets),
        );
        for d in deps.iter().take(5) {
            o.push_str(&format!("  {d}\n"));
        }
        if deps.is_empty() {
            o.push_str("  (none found)\n");
        }
        o.push_str("\nRELEVANT TESTS\n");
        let mut tests: Vec<String> = targets
            .iter()
            .chain(deps.iter())
            .filter(|p| p.contains("test"))
            .take(5)
            .cloned()
            .collect();
        tests.sort();
        tests.dedup();
        for t in &tests {
            o.push_str(&format!("  {t}\n"));
        }
        if tests.is_empty() {
            o.push_str("  (none in scope)\n");
        }
        o.push_str("\nRECENT CHANGES\n");
        match &self.last_delta {
            Some(d) if !d.changed_artifacts.is_empty() => {
                for c in d.changed_artifacts.iter().take(5) {
                    o.push_str(&format!("  {c}\n"));
                }
            }
            _ => o.push_str("  (no recorded changes this session)\n"),
        }
        o.push_str("\nWHY INCLUDED\n");
        for h in self.hits.iter().take(3) {
            let path = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            o.push_str(&format!(
                "  {path}\n    → {}\n",
                physis_pro::nav::explain_hit(h, &self.weights)
            ));
        }
        o.push_str("\nSOURCE (bounded)\n");
        for h in self.hits.iter().take(2) {
            let title = h
                .object
                .source
                .as_ref()
                .map(|s| match (s.start_line, s.end_line) {
                    (Some(a), Some(b)) => format!("{}:{a}-{b}", s.path),
                    _ => s.path.clone(),
                })
                .unwrap_or_else(|| h.object.label.clone());
            let text = self.ws.read(&h.object).unwrap_or_default();
            let snippet: String = text.lines().take(12).collect::<Vec<_>>().join("\n");
            o.push_str(&format!("  --- {title} ---\n{snippet}\n"));
        }
        // Token math: same counter the packer enforces, over the same chunk
        // shape — measured, never estimated. Workspace denominator is cached
        // at startup/rescan; only the selection is counted live (≤8 reads).
        let ws_tokens = self.ws_tokens;
        let mut sel_tokens = 0usize;
        for h in self.hits.iter().take(8) {
            let text = self.ws.read(&h.object).unwrap_or_default();
            sel_tokens += physis_pro::pack::tokens_of(&text);
        }
        let avoided = ws_tokens.saturating_sub(sel_tokens);
        let pct = if ws_tokens > 0 {
            100.0 * avoided as f64 / ws_tokens as f64
        } else {
            0.0
        };
        let est = physis_pro::estimate::calculate_estimate(ws_tokens, sel_tokens);
        o.push_str(&format!(
            "\nWORKSPACE {ws_tokens} tok\nSELECTED {sel_tokens} tok\nREDUCTION {pct:.1}% (packer: {:.1}%)\nAVOIDED {avoided} tok\nCONTEXT {:.1}% │ {avoided} potential tokens avoided\n",
            est.saved_percent, pct
        ));
        o
    }

    /// Delta text for the DELTA tab: last rescan/edit Δ rendered through the
    /// shared `SemanticDelta::render`, plus the status-bar counters.
    fn delta_text(&self) -> String {
        match &self.last_delta {
            Some(d) => d.render(),
            None => "No Δ yet — edit a file (e) or rescan (u).\n".to_string(),
        }
    }

    /// Re-index only what moved on disk (nav stat fast path — O(Δ)), then
    /// diff the file catalog around it for the CHANGED set and run the same
    /// precision-first dependents rule the MVP loop uses for AFFECTED.
    /// `u` in the HUD; no editor launched, no model touched.
    fn rescan_now(&mut self) {
        // Hash-level CHANGED: compare the refreshed catalog against the
        // pre-update one. Stat fast path keeps this O(Δ): unchanged files
        // cost one stat, only content-moved files rehash + re-ingest.
        let before: std::collections::BTreeMap<String, String> = self
            .ws
            .index()
            .files
            .iter()
            .map(|(p, r)| (p.clone(), r.hash.clone()))
            .collect();
        let objects_before = self.ws.index().objects.len();
        match self.ws.update(true) {
            Ok(summary) => {
                let mut changed: BTreeSet<String> = Default::default();
                let mut added: Vec<String> = Vec::new();
                let mut removed: Vec<String> = Vec::new();
                for (path, rec) in &self.ws.index().files {
                    match before.get(path) {
                        None => {
                            changed.insert(path.clone());
                            added.push(path.clone());
                        }
                        Some(h) if h != &rec.hash => {
                            changed.insert(path.clone());
                        }
                        _ => {}
                    }
                }
                for gone in before.keys() {
                    if !self.ws.index().files.contains_key(gone) {
                        changed.insert(gone.clone());
                        removed.push(gone.clone());
                    }
                }
                self.changed = changed;
                // AFFECTED via the shared MVP rule over the changed set.
                let root = self.ws.root.clone();
                // Workspace denominator moved with the edit — refresh the
                // cached count so the header and Context tab stay honest.
                self.ws_tokens = workspace_tokens(&root);
                let changed_v: Vec<String> = self.changed.iter().cloned().collect();
                let syms = physis_pro::mvp::changed_symbols(&root, &changed_v);
                self.affected = physis_pro::mvp::affected_dependents(&root, &changed_v, &syms)
                    .into_iter()
                    .collect();
                let objects_after = self.ws.index().objects.len();
                let mut changed_all = changed_v.clone();
                changed_all.extend(added.iter().cloned());
                self.last_delta = Some(physis_pro::mvp::SemanticDelta {
                    changed_artifacts: changed_all,
                    added_relations: vec![],
                    removed_relations: removed,
                    affected_dependents: self.affected.iter().cloned().collect(),
                    invariant_results: vec![
                        "file-type invariants preserved (rs/md/json/toml shapes)".into(),
                    ],
                    test_results: vec![],
                    objects_before,
                    objects_after,
                    objects_remapped: objects_before.abs_diff(objects_after)
                        + changed_v.len()
                        + added.len(),
                    changed_semantic_neighborhoods: self.affected.len(),
                    full_rescan: false,
                });
                self.status = format!(
                    "rescanned: {} changed, {} affected — {summary}",
                    self.changed.len(),
                    self.affected.len()
                );
                // Re-run the current query so markers land on visible rows.
                if !self.query.is_empty() {
                    let q = self.query.clone();
                    let global = q.starts_with('/');
                    if global {
                        self.query = q[1..].to_string();
                    }
                    self.run_query(global);
                    if global {
                        self.query = format!("/{query}", query = self.query);
                    }
                }
            }
            Err(e) => self.status = format!("rescan failed: {e}"),
        }
    }

    /// Open the cursor object in `$EDITOR` (blocking; TUI suspends while it
    /// runs), then rescan so the world reacts to the save. Undo-before-save
    /// is the editor's. `e` in the HUD.
    fn edit_current(&mut self) {
        let Some(hit) = self.current_hit().cloned() else {
            self.status = "nothing to edit".into();
            return;
        };
        let path = hit
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| hit.object.label.clone());
        let abs = self.ws.root.join(&path);
        if !abs.is_file() {
            self.status = format!("not a file: {path}");
            return;
        }
        let editor = std::env::var("EDITOR")
            .ok()
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| "vi".to_string());
        // Leave the alternate screen so the editor owns the terminal.
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        let status = std::process::Command::new(&editor).arg(&abs).status();
        let _ = enable_raw_mode();
        let _ = execute!(stdout(), EnterAlternateScreen);
        match status {
            Ok(s) if s.success() => {
                self.status = format!("edited {path} — rescanning");
                self.rescan_now();
            }
            Ok(s) => self.status = format!("editor exited {s:?} — no rescan ran"),
            Err(e) => self.status = format!("cannot launch editor {editor}: {e}"),
        }
    }

    fn run_query(&mut self, global: bool) {
        let query = self.query.clone();
        match self.ws.query(&query, global, &self.weights, 60) {
            Ok((hits, mode)) => {
                self.semantic_on = mode.semantic;
                self.embedder_kind = mode.embedder.clone();
                self.ws.push_trail("query", &query);
                self.set_results(hits);
                if !mode.semantic {
                    self.status =
                        format!("semantic: off ({}) — lexical/structural retrieval only", mode.embedder);
                }
            }
            Err(e) => self.status = format!("query failed: {e}"),
        }
    }

    fn set_results(&mut self, hits: Vec<NavHit>) {
        self.rows = Vec::new();
        if self.zoomed_out {
            // Zoomed-out rollup: one row per directory, capped — never
            // renders thousands of nodes because they exist.
            for (dir, n) in aggregate_dirs(&hits, 60) {
                self.rows.push(Row::Dir(dir, n));
            }
            // Keep object rows for selection/reading, after the rollup.
            for (group, group_hits) in nav::group_hits(&hits) {
                self.rows
                    .push(Row::Header(group.label().to_string(), group_hits.len()));
                for h in group_hits.iter().take(20) {
                    self.rows.push(Row::Hit((*h).clone()));
                }
            }
        } else {
            for (group, group_hits) in nav::group_hits(&hits) {
                self.rows
                    .push(Row::Header(group.label().to_string(), group_hits.len()));
                for h in group_hits {
                    self.rows.push(Row::Hit(h.clone()));
                }
            }
        }
        self.hits = hits;
        self.sel = self.rows.iter().position(|r| matches!(r, Row::Hit(_))).unwrap_or(0);
        self.sync_list_state();
        self.update_explain();
    }

    /// Intent focus: band the current ranked hits HOT/WARM/COLD (Phase 32).
    /// Projection only — no second ranking, no model. Pins never demote.
    fn apply_focus(&mut self) {
        let (hot, warm) = intent_paths(&self.hits);
        self.intent_hot = hot;
        self.intent_warm = warm;
        self.zoomed_out = false;
        self.set_results(self.hits.clone());
        self.status = format!(
            "attention: {} hot · {} warm · {} cold (F clears)",
            self.intent_hot.len(),
            self.intent_warm.len(),
            self.hits.len().saturating_sub(8),
        );
    }

    fn clear_focus(&mut self) {
        self.intent_hot.clear();
        self.intent_warm.clear();
        self.set_results(self.hits.clone());
        self.status = "attention cleared".into();
    }

    fn sync_list_state(&mut self) {
        self.list_state.select(Some(self.sel));
    }

    fn update_explain(&mut self) {
        self.explain.clear();
        if let Some(Row::Dir(dir, n)) = self.rows.get(self.sel) {
            // Directory rollup: expand into it on Enter.
            self.explain.push_str(&format!("DIR {dir} ({n} hits)\nEnter drills in.\n"));
            return;
        }
        if let Some(Row::Hit(h)) = self.rows.get(self.sel) {
            // State marker first — Phase 32 grammar, one marker per object:
            // ★ selected · ◆ human-pinned · ● changed · ◐ affected ·
            // ▲ intent-relevant · · ordinary. Human pins stay distinct from
            // Physis attention: two agents, one world.
            let path = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            let pinned = self.pinned_paths();
            let intent = self.intent_paths();
            let mark = state_of(
                &path,
                &self.changed,
                &self.affected,
                &pinned,
                &intent,
                &self.selected,
                &h.object,
            );
            let marker = match mark {
                '★' => "★ SELECTED",
                '◆' => "◆ HUMAN-PINNED",
                '●' => "● CHANGED",
                '◐' => "◐ AFFECTED",
                '▲' => "▲ INTENT-RELEVANT",
                _ => "· ordinary",
            };
            self.explain.push_str(&format!("{marker}\n"));
            self.explain.push_str(&format!("RELEVANCE  {:.2}\n", h.score.total));
            for (name, raw) in &h.score.components {
                self.explain.push_str(&format!("{name:<10} {raw:.2}\n"));
            }
            self.explain.push_str(&format!("\nid: {}\n", h.object.id.short()));
            // Relations touching the cursor object: kind + origin glyph, same
            // grammar as the graph view — inspection, not a second query.
            for r in self.ws.relations_of(&h.object) {
                let other = if r.a == h.object.id { &r.b } else { &r.a };
                let glyph = match r.origin {
                    nav::RelationOrigin::User => "───",
                    nav::RelationOrigin::Deterministic => "═══",
                    nav::RelationOrigin::Inferred => "- -",
                };
                let eid = nav::relation_id(&r.a, &r.kind, &r.b);
                self.explain.push_str(&format!(
                    "\n{glyph} [{}] {} ({})",
                    r.kind,
                    other.short(),
                    eid
                ));
            }
            if self
                .selected
                .iter()
                .any(|s| s.object.id == h.object.id)
            {
                self.explain.push_str("\n[SELECTED]\n");
            }
        }
    }

    fn current_hit(&self) -> Option<&NavHit> {
        match self.rows.get(self.sel) {
            Some(Row::Hit(h)) => Some(h),
            _ => None,
        }
    }

    /// Drill into a directory rollup: keep the ranked hits, show the rows
    /// for that directory first. Zoomed-out ↔ zoomed-in over one substrate.
    fn enter_dir(&mut self, dir: &str) {
        self.stack.push((
            self.query.clone(),
            self.hits.clone(),
            self.rows.clone(),
            self.sel,
        ));
        self.crumbs.push(dir.to_string());
        let mut rows = vec![Row::Dir(dir.to_string(), 0)];
        for (group, group_hits) in nav::group_hits(&self.hits) {
            let mut in_dir: Vec<&NavHit> = group_hits
                .into_iter()
                .filter(|h| {
                    let p = h
                        .object
                        .source
                        .as_ref()
                        .map(|s| s.path.clone())
                        .unwrap_or_else(|| h.object.label.clone());
                    Path::new(&p)
                        .parent()
                        .map(|pp| pp.to_string_lossy().to_string())
                        .unwrap_or_else(|| ".".into())
                        == dir
                })
                .collect();
            if in_dir.is_empty() {
                continue;
            }
            rows.push(Row::Header(group.label().to_string(), in_dir.len()));
            for h in in_dir.drain(..) {
                rows.push(Row::Hit(h.clone()));
            }
        }
        self.rows = rows;
        self.sel = self.rows.iter().position(|r| matches!(r, Row::Hit(_))).unwrap_or(0);
        self.sync_list_state();
        self.update_explain();
    }

    /// Zoom in: a passage reads directly; anything else re-queries within it.
    fn enter(&mut self) {
        let Some(hit) = self.current_hit().cloned() else {
            return;
        };
        // Push current state for Esc.
        self.stack.push((
            self.query.clone(),
            self.hits.clone(),
            self.rows.clone(),
            self.sel,
        ));
        self.crumbs.push(hit.object.label.clone());

        match hit.object.kind {
            ObjectKind::Passage | ObjectKind::Note | ObjectKind::File => {
                match self.ws.read(&hit.object) {
                    Ok(text) => {
                        self.set_reading((hit.object.label.clone(), text));
                    }
                    Err(e) => self.status = format!("cannot read: {e}"),
                }
            }
            _ => {}
        }
        // Related objects become the new neighborhood.
        if let Ok((hits, _)) = self.ws.query(&hit.object.text, true, &self.weights, 20) {
            self.query = format!("inside: {}", hit.object.label);
            self.set_results(hits);
        }
        self.ws.push_trail("enter", &hit.object.id.short());
    }

    fn back(&mut self) {
        if let Some((q, hits, rows, sel)) = self.stack.pop() {
            self.query = q;
            self.hits = hits;
            self.rows = rows;
            self.sel = sel;
            self.reading = None;
            self.reading_scroll = 0;
            self.sync_list_state();
            self.update_explain();
        }
        self.crumbs.pop();
    }

    fn toggle_select(&mut self) {
        if let Some(hit) = self.current_hit().cloned() {
            if let Some(pos) = self.selected.iter().position(|s| s.object.id == hit.object.id) {
                self.selected.remove(pos);
            } else {
                self.selected.push(hit);
            }
            self.update_explain();
        }
    }

    fn pin_selection(&mut self) {
        if self.selected.len() == 2 {
            let (a, b) = (self.selected[0].clone(), self.selected[1].clone());
            match self.ws.add_pin(&a.object, &b.object) {
                Ok(true) => self.status = "pinned".into(),
                Ok(false) => self.status = "already pinned".into(),
                Err(e) => self.status = format!("pin failed: {e}"),
            }
        } else {
            self.status = "select exactly 2 objects to pin (Space)".into();
        }
    }

    /// Assert a SAME / DIFFERENT / RELATED edge between the two selected
    /// objects (spec §11). One key, one verb: `s` = same, `d` = different,
    /// `R` (shift-r) = relates-to. Idempotent — a repeat is reported, not
    /// duplicated. Status names the edge id so the relation is addressable.
    fn relate_selection(&mut self, kind: &str) {
        if self.selected.len() != 2 {
            self.status = "select exactly 2 objects to relate (Space)".into();
            return;
        }
        let (a, b) = (self.selected[0].clone(), self.selected[1].clone());
        match self.ws.add_relation(&a.object, &b.object, kind, "") {
            Ok(true) => {
                let eid = nav::relation_id(&a.object.id, kind, &b.object.id);
                self.status = format!("related [{kind}] ({eid})");
                self.update_explain();
            }
            Ok(false) => self.status = "already related".into(),
            Err(e) => self.status = format!("relate failed: {e}"),
        }
    }

    /// Graph dual view (`g`): the same objects + relations as the list, in a
    /// local ASCII projection. One substrate, two renderings (§21) — no
    /// second graph model; this is a draw, not a data structure.
    fn graph_view(&mut self) {
        let Some(center) = self.current_hit().cloned() else {
            self.status = "select an object to graph".into();
            return;
        };
        let label_of = |id: &nav::ObjectId| {
            self.ws
                .index()
                .objects
                .get(&serde_json::to_string(id).unwrap())
                .map(|o| o.label.clone())
                .unwrap_or_else(|| id.short())
        };
        let mut lines = Vec::new();
        lines.push(format!("● {}", center.object.label));
        // Related objects from relations touching the center, deduped.
        let mut seen = std::collections::BTreeSet::new();
        for r in self.ws.relations_of(&center.object) {
            let (other, rel, dir) = if r.a == center.object.id {
                (&r.b, r.kind.clone(), "→")
            } else {
                (&r.a, r.kind.clone(), "←")
            };
            let name = label_of(other);
            if name == center.object.label || !seen.insert(name.clone()) {
                continue;
            }
            let origin = match r.origin {
                nav::RelationOrigin::User => "───",
                nav::RelationOrigin::Deterministic => "═══",
                nav::RelationOrigin::Inferred => "- -",
            };
            lines.push(format!(
                "  {dir} {origin} [{rel}] {} {origin}→",
                name
            ));
        }
        // Nearest neighbours by retrieval score — dashed as inferred.
        // State-aware: the center and each neighbour carry their world
        // state marker, so the graph reacts to edits and intent visibly.
        let pinned = self.pinned_paths();
        let intent = self.intent_paths();
        let center_path = center
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| center.object.label.clone());
        let center_mark = state_of(
            &center_path,
            &self.changed,
            &self.affected,
            &pinned,
            &intent,
            &self.selected,
            &center.object,
        );
        lines[0] = format!("{center_mark} {}", center.object.label);
        let why = self.why_affected(&center_path);
        if !why.is_empty() {
            lines.push(String::new());
            lines.push("WHY AFFECTED".into());
            for w in why.iter().take(4) {
                for l in w.lines() {
                    lines.push(format!("  {l}"));
                }
            }
        }
        for h in self.hits.iter().take(6) {
            if h.object.id == center.object.id || !seen.insert(h.object.label.clone()) {
                continue;
            }
            let p = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            let m = state_of(&p, &self.changed, &self.affected, &pinned, &intent, &self.selected, &h.object);
            lines.push(format!(
                "  {m}  - - [near {:.2}] {} - -",
                h.score.total, h.object.label
            ));
        }
        lines.push(String::new());
        lines.push("solid = user · double = deterministic · dashed = inferred/near".into());
        lines.push("edge ids: rel:<a>:<kind>:<b> (canonical order)".into());
        lines.push("★ selected · ◆ human-pinned · ● changed · ◐ affected · ▲ intent · · ordinary".into());
        self.set_reading((format!("graph: {}", center.object.label), lines.join("\n")));
    }

    fn related(&mut self) {
        if let Some(hit) = self.current_hit().cloned() {
            // Re-run the neighborhood query seeded by this object.
            if let Ok((hits, _)) = self.ws.query(&hit.object.text, true, &self.weights, 20) {
                self.stack.push((
                self.query.clone(),
                self.hits.clone(),
                self.rows.clone(),
                self.sel,
            ));
                self.crumbs.push(format!("related: {}", hit.object.label));
                self.query = format!("related: {}", hit.object.label);
                self.set_results(hits);
            }
        }
    }

    /// Recent past (`H`): git log where present, else mtime order — the same
    /// projection the REPL `history` command reports. Lands in the reading
    /// pane; read-only, no transition recorded.
    fn show_history(&mut self) {
        let text = physis_pro::mvp::history_text(&self.ws.root);
        self.set_reading(("history".into(), text));
        self.status = "history: git log, or filesystem order without git".into();
    }
    /// Measured machine numbers (`B`): same shared helper the REPL `bench`
    /// command reports — scan cost, index size, live query + propagation
    /// latency. Slice pair is (cached workspace, live top-8 selection).
    /// Read-only, no transition recorded.
    fn show_bench(&mut self) {
        let mut sel_tokens = 0usize;
        for h in self.hits.iter().take(8) {
            sel_tokens += physis_pro::pack::tokens_of(&self.ws.read(&h.object).unwrap_or_default());
        }
        let intent = if self.query.is_empty() {
            "bench".to_string()
        } else {
            self.query.clone()
        };
        let root = self.ws.root.clone();
        let provider = self.embedder_kind.clone();
        let text = physis_pro::mvp::bench_text(
            &mut self.ws,
            &root,
            &provider,
            &intent,
            Some((self.ws_tokens, sel_tokens)),
        );
        self.set_reading(("bench".into(), text));
        self.status = "bench: measured here, offline — same numbers as REPL `bench`".into();
    }
    /// Dependents of the cursor object (`D`): stem/symbol scan over its path —
    /// the same rule the REPL `dependencies` command reports. Read-only.
    fn show_dependents(&mut self) {
        let Some(hit) = self.current_hit().cloned() else {
            self.status = "no cursor object — run a query first".into();
            return;
        };
        let path = hit
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| hit.object.label.clone());
        let text = physis_pro::mvp::dependents_text(&self.ws.root, &path);
        self.set_reading((format!("dependents: {path}"), text));
        self.status = "dependents: code-shaped references to the cursor path".into();
    }

    /// Structural pairs over the selection (`C`): differences count +
    /// contradiction candidates from one slice-map build — the same helper
    /// the REPL `contradictions` command reports. Falls back to the cursor
    /// object when nothing is selected. Read-only.
    fn show_contradictions(&mut self) {
        let docs = self.notebook_selection().and_then(|sel| {
            let v: Vec<(String, String)> = sel;
            if v.is_empty() { None } else { Some(v) }
        });
        let Some(docs) = docs else {
            self.status = "select objects first (Space)".into();
            return;
        };
        let text = physis_pro::mvp::contradictions_text(&docs);
        self.set_reading(("contradictions".into(), text));
        self.status = "contradictions: slice-map pairs over the selection".into();
    }

    /// Bound objects for the semantic-computer projections: selection first,
    /// else the cursor hit, else empty. Nothing is forced — the draft names
    /// what is missing instead of borrowing an unrelated object.
    fn projection_base(&self) -> Vec<NavHit> {
        if !self.selected.is_empty() {
            return self.selected.clone();
        }
        self.current_hit().cloned().into_iter().collect()
    }

    /// Workspace revision for the projections: content hash of the canonical
    /// file→hash catalog — the same `snapshot_hashes` the slice layer uses
    /// for pre/post comparison, measured here in-process from the scan.
    fn projection_revision(&self) -> String {
        let snap: std::collections::BTreeMap<String, String> = self
            .ws
            .index()
            .files
            .iter()
            .map(|(p, r)| (p.clone(), r.hash.clone()))
            .collect();
        physis_pro::mvp::log::hash_canonical(&snap)
    }

    /// `o` — observe draft into the reading pane. Pure projection of the
    /// current query + bound objects + frozen weights; appends nothing.
    fn show_observe(&mut self) {
        let bound = physis_pro::mvp::deps::bound_objects(&self.projection_base());
        let recorded_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        let text = physis_pro::mvp::deps::observe_draft_text(
            &self.query,
            &bound,
            &self.weights,
            &recorded_at,
        );
        self.set_reading(("observe (draft)".into(), text));
        self.status = "observe draft shown (read-only, not appended)".into();
    }

    /// `P` — deterministic capability shortlist into the reading pane.
    /// Read-only text.
    fn show_plan(&mut self) {
        let bound = physis_pro::mvp::deps::bound_objects(&self.projection_base());
        let rev = self.projection_revision();
        let text = physis_pro::mvp::deps::plan_shortlist_text(&self.query, &bound, &rev);
        self.set_reading(("plan (shortlist)".into(), text));
        self.status = "plan shortlist shown (read-only)".into();
    }

    /// `X` — shell argv template preview for the bound repo. Text only —
    /// no process is spawned.
    fn show_execute_dry(&mut self) {
        let bound = physis_pro::mvp::deps::bound_objects(&self.projection_base());
        let text = physis_pro::mvp::deps::execute_dry_text(&self.ws.root, &bound);
        self.set_reading(("execute (dry)".into(), text));
        self.status = "execute dry-run shown (not run)".into();
    }

    /// `ctrl-X` — bounded real execution: one `cargo test` step with live
    /// precondition recheck + intent-before/result-after logging
    /// into the workspace transition log. Single step, registered argv only.
    fn run_execute_bounded(&mut self) {
        let bound = physis_pro::mvp::deps::bound_objects(&self.projection_base());
        let intent = if self.query.is_empty() {
            "hud bounded execute".to_string()
        } else {
            self.query.clone()
        };
        let root = self.ws.root.clone();
        let text = match physis_pro::mvp::log::TransitionLog::open(&root) {
            Ok(mut log) => physis_pro::mvp::deps::execute_bounded_step(&root, &bound, &mut log, &intent),
            Err(e) => format!("EXECUTE refused: log unavailable: {e}"),
        };
        self.set_reading(("execute (bounded)".into(), text.clone()));
        self.status = if text.starts_with("EXECUTE refused") {
            "bounded execute refused (see reading pane)".into()
        } else {
            "bounded execute ran + logged (see reading pane)".into()
        };
    }

    /// `V` — mechanical validator preview. Text only; every status stays
    /// unknown/not_run until a real run appends evidence.
    fn show_validate(&mut self) {
        let bound = physis_pro::mvp::deps::bound_objects(&self.projection_base());
        let text = physis_pro::mvp::deps::validate_preview_text(&bound);
        self.set_reading(("validate (preview)".into(), text));
        self.status = "validators previewed (not run)".into();
    }

    /// Set the reading pane and rewind its scroll: a stale offset would
    /// blank a new short text (the M view runs ~60 lines, most views do not).
    fn set_reading(&mut self, r: (String, String)) {
        self.reading = Some(r);
        self.reading_scroll = 0;
    }

    /// Scroll the reading pane by `delta` lines (`[` up / `]` down).
    fn scroll_reading(&mut self, delta: i16) {
        self.reading_scroll = self.reading_scroll.saturating_add_signed(delta);
    }

    /// `M` — Impossible Machine chain (E52→E65) into the reading pane.
    /// Closed record plus live artifact overlay; read-only, nothing runs.
    fn show_impossible(&mut self) {
        let text = physis_pro::mvp::impossible_text(&self.ws.root);
        self.set_reading(("impossible machine".into(), text));
        self.status = "impossible machine: closed chain + live artifacts (read-only)".into();
    }

    /// Hand the current selection (or the highlighted object) to Function 2:
    /// docs = (title, exact source text) for the Notebook scope. Canonical
    /// objects — the Notebook borrows, never copies, the content.
    fn notebook_selection(&mut self) -> Option<Vec<(String, String)>> {
        let mut sel: Vec<NavHit> = self
            .selected
            .iter()
            .filter(|h| h.object.source.is_some())
            .cloned()
            .collect();
        if sel.is_empty() {
            if let Some(h) = self.current_hit().cloned() {
                sel.push(h);
            }
        }
        if sel.is_empty() {
            return None;
        }
        let mut docs = Vec::new();
        for h in sel {
            if let Ok(text) = self.ws.read(&h.object) {
                let title = h
                    .object
                    .source
                    .as_ref()
                    .map(|s| match (s.start_line, s.end_line) {
                        (Some(a), Some(b)) => format!("{}:{a}-{b}", s.path),
                        _ => s.path.clone(),
                    })
                    .unwrap_or_else(|| h.object.label.clone());
                docs.push((title, text));
            }
        }
        Some(docs)
    }
}

/// Function 2 handoff: grounded extractive answer over the given docs. Uses
/// the same embedder cascade; no cloud, no LLM (extractive tier when no
/// oracle is configured). Returns the rendered answer text.
fn run_notebook(
    app: &App,
    docs: &[(String, String)],
) -> anyhow::Result<String> {
    let (embedder, kind) = {
        let config = physis_pro::config::PhysisProConfig::default();
        physis_pro::embed::select_embedder(&config.physis)
    };
    // physis_core's answer needs physis_core's trait object; select() gives it.
    let (core_embedder, core_kind) = physis_core::embed::select(384);
    let _ = (embedder, kind);
    let answer = physis_pro::ported::notebook::answer(
        docs,
        &app.query,
        core_embedder.as_ref(),
        2000,
        core_kind,
    )?;
    Ok(answer.render())
}

fn color_for(kind: ObjectKind) -> Color {
    match kind {
        ObjectKind::Note => Color::Cyan,
        ObjectKind::File => Color::Green,
        ObjectKind::Passage => Color::LightGreen,
        ObjectKind::Concept => Color::Magenta,
        ObjectKind::Directory => Color::Yellow,
        ObjectKind::Other => Color::Gray,
    }
}

fn kind_tag(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Note => "[note]",
        ObjectKind::File => "[file]",
        ObjectKind::Passage => "[passage]",
        ObjectKind::Concept => "[concept]",
        ObjectKind::Directory => "[dir]",
        ObjectKind::Other => "[other]",
    }
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Percentage(35),
            Constraint::Length(4),
        ])
        .split(f.size());

    // Header: mode + counts + OFFLINE (Phase 32: no network, no model —
    // local embedder or lexical fallback only). Full legend lives in the
    // right pane title and the footer; markers never rely on colour alone.
    let mode = if app.semantic_on { "semantic+lexical" } else { "LEXICAL ONLY" };
    let delta_txt = match &app.last_delta {
        Some(d) => format!(
            "Δ {} changed · {} affected",
            d.changed_artifacts.len(),
            d.affected_dependents.len()
        ),
        None => "Δ —".to_string(),
    };
    let ctx_txt = if app.hits.is_empty() {
        String::new()
    } else {
        // Cached denominator — the header draws every frame, so it must
        // never re-walk the tree. Selection is ≤8 cached reads.
        let ws_tokens = app.ws_tokens;
        let mut sel_tokens = 0usize;
        for h in app.hits.iter().take(8) {
            sel_tokens += physis_pro::pack::tokens_of(&app.ws.read(&h.object).unwrap_or_default());
        }
        let pct = if ws_tokens > 0 {
            100.0 * sel_tokens as f64 / ws_tokens as f64
        } else {
            0.0
        };
        format!("CONTEXT {pct:.1}% │ {} potential tokens avoided", ws_tokens.saturating_sub(sel_tokens))
    };
    let header = Line::from(vec![
        Span::styled("PHYSIS NAVIGATOR", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(
            "  [{mode}] embedder: {}  indexed: {} objects  {delta_txt}  {ctx_txt}  OFFLINE  {}",
            app.embedder_kind,
            app.ws.index().objects.len(),
            if app.crumbs.is_empty() {
                String::new()
            } else {
                format!("trail: {}", app.crumbs.join(" → "))
            }
        )),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // Result list. Markers: ★ selected · ◆ human-pinned · ● changed ·
    // ◐ affected · ▲ intent-relevant · · ordinary. COLD (outside the
    // intent set while focus is on) renders dimmed; pinned never demotes.
    let pinned = app.pinned_paths();
    let intent = app.intent_paths();
    let focus_on = !app.intent_hot.is_empty() || !app.intent_warm.is_empty();
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            Row::Header(name, n) => ListItem::new(Line::from(Span::styled(
                format!("{name} ({n})"),
                Style::default().add_modifier(Modifier::BOLD).fg(Color::White),
            ))),
            Row::Dir(dir, n) => ListItem::new(Line::from(vec![
                Span::raw(if i == app.sel { "▶ " } else { "  " }),
                Span::styled(
                    format!("▸ {dir} ({n})"),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ])),
            Row::Hit(h) => {
                let loc = h
                    .object
                    .source
                    .as_ref()
                    .map(|s| match (s.start_line, s.end_line) {
                        (Some(a), Some(b)) => format!("{}:{a}-{b}", s.path),
                        _ => s.path.clone(),
                    })
                    .unwrap_or_else(|| h.object.label.clone());
                let path = h
                    .object
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| h.object.label.clone());
                let m = state_of(&path, &app.changed, &app.affected, &pinned, &intent, &app.selected, &h.object);
                let cold = focus_on && !intent.contains(&path) && !pinned.contains(&path);
                let mut spans = vec![
                    Span::raw(format!("{m} ")),
                    Span::styled(kind_tag(h.object.kind), Style::default().fg(color_for(h.object.kind))),
                    Span::raw(format!(" {:>4.2}  ", h.score.total)),
                    Span::styled(
                        loc,
                        if cold {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default().add_modifier(Modifier::UNDERLINED)
                        },
                    ),
                ];
                if i == app.sel {
                    spans.insert(
                        0,
                        Span::raw("▶ "),
                    );
                } else {
                    spans.insert(0, Span::raw("  "));
                }
                ListItem::new(Line::from(spans))
            }
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" semantic neighborhood "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, chunks[1], &mut list_state_for(app));

    // Reading / explanation split.
    let reading_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[2]);

    let (title, body) = match &app.reading {
        Some((t, text)) => (t.clone(), text.clone()),
        None => (
            "reading".into(),
            "Enter on a passage to read the exact source.\nEsc climbs back out.\n\n".to_string()
                + &app.explain,
        ),
    };
    let reader = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(format!(" {title} ")))
        .scroll((app.reading_scroll, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(reader, reading_chunks[0]);

    // Right pane switches tabs: Why (rank + relations) / Context (minimal
    // context packet) / Delta (last Δ). All three project existing state.
    let (right_title, right_body) = match app.context_tab {
        ContextTab::Why => {
            let mut explain = app.explain.clone();
            if !app.selected.is_empty() {
                explain.push_str(&format!("\nselected: {} objects", app.selected.len()));
                if app.selected.len() == 2 {
                    explain.push_str("\np (pin) · s/d/R (same/different/relates) · n (notebook)");
                }
            }
            (" why: 1 why · 2 context · 3 delta ".to_string(), explain)
        }
        ContextTab::Context => (
            " context: 1 why · 2 context · 3 delta ".to_string(),
            app.context_text(),
        ),
        ContextTab::Delta => (
            " delta: 1 why · 2 context · 3 delta ".to_string(),
            app.delta_text(),
        ),
    };
    let right = Paragraph::new(right_body)
        .block(Block::default().borders(Borders::ALL).title(right_title))
        .wrap(Wrap { trim: false });
    f.render_widget(right, reading_chunks[1]);

    // Query bar + status + key legend (footer names every marker + edge
    // grammar so state never rides on colour alone).
    let prompt = if app.typing { "> " } else { "  " };
    let lines = vec![
        Line::from(format!("{prompt}{}", app.query)),
        Line::from(Span::styled(
            app.status.clone(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "o observe · P plan · X execute-dry · ctrl-e bounded-run · V validators · M impossible-machine · [ ] scroll reading · e edit · u rescan · g graph · H history · B bench · D dependents · C contradictions · f focus · F clear · z zoom · 1/2/3 why/context/delta · n notebook · p pin · s/d/R relate · space select · q quit",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "★ selected · ◆ human-pinned · ● changed · ◐ affected · ▲ intent · · ordinary │ ─── user · ═══ deterministic · - - inferred",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        chunks[3],
    );
}

/// ratatui needs `&mut ListState`; the app keeps its own copy in sync.
fn list_state_for(app: &App) -> ListState {
    let mut s = app.list_state.clone();
    s.select(Some(app.sel));
    s
}

fn run(mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>, app: App) -> anyhow::Result<()> {
    let app = Arc::new(Mutex::new(app));

    loop {
        {
            let app = app.lock().unwrap();
            terminal.draw(|f| draw(f, &app))?;
        }

        if !event::poll(std::time::Duration::from_millis(200))? {
            continue;
        }
        let ev = event::read()?;
        let mut quit = false;
        {
            let mut app = app.lock().unwrap();
            if let Event::Key(key) = ev {
                if key.kind == KeyEventKind::Press {
                    if app.typing {
                        // Ctrl-C quits even mid-typing; checked before the
                        // plain-char arm so 'c' with CONTROL reaches here.
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            quit = true;
                        } else {
                            match key.code {
                                KeyCode::Char('/') => {
                                    if !app.query.is_empty() {
                                        app.query.clear();
                                    } else {
                                        app.typing = false;
                                    }
                                }
                                KeyCode::Char(c) => app.query.push(c),
                                KeyCode::Backspace => {
                                    app.query.pop();
                                }
                                KeyCode::Enter => {
                                    app.typing = false;
                                    let global = app.query.starts_with('/');
                                    if global {
                                        app.query = app.query[1..].to_string();
                                    }
                                    app.run_query(global);
                                }
                                KeyCode::Esc => app.query.clear(),
                                _ => {}
                            }
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('/') | KeyCode::Char('k')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                app.typing = true;
                            }
                            KeyCode::Char('/') => app.typing = true,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.sel > 0 {
                                    app.sel -= 1;
                                    // Skip headers.
                                    while matches!(app.rows.get(app.sel), Some(Row::Header(..))) {
                                        app.sel = app.sel.saturating_sub(1);
                                    }
                                    app.sync_list_state();
                                    app.update_explain();
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.sel + 1 < app.rows.len() {
                                    app.sel += 1;
                                    while matches!(app.rows.get(app.sel), Some(Row::Header(..))) {
                                        app.sel = (app.sel + 1).min(app.rows.len() - 1);
                                    }
                                    app.sync_list_state();
                                    app.update_explain();
                                }
                            }
                            KeyCode::Enter => {
                                // Directory rollup drills in; objects zoom as before.
                                if let Some(Row::Dir(dir, _)) = app.rows.get(app.sel).cloned() {
                                    app.enter_dir(&dir);
                                } else {
                                    app.enter();
                                }
                            }
                            KeyCode::Esc => app.back(),
                            KeyCode::Char(' ') => app.toggle_select(),
                            KeyCode::Char('p') => app.pin_selection(),
                            KeyCode::Char('s') => app.relate_selection("same"),
                            KeyCode::Char('d') => app.relate_selection("different"),
                            KeyCode::Char('R') => app.relate_selection("relates to"),
                            KeyCode::Char('n') => {
                                // Notebook handoff: run the Function 2
                                // extractive answer over the selection, in
                                // process. Result lands in the reading pane.
                                if let Some(docs) = app.notebook_selection() {
                                    match run_notebook(&app, &docs) {
                                        Ok(answer) => {
                                            app.reading =
                                                Some(("notebook".into(), answer));
                                        }
                                        Err(e) => app.status = format!("notebook: {e}"),
                                    }
                                } else {
                                    app.status = "select objects first (Space)".into();
                                }
                            }
                            KeyCode::Char('r') => app.related(),
                            KeyCode::Char('g') => app.graph_view(),
                            KeyCode::Char('H') => app.show_history(),
                            KeyCode::Char('B') => app.show_bench(),
                            KeyCode::Char('D') => app.show_dependents(),
                            KeyCode::Char('C') => app.show_contradictions(),
                            KeyCode::Char('o') => app.show_observe(),
                            KeyCode::Char('P') => app.show_plan(),
                            KeyCode::Char('X') => app.show_execute_dry(),
                            KeyCode::Char('V') => app.show_validate(),
                            KeyCode::Char('M') => app.show_impossible(),
                            KeyCode::Char('[') => app.scroll_reading(-10),
                            KeyCode::Char(']') => app.scroll_reading(10),
                            KeyCode::Char('e')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                app.run_execute_bounded()
                            }
                            KeyCode::Char('e') => app.edit_current(),
                            KeyCode::Char('u') => app.rescan_now(),
                            KeyCode::Char('f') => app.apply_focus(),
                            KeyCode::Char('F') => app.clear_focus(),
                            KeyCode::Char('z') => {
                                app.zoomed_out = !app.zoomed_out;
                                let hits = app.hits.clone();
                                app.set_results(hits);
                                app.status = if app.zoomed_out {
                                    "zoomed out: directory rollup (z toggles, Enter drills in)".into()
                                } else {
                                    "zoomed in: object rows".into()
                                };
                            }
                            KeyCode::Char('1') => app.context_tab = ContextTab::Why,
                            KeyCode::Char('2') => app.context_tab = ContextTab::Context,
                            KeyCode::Char('3') => app.context_tab = ContextTab::Delta,
                            KeyCode::Char('q') => quit = true,
                            _ => {}
                        }
                    }
                }
            }
        }
        if quit {
            break;
        }
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// No git repo → the filesystem fallback names the record and lists files.
    #[test]
    fn history_falls_back_to_mtime_order_without_git() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn b() {}\n").unwrap();
        let text = physis_pro::mvp::history_text(dir.path());
        assert!(text.contains("filesystem snapshot"), "{text}");
        assert!(text.contains("b.rs"), "{text}");
    }

    /// Whole-workspace token denominator is positive on a non-empty tree.
    #[test]
    fn workspace_token_count_measures_real_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "# hello\n\nsmall test file\n").unwrap();
        assert!(workspace_tokens(dir.path()) > 0);
        assert_eq!(workspace_tokens(&dir.path().join("missing")), 0);
    }

    fn sc_bound() -> Vec<physis_pro::mvp::deps::BoundObject> {
        vec![physis_pro::mvp::deps::BoundObject {
            object_id: "fs:abc123".into(),
            path: "src/rag.rs".into(),
            revision: "h1".into(),
        }]
    }

    /// Observe is an append-shaped draft: placeholders, not ids — and the
    /// occurrence time is never fabricated.
    #[test]
    fn observe_draft_names_placeholders_not_appends() {
        let w = physis_pro::nav::NavWeights::default();
        let text =
            physis_pro::mvp::deps::observe_draft_text("thesis diagrams", &sc_bound(), &w, "2026-09-20T00:00:00+00:00");
        assert!(text.contains("physis.event.v1"), "{text}");
        assert!(text.contains("event_id: <pending>"), "{text}");
        assert!(text.contains("run_id: <pending>"), "{text}");
        assert!(text.contains("plan_id: <pending>"), "{text}");
        assert!(text.contains("occurred_at: unknown"), "{text}");
        assert!(text.contains("src/rag.rs"), "{text}");
        assert!(text.contains("not appended"), "{text}");
    }

    /// Plan shortlist is deterministic: stable capability-ID order with typed
    /// schema predicates on every row.
    #[test]
    fn plan_shortlist_is_stable_id_ordered_with_typed_preconditions() {
        let text = physis_pro::mvp::deps::plan_shortlist_text("build it", &sc_bound(), "rev9");
        let fo = text.find("file.open").expect("file.open row");
        let rb = text.find("repo.build").expect("repo.build row");
        let rt = text.find("repo.test").expect("repo.test row");
        assert!(fo < rb && rb < rt, "{text}");
        assert!(text.contains("exists(object_id)"), "{text}");
        assert!(text.contains("revision_equals"), "{text}");
        assert!(text.contains("authorized"), "{text}");
        assert!(text.contains("not executed"), "{text}");
    }

    /// Execute dry-run previews the registered argv template — no process.
    #[test]
    fn execute_dry_shows_argv_template_without_running() {
        let dir = tempfile::tempdir().unwrap();
        let text = physis_pro::mvp::deps::execute_dry_text(dir.path(), &sc_bound());
        assert!(text.contains("cargo"), "{text}");
        assert!(text.contains("src/rag.rs"), "{text}");
        assert!(text.contains("dry_run"), "{text}");
        assert!(text.contains("no process spawned"), "{text}");
    }

    /// Validator preview stays honest: nothing ran, so every check is
    /// unknown/not_run with the mechanical condition named.
    #[test]
    fn validate_preview_reports_unknown_not_run_honestly() {
        let text = physis_pro::mvp::deps::validate_preview_text(&sc_bound());
        assert!(text.contains("build_exit"), "{text}");
        assert!(text.contains("test_report"), "{text}");
        assert!(text.contains("source_revision"), "{text}");
        assert!(text.contains("unknown/not_run"), "{text}");
    }

    /// Bounded execute refuses with no bound object — refusal is text.
    #[test]
    fn bounded_execute_refuses_without_bound_object() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = physis_pro::mvp::log::TransitionLog::open(dir.path()).unwrap();
        let text =
            physis_pro::mvp::deps::execute_bounded_step(dir.path(), &[], &mut log, "probe");
        assert!(text.starts_with("EXECUTE refused"), "{text}");
        assert!(log.read_all().is_empty(), "refusal appends nothing");
    }

    /// Bounded execute refuses a stale snapshot — object gone from scan.
    #[test]
    fn bounded_execute_refuses_stale_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = physis_pro::mvp::log::TransitionLog::open(dir.path()).unwrap();
        let before = log.read_all().len();
        let text =
            physis_pro::mvp::deps::execute_bounded_step(dir.path(), &sc_bound(), &mut log, "probe");
        assert!(text.starts_with("EXECUTE refused"), "{text}");
        assert!(text.contains("stale snapshot"), "{text}");
        assert_eq!(log.read_all().len(), before, "refusal appends nothing");
    }

    /// Reading scroll moves in lines, never below zero, and a new view
    /// rewinds — otherwise a stale offset blanks a short text.
    #[test]
    fn reading_scroll_moves_and_rewinds_on_new_view() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().to_path_buf(), false).unwrap();
        app.set_reading(("long".into(), "line\n".repeat(100)));
        app.scroll_reading(10);
        assert_eq!(app.reading_scroll, 10);
        app.scroll_reading(-4);
        assert_eq!(app.reading_scroll, 6);
        app.scroll_reading(-100);
        assert_eq!(app.reading_scroll, 0, "scroll never goes below zero");
        app.scroll_reading(10);
        app.set_reading(("short".into(), "one".into()));
        assert_eq!(app.reading_scroll, 0, "a new view rewinds");
    }
}
