//! physis-pro-tui — control panel for the physis engine.
//!
//! Four tabs (cycle with Tab):
//!   STATUS   — live engine health (HTTP), embedder, node/edge counts, connection.
//!   MACHINES — pick a philosophical machine and run it on a sample (in-process).
//!   CONFIG   — edit + save ~/.physis/config.json (dream/pdca/scan/embed knobs).
//!   WIKI     — browse the UberWiki by MEANING: describe what a thing was about
//!              and get cosine-ranked nodes back, each showing whether it was
//!              reported as worked/blocked and what it sits next to.
//!
//! Keys: Tab switch · ↑/↓ select · Enter run (machines) · +/- edit (config) ·
//!       s save (config) · r refresh (status) · / search (wiki) · q quit.
//!
//! Env: PHYSIS_URL, PHYSIS_API_TOKEN, PHYSIS_USER (which tenant's wiki to read —
//! coherence state is per-user, so the default `_default` tenant is empty if
//! your nodes were written under `aion` / `praxis`).

use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dotenvy::dotenv;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Terminal,
};

use chrono::{Duration as ChronoDuration, Utc};
use physis_pro::config::PhysisConfig;
use physis_pro::machines::{chronos, elenchus, harmony, lex, polis, soma, techne};
use physis_pro::ported::system::Workspace;
use physis_pro::world_object::{RelationKind, WorldObject};

const TABS: [&str; 5] = ["STATUS", "MACHINES", "CONFIG", "WIKI", "STREAM"];
const TAB_WIKI: usize = 3;
const TAB_STREAM: usize = 4;
const MACHINES: [(&str, &str); 7] = [
    ("chronos", "temporal — rhythms, what's due / slipping"),
    ("techne", "codebase — capabilities + change impact"),
    ("soma", "body — effort/recovery, drift to overreaching"),
    ("elenchus", "dialogue — claim standing (Toulmin)"),
    ("lex", "contracts — breach paths + gaps"),
    ("harmony", "music — intervals + motifs"),
    ("polis", "social — ties, reciprocity, fading"),
];
const CFG_FIELDS: [&str; 5] = [
    "network_scan_interval_secs",
    "dream_batch_size",
    "pdca_stagnant_threshold",
    "pdca_stagnant_window",
    "embed_dim",
];

#[derive(Default)]
struct EngineStatus {
    online: bool,
    embedder: String,
    detail: String,
}

/// One recalled node. `density` is the engine's derived score; `asserted` is the
/// reported verdict, when anyone has judged it (see PLAN.md coherence axes).
struct WikiHit {
    /// Coherence node id — the handle the holarchy walk needs.
    id: String,
    label: String,
    density: f32,
    asserted: Option<f32>,
}

struct App {
    tab: usize,
    machine_sel: usize,
    machine_out: String,
    cfg: PhysisConfig,
    cfg_sel: usize,
    status_msg: String,
    url: String,
    token: Option<String>,
    /// Tenant to browse (`X-Physis-User`). Coherence state is per-user, so
    /// without this the WIKI tab reads the empty `_default` tenant while the
    /// real content sits under `aion` / `praxis` — the tool would look broken.
    user: String,
    /// The engine process, when started from here.
    engine: Option<std::process::Child>,
    /// WIKI tab: browse the UberWiki by meaning rather than by name.
    wiki_query: String,
    /// True while typing a query — keys go to the box, not to navigation.
    wiki_typing: bool,
    wiki_hits: Vec<WikiHit>,
    wiki_sel: usize,
    wiki_preview: String,
    /// Query edited since the last search — drives the debounced live re-rank.
    wiki_dirty: bool,
    /// When the query was last touched, for that debounce.
    wiki_last_input: std::time::Instant,
    /// Graph walk: previous result lists, so Backspace climbs back out.
    wiki_stack: Vec<Vec<WikiHit>>,
    /// Labels of the nodes walked into, shown as a breadcrumb.
    wiki_crumbs: Vec<String>,
    /// Semiotic grid, fetched once — it describes the loaded ontologies, not
    /// tenant state, so it does not change while the engine runs.
    wiki_grid: Vec<GridCell>,
    /// Distinct domains / modes in grid order (rows and columns of the map).
    wiki_domains: Vec<String>,
    wiki_modes: Vec<String>,
    /// True when the heatmap has the keys — arrows then move the cell cursor
    /// instead of the result list. A focus toggle avoids fighting j/k, which
    /// already navigate results.
    wiki_map_focus: bool,
    /// Condition-slider position for the map, 0.0 responsive ..= 1.0 trusting.
    /// Mirrors the studio's range input; `[` and `]` move it.
    wiki_confidence: f32,
    wiki_cell_row: usize,
    wiki_cell_col: usize,
    /// STREAM tab: the navigation path from the root object to the current
    /// one. Every entry is a `WorldObject`; the last is rendered, the previous
    /// ones form the breadcrumb.
    stream_stack: Vec<WorldObject>,
    /// Children of the current object, refreshed on navigation rather than per
    /// frame — a directory's `from_path` children or a file's symbols.
    stream_children: Vec<WorldObject>,
    /// Selected child, when the current object has any.
    stream_sel: usize,
    /// Magic textbox: when open, keys edit `cmd_input` and Enter dispatches it
    /// to a deterministic operator. Opened with `:` from any tab.
    cmd_open: bool,
    cmd_input: String,
    /// STREAM tab: show the current object's typed edges as a graph projection
    /// instead of the object body and child list. Same context, another view.
    stream_graph: bool,
    /// The workspace observation store. Every deterministic action appends one
    /// record here, through the same writer `physis system` and the MCP history
    /// tools read — so a textbox action is recoverable as history, not a
    /// parallel event log. `None` when the store could not be opened.
    obs: Option<Workspace>,
    /// The live agent run, when one was started from the textbox. Shared with
    /// the run thread; rendered in STREAM as a `WorldObject` of kind `Agent`.
    agent_run: Option<Arc<Mutex<AgentRun>>>,
}

/// One `AgentEvent` as the panel shows it: the same fields the SSE endpoint
/// serialises, kept plain so the TUI never depends on `ai::agent` types.
#[derive(Debug, Clone, PartialEq)]
struct AgentEventView {
    round: u32,
    kind: String,
    text: String,
}

/// A run's observable state, updated by the run thread and read by the render
/// loop. The `WorldObject` projection is built from this at draw time, so the
/// view type stays free of runtime state.
#[derive(Debug, Clone)]
struct AgentRun {
    id: String,
    provider: String,
    model: String,
    prompt: String,
    status: String,
    events: Vec<AgentEventView>,
}

impl AgentRun {
    fn new(prompt: &str) -> Self {
        Self {
            id: "pending".into(),
            provider: String::new(),
            model: String::new(),
            prompt: prompt.to_string(),
            status: "running".into(),
            events: Vec::new(),
        }
    }
}

/// Cap on the retained trace. A runaway or hostile engine must not grow the
/// panel's memory without bound; the newest events are the ones worth keeping.
const MAX_AGENT_EVENTS: usize = 500;

/// Append one event, dropping the oldest past the cap. A `done` event carries
/// the server-assigned run id in its text.
fn push_event(run: &Arc<Mutex<AgentRun>>, ev: AgentEventView) {
    if let Ok(mut r) = run.lock() {
        if ev.kind == "done" {
            r.id = ev.text.clone();
        }
        r.events.push(ev);
        let over = r.events.len().saturating_sub(MAX_AGENT_EVENTS);
        if over > 0 {
            r.events.drain(..over);
        }
    }
}

/// One cell of the semiotic grid: a DOMAIN x MODE intersection, how strongly the
/// loaded ontologies populate it, and the entries that define it.
struct GridCell {
    domain: String,
    mode: String,
    activation: f32,
    entries: Vec<String>,
    /// 0..1 health from the server's `cell_condition` — how the cell is doing,
    /// as opposed to `activation`/`entries.len()`, which say how big it is.
    condition: f32,
    /// The classifier's live multiplier for the cell (1.0 neutral).
    standing: f32,
    /// How many human verdicts fed `condition`; 0 means it is an estimate from
    /// population and standing alone.
    verdicts: u32,
}

/// Should a live re-rank fire?
///
/// Every search is an HTTP round-trip *and* an ONNX embed of the query, so
/// firing per keystroke would queue a request behind every letter and make the
/// box feel worse than pressing Enter. Waiting for a pause in typing gives the
/// "results move as you think" behaviour without hammering the engine.
///
/// Short fragments are skipped: 1-2 characters match nearly everything, so they
/// cost a round-trip to produce noise.
fn should_search(dirty: bool, idle_ms: u128, query_len: usize) -> bool {
    dirty && query_len >= 3 && idle_ms >= 350
}

/// Resolve the physis-pro-web binary: sibling of this exe, else rely on PATH.
fn engine_bin() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sib = dir.join("physis-pro-web");
            if sib.exists() {
                return sib;
            }
        }
    }
    std::path::PathBuf::from("physis-pro-web")
}

fn start_engine(app: &mut App) {
    if app.engine.is_some() {
        app.status_msg = "engine already started from here".into();
        return;
    }
    match std::process::Command::new(engine_bin())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => {
            app.status_msg = format!("engine started (pid {})", child.id());
            app.engine = Some(child);
        }
        Err(e) => app.status_msg = format!("start failed: {e} (is physis-pro-web built?)"),
    }
}

fn stop_engine(app: &mut App) {
    match app.engine.take() {
        Some(mut child) => {
            let _ = child.kill();
            let _ = child.wait();
            app.status_msg = "engine stopped".into();
        }
        None => app.status_msg = "no engine started from here (start it with 's')".into(),
    }
}

/// POST /api/v1/config/reload to apply saved config knobs live. Blocks briefly.
fn reload_config(app: &mut App) {
    let url = format!("{}/api/v1/config/reload", app.url);
    let token = app.token.clone();
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            app.status_msg = format!("reload runtime: {e}");
            return;
        }
    };
    app.status_msg = rt.block_on(async move {
        let client = reqwest::Client::new();
        let mut rb = client.post(&url);
        if let Some(t) = token {
            rb = rb.bearer_auth(t);
        }
        match rb.send().await {
            Ok(r) => match r.json::<serde_json::Value>().await {
                Ok(v) => v
                    .get("note")
                    .and_then(|n| n.as_str())
                    .unwrap_or("reloaded")
                    .to_string(),
                Err(_) => "reloaded".into(),
            },
            Err(e) => format!("reload failed: {e}"),
        }
    });
}

fn cfg_path(cfg: &PhysisConfig) -> std::path::PathBuf {
    cfg.data_dir.join("config.json")
}

fn load_config() -> PhysisConfig {
    let def = PhysisConfig::default();
    match std::fs::read_to_string(cfg_path(&def)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or(def),
        Err(_) => def,
    }
}

fn save_config(cfg: &PhysisConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.data_dir)?;
    let s = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(cfg_path(cfg), s)
}

/// Apply +/- to the selected config field.
fn edit_field(cfg: &mut PhysisConfig, field: usize, up: bool) {
    let s = if up { 1.0 } else { -1.0 };
    match field {
        0 => {
            cfg.network_scan_interval_secs =
                (cfg.network_scan_interval_secs as i64 + (s * 5.0) as i64).max(1) as u64
        }
        1 => cfg.dream_batch_size = (cfg.dream_batch_size as i64 + s as i64).max(1) as usize,
        2 => cfg.pdca_stagnant_threshold = (cfg.pdca_stagnant_threshold + s * 0.05).clamp(0.0, 1.0),
        3 => {
            cfg.pdca_stagnant_window = (cfg.pdca_stagnant_window as i64 + s as i64).max(1) as usize
        }
        4 => cfg.embed_dim = (cfg.embed_dim as i64 + (s * 64.0) as i64).max(64) as usize,
        _ => {}
    }
}

fn cfg_value(cfg: &PhysisConfig, field: usize) -> String {
    match field {
        0 => cfg.network_scan_interval_secs.to_string(),
        1 => cfg.dream_batch_size.to_string(),
        2 => format!("{:.2}", cfg.pdca_stagnant_threshold),
        3 => cfg.pdca_stagnant_window.to_string(),
        4 => cfg.embed_dim.to_string(),
        _ => String::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let url = std::env::var("PHYSIS_URL").unwrap_or_else(|_| "http://127.0.0.1:19876".into());
    let token = std::env::var("PHYSIS_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
    // Which tenant's wiki to browse. Matches the aion client's env var.
    let user = std::env::var("PHYSIS_USER").unwrap_or_else(|_| "_default".into());

    let status = Arc::new(Mutex::new(EngineStatus::default()));

    // Background poller: health + embedder, every 3s. Own runtime/thread so the
    // blocking UI loop never waits on the network.
    {
        let status = status.clone();
        let url = url.clone();
        let token = token.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let client = reqwest::Client::new();
            rt.block_on(async move {
                loop {
                    let mut st = EngineStatus::default();
                    let health = client.get(format!("{url}/health")).send().await;
                    st.online = matches!(&health, Ok(r) if r.status().is_success());
                    if st.online {
                        let mut rb = client.get(format!("{url}/api/v1/embedder"));
                        if let Some(t) = &token {
                            rb = rb.bearer_auth(t);
                        }
                        if let Ok(resp) = rb.send().await {
                            if let Ok(v) = resp.json::<serde_json::Value>().await {
                                st.embedder = v
                                    .get("kind")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("?")
                                    .to_string();
                                st.detail = v.to_string();
                            }
                        }
                    }
                    *status.lock().unwrap() = st;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            });
        });
    }

    // One working directory for the navigator root and the observation store,
    // so the actions this panel records land in the same `.physis/system` store
    // that `physis system history` and the MCP history tools read.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let obs = {
        let state = std::env::var_os("PHYSIS_CORE_DIR")
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| cwd.join(".physis/system"));
        Workspace::open(&cwd, &state).ok()
    };

    let mut app = App {
        tab: 0,
        machine_sel: 0,
        machine_out: "press Enter to run".into(),
        cfg: load_config(),
        cfg_sel: 0,
        status_msg: format!("engine: {url}"),
        url,
        token,
        user,
        engine: None,
        wiki_query: String::new(),
        wiki_typing: false,
        wiki_hits: Vec::new(),
        wiki_sel: 0,
        wiki_preview: "press / to search the wiki by meaning".into(),
        wiki_dirty: false,
        wiki_last_input: std::time::Instant::now(),
        wiki_stack: Vec::new(),
        wiki_crumbs: Vec::new(),
        wiki_grid: Vec::new(),
        wiki_domains: Vec::new(),
        wiki_modes: Vec::new(),
        wiki_map_focus: false,
        // Matches ConditionLens::Balanced, so the TUI and the studio open on
        // the same reading of the same grid.
        wiki_confidence: 0.35,
        wiki_cell_row: 0,
        wiki_cell_col: 0,
        stream_stack: vec![WorldObject::from_directory(&cwd)],
        stream_children: Vec::new(),
        stream_sel: 0,
        cmd_open: false,
        cmd_input: String::new(),
        stream_graph: false,
        obs,
        agent_run: None,
    };
    stream_refresh(&mut app);

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let res = run(&mut terminal, &mut app, &status);

    // Kill-on-quit: an engine started from here doesn't outlive the panel.
    if let Some(mut child) = app.engine.take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

/// Blocking GET/POST helper for the WIKI tab. The UI loop is synchronous, so
/// each call spins a current-thread runtime the same way `reload_config` does —
/// searches are user-initiated (Enter), not per-keystroke, so this never runs
/// hot enough to justify a background poller.
fn wiki_fetch(
    app: &App,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let url = format!("{}{}", app.url, path);
    let token = app.token.clone();
    let user = app.user.clone();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let mut rb = match &body {
            Some(_) => client.post(&url),
            None => client.get(&url),
        };
        if let Some(t) = token {
            rb = rb.bearer_auth(t);
        }
        rb = rb.header("X-Physis-User", user);
        if let Some(b) = body {
            rb = rb.json(&b);
        }
        let resp = rb.send().await.map_err(|e| format!("{e}"))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("{e}"))
    })
}

/// Turn a `/coherence/nodes` payload into hits.
///
/// `asserted` is absent for unjudged nodes (the engine skips serialising it),
/// and that absence must survive as `None` rather than collapse to 0.0 — "nobody
/// has judged this" and "judged inert" are different states and the browser
/// shows them differently.
fn parse_hits(v: &serde_json::Value) -> Vec<WikiHit> {
    v.get("results")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .map(|n| WikiHit {
                    id: n
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    label: n
                        .get("label")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    density: n.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                    asserted: n.get("asserted").and_then(|x| x.as_f64()).map(|x| x as f32),
                })
                // An unlabeled node has no readable handle, so it cannot be
                // browsed by content — drop it rather than show a blank row.
                .filter(|h| !h.label.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Recall nodes near the query *by meaning* — the whole point of the browser:
/// you remember what a thing was about, not what it was called.
fn wiki_search(app: &mut App) -> bool {
    let q = app.wiki_query.trim().to_string();
    if q.is_empty() {
        app.status_msg = "type a query first (/)".into();
        return false;
    }
    let path = format!(
        "/api/v1/coherence/nodes?q={}&max=30",
        urlencoding_minimal(&q)
    );
    match wiki_fetch(app, &path, None) {
        Ok(v) => {
            app.wiki_hits = parse_hits(&v);
            app.wiki_sel = 0;
            app.status_msg = format!("{} hits for \"{q}\"", app.wiki_hits.len());
            if app.wiki_hits.is_empty() {
                app.wiki_preview = "nothing near that yet — the wiki only knows what has been \
                                    registered into this tenant"
                    .into();
            } else {
                wiki_load_preview(app);
            }
            true
        }
        Err(e) => {
            app.status_msg = format!("search failed: {e}");
            app.wiki_preview = format!(
                "search failed: {e}\n\nis physis-pro-web running? (STATUS tab, 's' starts it)"
            );
            false
        }
    }
}

/// Neighbours of the selected node — what this thing sits next to in meaning.
fn wiki_load_preview(app: &mut App) {
    let Some(hit) = app.wiki_hits.get(app.wiki_sel) else {
        return;
    };
    let label = hit.label.clone();
    let verdict = match hit.asserted {
        Some(a) if a > 0.2 => format!("reported: WORKED ({a:+.2})"),
        Some(a) if a < -0.2 => format!("reported: BLOCKED ({a:+.2})"),
        Some(a) => format!("reported: inert ({a:+.2})"),
        None => "reported: — (nobody has judged this)".to_string(),
    };
    let head = format!(
        "{label}\n\ndensity: {:.3}   {verdict}\n\nnearest in meaning:\n",
        hit.density
    );
    match wiki_fetch(
        app,
        "/api/v1/reconstruct",
        Some(serde_json::json!({ "input": label })),
    ) {
        Ok(v) => {
            let mut out = head;
            let neighbours = v
                .get("neighbors")
                .and_then(|n| n.as_array())
                .cloned()
                .unwrap_or_default();
            for n in neighbours.iter().take(6) {
                let l = n
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or("(unlabeled)");
                let sim = n
                    .get("cosine_similarity")
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0.0);
                let mark = match n.get("asserted").and_then(|x| x.as_f64()) {
                    Some(a) if a > 0.2 => "+",
                    Some(a) if a < -0.2 => "-",
                    Some(_) => "0",
                    None => " ",
                };
                out.push_str(&format!("  {mark} {sim:.3}  {l}\n"));
            }
            app.wiki_preview = out;
        }
        Err(e) => app.wiki_preview = format!("{head}  (neighbours unavailable: {e})"),
    }
}

/// Block character for a cell's activation, densest first.
///
/// Activation counts how many ontology entries populate a cell, so it is a map
/// of what the *grid* can express — not of what this tenant has recorded. An
/// empty cell means "no ontology covers this intersection", which is exactly
/// what `discover` exists to find.
fn shade(activation: f32) -> char {
    match activation {
        a if a >= 1.5 => '█',
        a if a >= 0.8 => '▓',
        a if a > 0.0 => '▒',
        _ => '·',
    }
}

/// Colour for a cell's *condition*, orthogonal to the density `shade` draws.
///
/// The glyph says how big the cell is; the colour says how it is doing. Before
/// this, both channels carried population, so a cell the classifier demotes on
/// every query was indistinguishable from a healthy one of the same size.
///
/// An uncovered cell (`activation == 0.0`) is dark grey: no ontology reaches it,
/// so there is nothing to be in good or bad condition. That is absence of
/// evidence, and must not render as the bad end of the ramp.
fn condition_color(condition: f32, activation: f32) -> Color {
    if activation <= 0.0 {
        return Color::DarkGray;
    }
    match condition {
        c if c >= 0.75 => Color::Green,
        c if c >= 0.5 => Color::Cyan,
        c if c >= 0.25 => Color::Yellow,
        _ => Color::Red,
    }
}

/// Load the semiotic grid once. It describes the loaded ontologies rather than
/// tenant state, so it cannot change while the engine runs.
/// Move the condition slider and redraw the map with the new lens.
///
/// Clears the cached grid first: `wiki_load_grid` is a load-once, and the
/// condition values are computed server-side per lens, so a stale grid would
/// leave the colours reporting the previous position.
fn wiki_nudge_confidence(app: &mut App, delta: f32) {
    app.wiki_confidence = (app.wiki_confidence + delta).clamp(0.0, 1.0);
    app.wiki_grid.clear();
    wiki_load_grid(app);
    app.status_msg = format!(
        "evidence needed: {:.2} ({})",
        app.wiki_confidence,
        if app.wiki_confidence <= 0.02 {
            "responsive — every cell reacts at full strength"
        } else if app.wiki_confidence >= 0.98 {
            "trusting — thin cells hold near neutral"
        } else {
            "balanced"
        }
    );
}

fn wiki_load_grid(app: &mut App) {
    if !app.wiki_grid.is_empty() {
        return;
    }
    let path = format!(
        "/api/v1/semiotic/grid?confidence={:.2}",
        app.wiki_confidence
    );
    match wiki_fetch(app, &path, None) {
        Ok(v) => {
            let cells = v
                .get("cells")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            app.wiki_grid = cells
                .iter()
                .map(|c| GridCell {
                    domain: c
                        .get("domain")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    mode: c
                        .get("mode")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    activation: c.get("activation").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                    entries: c
                        .get("entries")
                        .and_then(|e| e.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                    condition: c.get("condition").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                    // Absent `standing` means an older server that does not
                    // report it; 1.0 (neutral) is the honest reading, not 0.0.
                    standing: c.get("standing").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32,
                    verdicts: c.get("verdicts").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                })
                .collect();
            // Axis order follows first appearance, so the map matches the
            // engine's own DOMAIN x MODE ordering rather than an alphabetical
            // one the operator would not recognise.
            app.wiki_domains = distinct(app.wiki_grid.iter().map(|c| c.domain.clone()));
            app.wiki_modes = distinct(app.wiki_grid.iter().map(|c| c.mode.clone()));
            app.status_msg = format!(
                "grid: {} x {} cells",
                app.wiki_domains.len(),
                app.wiki_modes.len()
            );
        }
        Err(e) => app.status_msg = format!("grid unavailable: {e}"),
    }
}

fn distinct(it: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in it {
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// The cell under the map cursor.
fn cell_at(app: &App, row: usize, col: usize) -> Option<&GridCell> {
    let domain = app.wiki_domains.get(row)?;
    let mode = app.wiki_modes.get(col)?;
    app.wiki_grid
        .iter()
        .find(|c| &c.domain == domain && &c.mode == mode)
}

/// Scope the results to the selected cell.
///
/// Nodes are not stored with a cell tag, so this cannot filter the existing
/// hits without re-classifying every one of them (an embed per row). Instead it
/// searches with the cell's own entry names, which is what defines that cell in
/// the shared vector space — the same region of meaning, reached honestly and
/// in one round-trip. The query is left visible so it is obvious what was run.
fn wiki_scope_to_cell(app: &mut App) {
    let Some(cell) = cell_at(app, app.wiki_cell_row, app.wiki_cell_col) else {
        app.status_msg = "no such cell".into();
        return;
    };
    // Empty cells are now rare — /semiotic/grid used to omit custom_domains
    // (where the 20 EXTRA_ONTOLOGIES live), reporting 27 of 70 cells populated
    // while `classify` was placing text in the other 43; it now builds from
    // all_domains() and every cell has entries. The fallback stays as a guard:
    // a trimmed ontology set can still empty a cell, and claiming "uncovered"
    // for a cell the classifier covers is the failure worth preventing.
    let (query, how) = if cell.entries.is_empty() {
        (format!("{} {}", cell.domain, cell.mode), "axis names")
    } else {
        (
            cell.entries
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            "cell entries",
        )
    };
    let note = format!("scoped to {}×{} by {how}", cell.domain, cell.mode);
    app.wiki_query = query;
    app.wiki_stack.clear();
    app.wiki_crumbs.clear();
    wiki_search(app);
    // Say which basis was used, so a thin result set is legible rather than
    // looking like the wiki is empty.
    app.status_msg = format!("{note} — {} hits", app.wiki_hits.len());
}

/// Walk into the selected node's holarchy edges — browse relations instead of
/// similarity. Similarity says "these mean alike"; an edge says "someone
/// asserted these are related", which is a different and usually stronger claim.
fn wiki_walk_into(app: &mut App) {
    let Some(hit) = app.wiki_hits.get(app.wiki_sel) else {
        return;
    };
    let (id, label) = (hit.id.clone(), hit.label.clone());
    if id.is_empty() {
        app.status_msg = "this row has no node id to walk from".into();
        return;
    }
    match wiki_fetch(
        app,
        &format!("/api/v1/holarchy/neighbors?id={}", urlencoding_minimal(&id)),
        None,
    ) {
        Ok(v) => {
            let hits: Vec<WikiHit> = v
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|n| WikiHit {
                            id: n
                                .get("id")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            label: n
                                .get("label")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            // An edge carries a weight, not a density; show it in
                            // the same column rather than inventing a fake score.
                            density: n.get("weight").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                            asserted: n.get("asserted").and_then(|x| x.as_f64()).map(|x| x as f32),
                        })
                        .collect()
                })
                .unwrap_or_default();
            if hits.is_empty() {
                // Don't push an empty frame — the user would have to press
                // Backspace to escape a view that shows nothing.
                app.status_msg = format!("no edges from \"{}\"", truncate(&label, 40));
                return;
            }
            app.wiki_stack.push(std::mem::take(&mut app.wiki_hits));
            app.wiki_crumbs.push(label);
            app.wiki_hits = hits;
            app.wiki_sel = 0;
            app.status_msg = format!("{} neighbour(s)", app.wiki_hits.len());
            wiki_load_preview(app);
        }
        Err(e) => app.status_msg = format!("walk failed: {e}"),
    }
}

/// Show what the dream loop is replaying, named.
///
/// Reuses the walk stack so Backspace returns to the results — the dream view
/// is another lens on the same list, not a separate mode to escape from.
fn wiki_dream(app: &mut App) {
    match wiki_fetch(app, "/api/v1/dream/replay", Some(serde_json::json!({}))) {
        Ok(v) => {
            let hits: Vec<WikiHit> = v
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|n| WikiHit {
                            id: n
                                .get("node_id")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            label: n
                                .get("label")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            density: n.get("density").and_then(|x| x.as_f64()).unwrap_or(0.0)
                                as f32,
                            asserted: n.get("asserted").and_then(|x| x.as_f64()).map(|x| x as f32),
                        })
                        .collect()
                })
                .unwrap_or_default();
            if hits.is_empty() {
                // Nothing to replay is good news, and the engine also skips
                // anything already in the dream archive — so say which it is
                // rather than showing a blank pane.
                app.status_msg =
                    "nothing to replay — no reported failures, or all already dreamt".into();
                return;
            }
            app.wiki_stack.push(std::mem::take(&mut app.wiki_hits));
            app.wiki_crumbs.push("dreams".to_string());
            app.wiki_hits = hits;
            app.wiki_sel = 0;
            app.status_msg = format!("{} node(s) being replayed", app.wiki_hits.len());
            wiki_load_preview(app);
        }
        Err(e) => app.status_msg = format!("dream replay failed: {e}"),
    }
}

/// Climb back out of a walk, one level per press.
fn wiki_walk_back(app: &mut App) {
    match app.wiki_stack.pop() {
        Some(prev) => {
            app.wiki_hits = prev;
            app.wiki_crumbs.pop();
            app.wiki_sel = 0;
            app.status_msg = "back".into();
            wiki_load_preview(app);
        }
        None => app.status_msg = "already at the search results".into(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Percent-encode the few characters that actually break a query string. Avoids
/// pulling a dependency in for one call site.
fn urlencoding_minimal(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => other
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    status: &Arc<Mutex<EngineStatus>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw(f, app, status))?;
        if !event::poll(Duration::from_millis(200))? {
            // Idle tick: re-rank if typing has paused. Doing it here (rather
            // than on each keypress) is what keeps one embed per pause instead
            // of one per letter.
            if app.wiki_typing
                && should_search(
                    app.wiki_dirty,
                    app.wiki_last_input.elapsed().as_millis(),
                    app.wiki_query.chars().count(),
                )
            {
                app.wiki_dirty = false;
                wiki_search(app);
            }
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != crossterm::event::KeyEventKind::Press {
            continue;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(());
        }
        // The magic textbox owns every key while it is open, for the same
        // reason the wiki box does: a verb must be typeable without the panel
        // reading its letters as commands.
        if app.cmd_open {
            match key.code {
                KeyCode::Esc => app.cmd_open = false,
                KeyCode::Enter => {
                    app.cmd_open = false;
                    run_command(app);
                }
                KeyCode::Backspace => {
                    app.cmd_input.pop();
                }
                KeyCode::Char(c) => app.cmd_input.push(c),
                _ => {}
            }
            continue;
        }
        // While typing a wiki query every printable key belongs to the box —
        // otherwise searching for "quality" would quit on the first letter.
        if app.wiki_typing {
            match key.code {
                KeyCode::Esc => {
                    app.wiki_typing = false;
                }
                KeyCode::Enter => {
                    app.wiki_typing = false;
                    app.wiki_dirty = false;
                    wiki_search(app);
                }
                KeyCode::Backspace => {
                    app.wiki_query.pop();
                    app.wiki_dirty = true;
                    app.wiki_last_input = std::time::Instant::now();
                }
                KeyCode::Char(c) => {
                    app.wiki_query.push(c);
                    app.wiki_dirty = true;
                    app.wiki_last_input = std::time::Instant::now();
                }
                _ => {}
            }
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Tab => app.tab = (app.tab + 1) % TABS.len(),
            KeyCode::BackTab => app.tab = (app.tab + TABS.len() - 1) % TABS.len(),
            KeyCode::Char(':') => {
                app.cmd_open = true;
                app.cmd_input.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => match app.tab {
                1 => app.machine_sel = (app.machine_sel + 1) % MACHINES.len(),
                2 => app.cfg_sel = (app.cfg_sel + 1) % CFG_FIELDS.len(),
                TAB_WIKI if app.wiki_map_focus => {
                    let n = app.wiki_domains.len().max(1);
                    app.wiki_cell_row = (app.wiki_cell_row + 1) % n;
                }
                TAB_WIKI if !app.wiki_hits.is_empty() => {
                    app.wiki_sel = (app.wiki_sel + 1) % app.wiki_hits.len();
                    wiki_load_preview(app);
                }
                TAB_STREAM => {
                    app.stream_sel = cycle(app.stream_children.len(), app.stream_sel, 1);
                }
                _ => {}
            },
            KeyCode::Up | KeyCode::Char('k') => match app.tab {
                1 => app.machine_sel = (app.machine_sel + MACHINES.len() - 1) % MACHINES.len(),
                2 => app.cfg_sel = (app.cfg_sel + CFG_FIELDS.len() - 1) % CFG_FIELDS.len(),
                TAB_WIKI if app.wiki_map_focus => {
                    let n = app.wiki_domains.len().max(1);
                    app.wiki_cell_row = (app.wiki_cell_row + n - 1) % n;
                }
                TAB_WIKI if !app.wiki_hits.is_empty() => {
                    app.wiki_sel = (app.wiki_sel + app.wiki_hits.len() - 1) % app.wiki_hits.len();
                    wiki_load_preview(app);
                }
                TAB_STREAM => {
                    app.stream_sel = cycle(app.stream_children.len(), app.stream_sel, -1);
                }
                _ => {}
            },
            // WIKI: '/' opens the query box, Enter re-runs the search.
            // Map focus: 'm' toggles it (and loads the grid on first use).
            // Arrows then move the cell cursor instead of the result list --
            // a focus toggle rather than new key bindings, so j/k keep meaning
            // exactly one thing.
            KeyCode::Char('m') if app.tab == TAB_WIKI => {
                wiki_load_grid(app);
                if !app.wiki_grid.is_empty() {
                    app.wiki_map_focus = !app.wiki_map_focus;
                }
            }
            KeyCode::Left if app.tab == TAB_WIKI && app.wiki_map_focus => {
                let n = app.wiki_modes.len().max(1);
                app.wiki_cell_col = (app.wiki_cell_col + n - 1) % n;
            }
            KeyCode::Right if app.tab == TAB_WIKI && app.wiki_map_focus => {
                let n = app.wiki_modes.len().max(1);
                app.wiki_cell_col = (app.wiki_cell_col + 1) % n;
            }
            // The condition slider, as keys: '[' toward responsive (every cell
            // reacts at full strength), ']' toward trusting (thin cells hold
            // near neutral until evidence accumulates). Same knob the studio
            // exposes as a range input.
            KeyCode::Char('[') if app.tab == TAB_WIKI && app.wiki_map_focus => {
                wiki_nudge_confidence(app, -0.1);
            }
            KeyCode::Char(']') if app.tab == TAB_WIKI && app.wiki_map_focus => {
                wiki_nudge_confidence(app, 0.1);
            }
            KeyCode::Char('/') if app.tab == TAB_WIKI => {
                app.wiki_typing = true;
                app.wiki_query.clear();
            }
            KeyCode::Enter if app.tab == TAB_WIKI && app.wiki_map_focus => {
                wiki_scope_to_cell(app);
                app.wiki_map_focus = false;
            }
            KeyCode::Enter if app.tab == TAB_WIKI => {
                wiki_search(app);
            }
            // Graph walk: 'g' descends into the selected node's edges,
            // Backspace climbs back out (vim-ish, as the §2 spec describes).
            KeyCode::Char('g') if app.tab == TAB_WIKI => wiki_walk_into(app),
            KeyCode::Char('d') if app.tab == TAB_WIKI => wiki_dream(app),
            KeyCode::Backspace if app.tab == TAB_WIKI => wiki_walk_back(app),
            KeyCode::Enter if app.tab == TAB_STREAM => stream_enter(app),
            KeyCode::Backspace if app.tab == TAB_STREAM => stream_back(app),
            KeyCode::Char('g') if app.tab == TAB_STREAM => {
                app.agent_run = None;
                app.stream_graph = !app.stream_graph;
            }
            KeyCode::Enter if app.tab == 1 => {
                app.machine_out = run_machine(MACHINES[app.machine_sel].0);
            }
            KeyCode::Char('+') | KeyCode::Char('=') if app.tab == 2 => {
                edit_field(&mut app.cfg, app.cfg_sel, true);
            }
            KeyCode::Char('-') if app.tab == 2 => edit_field(&mut app.cfg, app.cfg_sel, false),
            KeyCode::Char('s') if app.tab == 2 => {
                app.status_msg = match save_config(&app.cfg) {
                    Ok(()) => format!("saved {}", cfg_path(&app.cfg).display()),
                    Err(e) => format!("save failed: {e}"),
                };
            }
            // Status tab: engine control.
            KeyCode::Char('s') if app.tab == 0 => start_engine(app),
            KeyCode::Char('x') if app.tab == 0 => stop_engine(app),
            KeyCode::Char('r') if app.tab == 0 => reload_config(app),
            _ => {}
        }
    }
}

fn draw(f: &mut ratatui::Frame, app: &App, status: &Arc<Mutex<EngineStatus>>) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    // Tab bar
    let tabs = Tabs::new(TABS.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
        .select(app.tab)
        .block(
            Block::default()
                .title(" PHYSIS PRO 🌻 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, rows[0]);

    match app.tab {
        0 => draw_status(f, rows[1], app, status),
        1 => draw_machines(f, rows[1], app),
        2 => draw_config(f, rows[1], app),
        TAB_WIKI => draw_wiki(f, rows[1], app),
        TAB_STREAM => draw_stream(f, rows[1], app),
        _ => {}
    }

    let help = match app.tab {
        1 => " Tab switch · ↑/↓ select · Enter run · q quit",
        2 => " Tab switch · ↑/↓ field · +/- edit · s save · q quit",
        TAB_WIKI if app.wiki_typing => {
            " type — results re-rank when you pause · Enter search now · Esc cancel"
        }
        TAB_WIKI if app.wiki_map_focus => {
            " ←/→ mode · ↑/↓ domain · [ ] evidence · Enter scope to cell · m unfocus"
        }
        TAB_WIKI => " Tab · / search · ↑/↓ · g walk · d dreams · ⌫ back · m map · q quit",
        TAB_STREAM => " Tab · ↑/↓ · Enter open · ⌫ back · g graph · : command · q quit",
        _ => " Tab · s start · x stop · r reload · : command · q quit",
    };
    let foot = if app.cmd_open {
        Paragraph::new(Line::from(vec![
            Span::styled(
                ":",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(app.cmd_input.clone(), Style::default().fg(Color::Yellow)),
            Span::styled(
                "   Enter run · Esc cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(help, Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("   {}", app.status_msg),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
    };
    f.render_widget(foot, rows[2]);
}

/// A deterministic verb the magic textbox knows. Parsing is pure so the
/// routing table is testable without a terminal; an unrecognised verb is
/// reported, never guessed at.
#[derive(Debug, PartialEq)]
enum Command {
    Open(String),
    Find(String),
    View(usize),
    Graph,
    /// Invoke an agent on a prompt. Route is `model`, not deterministic.
    Agent(String),
    Help,
    Usage(&'static str),
    Empty,
    Unknown(String),
}

/// Route the textbox input to a verb. Deterministic operators only: a model or
/// agent call is a separate, later arm, so this table can never reach one by
/// accident.
fn parse_command(input: &str) -> Command {
    let mut parts = input.trim().splitn(2, char::is_whitespace);
    let verb = parts.next().unwrap_or("").to_lowercase();
    let arg = parts.next().unwrap_or("").trim().to_string();
    match verb.as_str() {
        "" => Command::Empty,
        "help" | "?" => Command::Help,
        "open" | "cd" | "o" => {
            if arg.is_empty() {
                Command::Usage("open <path>")
            } else {
                Command::Open(arg)
            }
        }
        "find" | "search" | "/" => {
            if arg.is_empty() {
                Command::Usage("find <query>")
            } else {
                Command::Find(arg)
            }
        }
        "view" | "tab" => match TABS.iter().position(|t| t.eq_ignore_ascii_case(&arg)) {
            Some(i) => Command::View(i),
            None => Command::Usage("view <status|machines|config|wiki|stream>"),
        },
        "graph" => Command::Graph,
        "agent" => {
            if arg.is_empty() {
                Command::Usage("agent <prompt>")
            } else {
                Command::Agent(arg)
            }
        }
        _ => Command::Unknown(verb),
    }
}

/// The action a parsed command performs, or `None` when it performs none
/// (help, usage, empty, unknown). Pure, so "nothing happened, record nothing"
/// is a testable rule rather than a property of the side-effecting arms.
fn command_action(cmd: &Command) -> Option<(&'static str, String)> {
    match cmd {
        Command::Open(path) => Some(("open", path.clone())),
        Command::Find(query) => Some(("find", query.clone())),
        Command::View(tab) => Some(("view", TABS[*tab].to_lowercase())),
        Command::Graph => Some(("graph", String::new())),
        Command::Agent(prompt) => Some(("agent", prompt.clone())),
        Command::Help | Command::Usage(_) | Command::Empty | Command::Unknown(_) => None,
    }
}

/// The structured observation an action appends. Pure: the record shape is
/// testable without touching a store. `route` is `deterministic` for an
/// operator over stored state and `model` for an agent; `object` is the
/// WorldObject identity the action landed on, when there is one.
fn action_record(
    route: &str,
    operator: &str,
    target: &str,
    object: Option<&str>,
    outcome: &str,
) -> (&'static str, String, serde_json::Value) {
    let mut payload = serde_json::json!({
        "actor": "human",
        "route": route,
        "operator": operator,
        "outcome": outcome,
    });
    if !target.is_empty() {
        payload["target"] = serde_json::json!(target);
    }
    if let Some(o) = object {
        payload["object"] = serde_json::json!(o);
    }
    let subject = match (object, target.is_empty()) {
        (Some(o), false) => format!("{operator} {target} -> {o}"),
        (Some(o), true) => format!("{operator} -> {o}"),
        (None, false) => format!("{operator} {target}"),
        (None, true) => operator.to_string(),
    };
    ("human", subject, payload)
}

/// Append one action to the workspace observation store. One call writes
/// exactly one record.
fn record_action(
    ws: &Workspace,
    route: &str,
    operator: &str,
    target: &str,
    object: Option<&str>,
    outcome: &str,
) -> anyhow::Result<()> {
    let (source, subject, payload) = action_record(route, operator, target, object, outcome);
    ws.observe(source, &subject, payload).map(|_| ())
}

/// Record through the panel's store; `None` means recorded, `Some(reason)` is
/// surfaced in the status line so a missing or failing store is visible rather
/// than a silent lie that history was written. The textbox still works either
/// way.
fn record(
    app: &App,
    route: &str,
    operator: &str,
    target: &str,
    object: Option<&str>,
    outcome: &str,
) -> Option<String> {
    let Some(ws) = app.obs.as_ref() else {
        return Some("no observation store".into());
    };
    record_action(ws, route, operator, target, object, outcome)
        .err()
        .map(|e| e.to_string())
}

/// Run one parsed command against the panel. Every action names its route in
/// the status line and appends exactly one observation through the existing
/// workspace store; commands that did nothing append nothing.
fn run_command(app: &mut App) {
    let cmd = parse_command(&app.cmd_input);
    app.cmd_input.clear();

    let Some((operator, target)) = command_action(&cmd) else {
        match cmd {
            Command::Help => {
                app.status_msg =
                    "open <path> · find <query> · view <status|machines|config|wiki|stream> · graph · agent <prompt> · help"
                        .into();
            }
            Command::Usage(usage) => app.status_msg = format!("usage: {usage}"),
            Command::Unknown(verb) => {
                app.status_msg = format!("no operator for '{verb}' — try: help");
            }
            Command::Empty => {}
            _ => {}
        }
        return;
    };

    // An agent run is asynchronous and model-routed: it records its own single
    // observation on completion (from the run thread), so it must not also go
    // through the deterministic record below.
    if let Command::Agent(prompt) = &cmd {
        let prompt = prompt.clone();
        app.status_msg.clear();
        app.tab = TAB_STREAM;
        app.stream_graph = false;
        start_agent_run(app, prompt.clone());
        app.status_msg = format!("route=model operator=agent started · {prompt}");
        return;
    }

    // Clear first so an arm can leave a detail (the hit count) that the route
    // line precedes rather than erases.
    app.status_msg.clear();
    let (outcome, object) = match cmd {
        Command::Open(path) => {
            let missing = !std::path::Path::new(&path).exists();
            let obj = WorldObject::from_path(&path);
            let identity = obj.identity.clone();
            app.stream_stack = vec![obj];
            stream_refresh(app);
            app.tab = TAB_STREAM;
            app.agent_run = None;
            (if missing { "failure" } else { "success" }, Some(identity))
        }
        Command::Find(query) => {
            app.wiki_query = query;
            app.tab = TAB_WIKI;
            app.agent_run = None;
            let ok = wiki_search(app);
            (if ok { "success" } else { "failure" }, None)
        }
        Command::View(tab) => {
            app.tab = tab;
            if tab == TAB_STREAM {
                app.stream_graph = false;
            }
            app.agent_run = None;
            (
                "success",
                app.stream_stack.last().map(|o| o.identity.clone()),
            )
        }
        Command::Graph => {
            app.tab = TAB_STREAM;
            app.stream_graph = true;
            app.agent_run = None;
            (
                "success",
                app.stream_stack.last().map(|o| o.identity.clone()),
            )
        }
        _ => ("success", None),
    };

    let route = format!("route=deterministic operator={operator} {outcome}");
    let detail = std::mem::take(&mut app.status_msg);
    let mut line = if detail.is_empty() {
        route
    } else {
        format!("{route} · {detail}")
    };
    if let Some(err) = record(
        app,
        "deterministic",
        operator,
        &target,
        object.as_deref(),
        outcome,
    ) {
        line = format!("{line} · record failed: {err}");
    }
    app.status_msg = line;
}

/// Parse one SSE line from `POST /api/v1/agent/run` into an event view. Pure
/// and tolerant: keep-alive comments and any non-`data:` line are ignored, and
/// a malformed payload is dropped rather than aborting the stream.
fn parse_agent_event(line: &str) -> Option<AgentEventView> {
    let data = line.strip_prefix("data:")?.trim();
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    Some(AgentEventView {
        round: v.get("round").and_then(|r| r.as_u64()).unwrap_or(0) as u32,
        kind: v.get("kind")?.as_str()?.to_string(),
        text: v
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

/// Consume the run's SSE body, pushing each event into `run` as it arrives.
/// Blocking by design: it runs on its own thread, so a long run never freezes
/// the panel. `Err` means the stream could not be opened.
fn stream_agent_run(
    url: &str,
    token: Option<&str>,
    user: &str,
    prompt: &str,
    run: &Arc<Mutex<AgentRun>>,
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({"prompt": prompt, "mode": "", "tools": []});
    let mut rb = client.post(format!("{url}/api/v1/agent/run")).json(&body);
    if let Some(t) = token {
        rb = rb.bearer_auth(t);
    }
    rb = rb.header("X-Physis-User", user);
    let resp = rb
        .send()
        .map_err(|e| format!("agent request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    for line in BufReader::new(resp).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let Some(ev) = parse_agent_event(&line) else {
            continue;
        };
        push_event(run, ev);
    }
    Ok(())
}

/// Start an agent run on its own thread and point STREAM at it. The run records
/// exactly one observation on completion — the result, route `model` — through
/// the same workspace store as every other action.
fn start_agent_run(app: &mut App, prompt: String) {
    let run = Arc::new(Mutex::new(AgentRun::new(&prompt)));
    app.agent_run = Some(run.clone());
    let url = app.url.clone();
    let token = app.token.clone();
    let user = app.user.clone();
    let store = app.obs.as_ref().map(|w| (w.root.clone(), w.state.clone()));
    std::thread::spawn(move || {
        let result = stream_agent_run(&url, token.as_deref(), &user, &prompt, &run);
        let (outcome, detail) = match &result {
            Ok(()) => ("success", "agent run completed".to_string()),
            Err(e) => ("failure", e.clone()),
        };
        if let Ok(mut r) = run.lock() {
            r.status = if result.is_ok() {
                "done".into()
            } else {
                format!("failed: {detail}")
            };
        }
        if let Some((root, state)) = store {
            if let Ok(ws) = Workspace::open(&root, &state) {
                let id = run.lock().map(|r| r.id.clone()).unwrap_or_default();
                let _ = record_action(&ws, "model", "agent", &prompt, Some(&id), outcome);
            }
        }
    });
}

/// Move a selection index by `delta`, wrapping around. Kept pure so the
/// wrap-around rule is testable without a terminal; an empty list has no
/// selection to move.
fn cycle(len: usize, cur: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (cur as isize + delta).rem_euclid(len as isize) as usize
}

/// Refresh the cached children of the current object. Called on mount and
/// after every descent/ascent, never per frame — walking a directory reads the
/// filesystem and the render loop runs several times a second.
fn stream_refresh(app: &mut App) {
    app.stream_children = app
        .stream_stack
        .last()
        .map(WorldObject::children)
        .unwrap_or_default();
    app.stream_sel = 0;
}

/// Descend into the selected child. A directory's children are its entries; a
/// file's are its structural symbols.
fn stream_enter(app: &mut App) {
    app.agent_run = None;
    if let Some(child) = app.stream_children.get(app.stream_sel).cloned() {
        app.stream_stack.push(child);
        stream_refresh(app);
    }
}

/// Climb back toward the root. The root is never popped, so the tab always has
/// something to render.
fn stream_back(app: &mut App) {
    app.agent_run = None;
    if app.stream_stack.len() > 1 {
        app.stream_stack.pop();
        stream_refresh(app);
    }
}

/// The current object's typed edges as an ASCII projection. Deterministic and
/// read-only: the same relations the STREAM list shows, grouped by kind, so
/// the two views cannot disagree. Bounded so a 100-child directory stays
/// readable.
fn graph_lines(obj: &WorldObject, max_per_kind: usize) -> Vec<String> {
    const KINDS: [RelationKind; 10] = [
        RelationKind::Contains,
        RelationKind::DependsOn,
        RelationKind::ProducedBy,
        RelationKind::SimilarTo,
        RelationKind::Contradicts,
        RelationKind::Calls,
        RelationKind::Controls,
        RelationKind::DerivedFrom,
        RelationKind::ModifiedBy,
        RelationKind::Requires,
    ];
    let mut lines = vec![format!("graph  {}", obj.identity)];
    let mut edges = 0;
    for kind in KINDS {
        let targets: Vec<&str> = obj.relations_of(kind).map(|r| r.target.as_str()).collect();
        if targets.is_empty() {
            continue;
        }
        edges += targets.len();
        lines.push(format!("├─ {:?} ({})", kind, targets.len()));
        for t in targets.iter().take(max_per_kind) {
            lines.push(format!("│    {t}"));
        }
        if targets.len() > max_per_kind {
            lines.push(format!("│    … and {} more", targets.len() - max_per_kind));
        }
    }
    if edges == 0 {
        lines.push("(no typed edges — the object stands alone)".into());
    }
    if let Some(parent) = &obj.containment {
        lines.push(format!("└─ inside  {parent}"));
    }
    lines
}

/// The STREAM view of a live agent run: the run projected as a `WorldObject`
/// of kind `Agent`, beside the trace of events it has emitted. Same surface as
/// any other object; the events are runtime state, not part of the view type.
fn draw_agent(f: &mut ratatui::Frame, area: ratatui::layout::Rect, run: &AgentRun) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    let obj = WorldObject::agent(
        format!("agent:{}", run.id),
        &run.provider,
        &run.model,
        &run.status,
    );
    f.render_widget(
        Paragraph::new(obj.identity.clone()).block(
            Block::default()
                .title(format!(" STREAM · agent · {} ", truncate(&run.prompt, 40)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        ),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    let body: Vec<Line> = obj.render().into_iter().map(Line::from).collect();
    f.render_widget(
        Paragraph::new(body)
            .block(Block::default().title(" agent ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    let items: Vec<ListItem> = run
        .events
        .iter()
        .rev()
        .take(40)
        .rev()
        .map(|e| {
            ListItem::new(Line::from(format!(
                "[{}] {} {}",
                e.round,
                e.kind,
                truncate(&e.text, 60)
            )))
        })
        .collect();
    f.render_widget(
        List::new(items).block(
            Block::default()
                .title(format!(" trace ({}) ", run.events.len()))
                .borders(Borders::ALL),
        ),
        cols[1],
    );
}

/// STREAM tab — the world-object navigator.
///
/// Breadcrumb of the path taken, the current object rendered provenance-first
/// (a navigational layer that cannot say why it said something is an oracle),
/// then the objects one step inside it. Read-only on purpose: this slice makes
/// the object model navigable, not yet editable.
fn draw_stream(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    // A live agent run owns the view until the operator navigates away; the
    // object it projects is built from the run state here, so WorldObject never
    // holds a reference to the provider runtime.
    if let Some(run) = app.agent_run.as_ref() {
        if let Ok(run) = run.lock() {
            draw_agent(f, area, &run);
            return;
        }
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    let crumb = app
        .stream_stack
        .iter()
        .map(|o| truncate(&o.identity, 28))
        .collect::<Vec<_>>()
        .join(" → ");
    let title = if app.stream_graph {
        " STREAM · graph · path "
    } else {
        " STREAM · path "
    };
    f.render_widget(
        Paragraph::new(crumb).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        ),
        rows[0],
    );

    if app.stream_graph {
        let body: Vec<Line> = app
            .stream_stack
            .last()
            .map(|o| graph_lines(o, 12).into_iter().map(Line::from).collect())
            .unwrap_or_default();
        f.render_widget(
            Paragraph::new(body)
                .block(Block::default().title(" graph ").borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
            rows[1],
        );
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[1]);

    let body: Vec<Line> = app
        .stream_stack
        .last()
        .map(|o| o.render().into_iter().map(Line::from).collect())
        .unwrap_or_default();
    f.render_widget(
        Paragraph::new(body)
            .block(Block::default().title(" object ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    let items: Vec<ListItem> = app
        .stream_children
        .iter()
        .map(|c| {
            ListItem::new(Line::from(
                format!("{:?}  {}", c.kind, c.content).to_lowercase(),
            ))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.stream_sel.min(items.len() - 1)));
    }
    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" inside ({}) ", app.stream_children.len()))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, cols[1], &mut state);
}

/// WIKI tab — search box on top, cosine-ranked hits left, preview right.
///
/// The colour dot encodes the *asserted* verdict, not the derived density:
/// green worked, red blocked, yellow inert, dim grey nobody has judged. Density
/// is shown as a number because "typical" and "worked" are different questions
/// and a single colour cannot honestly carry both.
fn draw_wiki(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    // Search box, results/preview, then the map. The map gets a fixed height
    // (one line per domain plus header and borders) so the result list keeps
    // every remaining row.
    let map_height = if app.wiki_domains.is_empty() {
        3
    } else {
        app.wiki_domains.len() as u16 + 4
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(map_height),
        ])
        .split(area);

    let query_style = if app.wiki_typing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let shown = if app.wiki_query.is_empty() {
        "(press / and describe what it was about)"
    } else {
        &app.wiki_query
    };
    let search = Paragraph::new(Line::from(Span::styled(shown.to_string(), query_style))).block(
        Block::default()
            .title(" search by meaning ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if app.wiki_typing {
                Color::Yellow
            } else {
                Color::DarkGray
            })),
    );
    f.render_widget(search, rows[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    let items: Vec<ListItem> = app
        .wiki_hits
        .iter()
        .map(|h| {
            let (dot, colour) = match h.asserted {
                Some(a) if a > 0.2 => ("●", Color::Green),
                Some(a) if a < -0.2 => ("●", Color::Red),
                Some(_) => ("●", Color::Yellow),
                None => ("○", Color::DarkGray),
            };
            let mut label = h.label.replace('\n', " ");
            if label.chars().count() > 46 {
                label = label.chars().take(45).collect::<String>() + "…";
            }
            ListItem::new(Line::from(vec![
                Span::styled(format!("{dot} "), Style::default().fg(colour)),
                Span::styled(
                    format!("{:.2} ", h.density),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(label),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !app.wiki_hits.is_empty() {
        state.select(Some(app.wiki_sel));
    }
    let list = List::new(items)
        .block(
            Block::default()
                .title(if app.wiki_crumbs.is_empty() {
                    format!(
                        " {} hits · tenant {} · ●worked ●blocked ●inert ○unjudged ",
                        app.wiki_hits.len(),
                        app.user
                    )
                } else {
                    // Show the walk path so it is obvious these are edges, not
                    // search results, and how deep the walk has gone.
                    format!(
                        " edges of {} · {} neighbour(s) ",
                        app.wiki_crumbs
                            .iter()
                            .map(|c| truncate(c, 18))
                            .collect::<Vec<_>>()
                            .join(" → "),
                        app.wiki_hits.len()
                    )
                })
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, cols[0], &mut state);

    let preview = Paragraph::new(app.wiki_preview.clone())
        .block(Block::default().title(" node ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, cols[1]);

    draw_heatmap(f, rows[2], app);
}

/// The 5x14 semiotic grid, shaded by how densely the ontologies populate each
/// DOMAIN x MODE cell, with a cursor when the map has focus.
fn draw_heatmap(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    if app.wiki_grid.is_empty() {
        let hint = Paragraph::new(Line::from(Span::styled(
            " press m to load the semiotic map ",
            Style::default().fg(Color::DarkGray),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(hint, area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    // Header: first letter of each mode, so 14 columns fit a narrow terminal.
    let mut header = vec![Span::raw(format!("{:<10}", ""))];
    for (i, m) in app.wiki_modes.iter().enumerate() {
        let initial = m.chars().next().unwrap_or('?');
        let style = if app.wiki_map_focus && i == app.wiki_cell_col {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        header.push(Span::styled(format!("{initial} "), style));
    }
    lines.push(Line::from(header));

    for (r, domain) in app.wiki_domains.iter().enumerate() {
        let row_focused = app.wiki_map_focus && r == app.wiki_cell_row;
        let mut spans = vec![Span::styled(
            format!("{:<10}", truncate(domain, 9)),
            if row_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            },
        )];
        for c in 0..app.wiki_modes.len() {
            let cell = cell_at(app, r, c);
            let act = cell.map(|x| x.activation).unwrap_or(0.0);
            let cond = cell.map(|x| x.condition).unwrap_or(0.0);
            let ch = shade(act);
            let on_cursor = app.wiki_map_focus && r == app.wiki_cell_row && c == app.wiki_cell_col;
            let style = if on_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(condition_color(cond, act))
            };
            spans.push(Span::styled(format!("{ch} "), style));
        }
        lines.push(Line::from(spans));
    }

    let title = match cell_at(app, app.wiki_cell_row, app.wiki_cell_col) {
        // The colour is a claim about the cell, so the title has to say what
        // the claim rests on — standing, and how many verdicts back it.
        Some(c) if app.wiki_map_focus => format!(
            " {}×{} · {} entries · standing ×{:.2} · {} · Enter scopes the search ",
            c.domain,
            c.mode,
            c.entries.len(),
            c.standing,
            match c.verdicts {
                0 => "no verdicts yet".to_string(),
                1 => "1 verdict".to_string(),
                n => format!("{n} verdicts"),
            },
        ),
        _ => " semiotic map · m to focus ".to_string(),
    };
    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if app.wiki_map_focus {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            }),
    );
    f.render_widget(widget, area);
}

fn draw_status(
    f: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    app: &App,
    status: &Arc<Mutex<EngineStatus>>,
) {
    let st = status.lock().unwrap();
    let dot = if st.online {
        Span::styled("● ONLINE", Style::default().fg(Color::Green))
    } else {
        Span::styled("● OFFLINE", Style::default().fg(Color::Red))
    };
    let mut lines = vec![
        Line::from(vec![Span::raw("engine   "), dot]),
        Line::from(format!("url      {}", app.url)),
        Line::from(format!(
            "embedder {}",
            if st.embedder.is_empty() {
                "—"
            } else {
                &st.embedder
            }
        )),
        Line::from(""),
    ];
    if !st.detail.is_empty() {
        lines.push(Line::from(Span::styled(
            "/api/v1/embedder:",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(st.detail.clone()));
    } else if !st.online {
        lines.push(Line::from(Span::styled(
            "start the engine: physis-pro-web",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let p = Paragraph::new(lines)
        .block(Block::default().title(" STATUS ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn draw_machines(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    let items: Vec<ListItem> = MACHINES
        .iter()
        .map(|(n, d)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{n:<10}"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*d, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.machine_sel));
    let list = List::new(items)
        .block(Block::default().title(" MACHINES ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, cols[0], &mut state);

    let out = Paragraph::new(app.machine_out.as_str())
        .block(
            Block::default()
                .title(format!(" {} OUTPUT ", MACHINES[app.machine_sel].0))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(out, cols[1]);
}

fn draw_config(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("file: {}", cfg_path(&app.cfg).display()),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];
    for (i, name) in CFG_FIELDS.iter().enumerate() {
        let sel = i == app.cfg_sel;
        lines.push(Line::from(vec![
            Span::styled(
                if sel { "▶ " } else { "  " },
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{name:<30}"),
                Style::default()
                    .fg(if sel { Color::White } else { Color::Gray })
                    .add_modifier(if sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(cfg_value(&app.cfg, i), Style::default().fg(Color::Yellow)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "+/- to change · s to save",
        Style::default().fg(Color::DarkGray),
    )));
    let p = Paragraph::new(lines).block(
        Block::default()
            .title(" CONFIG (~/.physis/config.json) ")
            .borders(Borders::ALL),
    );
    f.render_widget(p, area);
}

// ── In-process machine runs (the MACHINES tab "launches" these) ──────────────

fn run_machine(name: &str) -> String {
    match name {
        "chronos" => chronos_demo(),
        "techne" => techne_demo(),
        "soma" => soma_demo(),
        "elenchus" => elenchus_demo(),
        "lex" => lex_demo(),
        "harmony" => harmony_demo(),
        "polis" => polis_demo(),
        _ => format!("unknown machine: {name}"),
    }
}

fn chronos_demo() -> String {
    let now = Utc::now();
    let ev = |label: &str, days_ago: i64| chronos::TimedEvent {
        label: label.into(),
        at: now - ChronoDuration::days(days_ago),
    };
    let mut events: Vec<chronos::TimedEvent> = (0..7).map(|i| ev("gym", 2 + i * 2)).collect();
    for d in [32, 25, 18, 11] {
        events.push(ev("standup", d));
    }
    for d in [21, 9, 2] {
        events.push(ev("call mum", d));
    }
    let rhythms = chronos::detect_rhythms(&events);
    let mut s = format!("{} events → {} rhythms:\n", events.len(), rhythms.len());
    for r in &rhythms {
        s += &format!(
            "  {:<10} every {:>4.1}d  conf {:.2}  next {}\n",
            r.label,
            r.period_days,
            r.confidence,
            r.next_due.format("%Y-%m-%d")
        );
    }
    let slip = chronos::slipping(&rhythms, now, 0.5);
    if !slip.is_empty() {
        s += &format!(
            "slipping: {}",
            slip.iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    s
}

fn techne_demo() -> String {
    let f = |path: &str, lines: &[&str]| techne::SourceFile {
        path: path.into(),
        lines: lines.iter().map(|s| s.to_string()).collect(),
    };
    let files = vec![
        f("src/embed/mod.rs", &[]),
        f("src/classify.rs", &["use crate::embed::VectorEmbed;"]),
        f(
            "src/core.rs",
            &["use crate::trie::Trie;", "use crate::embed::VectorEmbed;"],
        ),
        f("src/trie.rs", &[]),
        f("src/bin/web_server.rs", &["use crate::core::PhysisCore;"]),
        f("src/spanner.rs", &["use crate::core::PhysisCore;"]),
    ];
    let mut s = String::from("capabilities:\n");
    for c in techne::classify_capabilities(&files) {
        s += &format!("  {:<14} {}\n", c.name, c.files.join(", "));
    }
    let edges = techne::dependency_edges(&files);
    s += &format!(
        "impact of `embed`: {}",
        techne::impact_of(&edges, "embed").join(", ")
    );
    s
}

fn soma_demo() -> String {
    let s = |day: u32, load: f32, recovery: f32| soma::Session {
        day,
        activity: "lift".into(),
        load,
        recovery,
    };
    let sessions = vec![
        s(1, 0.5, 0.9),
        s(2, 0.7, 0.8),
        s(3, 0.8, 0.6),
        s(4, 0.8, 0.5),
        s(5, 0.9, 0.4),
        s(6, 0.9, 0.35),
    ];
    let mut out = String::new();
    for x in &sessions {
        out += &format!(
            "  day {}  load {:.1} rec {:.1}  {:?}\n",
            x.day,
            x.load,
            x.recovery,
            soma::classify(x)
        );
    }
    let d = soma::drift(&sessions, 6);
    out += &format!(
        "drift {:+.2} → {}",
        d,
        if d > 0.5 {
            "overreaching risk"
        } else {
            "stable"
        }
    );
    out
}

fn elenchus_demo() -> String {
    let st =
        |id: &str, kind: elenchus::Kind, text: &str, target: Option<&str>| elenchus::Statement {
            id: id.into(),
            kind,
            text: text.into(),
            target: target.map(|s| s.into()),
        };
    let arg = vec![
        st(
            "c1",
            elenchus::Kind::Claim,
            "Remote work raises output",
            None,
        ),
        st(
            "w1",
            elenchus::Kind::Warrant,
            "fewer commute hours",
            Some("c1"),
        ),
        st(
            "c2",
            elenchus::Kind::Claim,
            "Open offices boost collaboration",
            None,
        ),
        st(
            "r2",
            elenchus::Kind::Rebuttal,
            "studies show -70% face time",
            Some("c2"),
        ),
        st(
            "c3",
            elenchus::Kind::Claim,
            "Four-day weeks always better",
            None,
        ),
    ];
    let mut s = String::new();
    for (id, standing) in elenchus::evaluate(&arg) {
        let t = arg.iter().find(|x| x.id == id).unwrap();
        s += &format!("  {:<3} {:<12?} {}\n", id, standing, t.text);
    }
    s += &format!(
        "press: {:?}\nrecalibrate: {:?}",
        elenchus::unsupported_claims(&arg),
        elenchus::contested_claims(&arg)
    );
    s
}

fn lex_demo() -> String {
    let c = |id: &str, kind: lex::Kind, requires: &[&str]| lex::Clause {
        id: id.into(),
        kind,
        text: id.into(),
        requires: requires.iter().map(|s| s.to_string()).collect(),
    };
    let contract = vec![
        c("deliver", lex::Kind::Obligation, &[]),
        c("pay", lex::Kind::Obligation, &["delivered"]),
        c("delivered", lex::Kind::Condition, &[]),
        c("refund", lex::Kind::Obligation, &["returned"]),
        c("warranty", lex::Kind::Right, &["registered"]),
    ];
    format!(
        "always due: {:?}\nbreach if `delivered` fails: {:?}\nunhandled: {:?}",
        lex::unconditional_obligations(&contract),
        lex::breach_paths(&contract, &["delivered".into()]),
        lex::dangling_conditions(&contract),
    )
}

fn harmony_demo() -> String {
    let melody = vec![60, 62, 64, 63, 65, 67, 69, 68, 67];
    let mut s = format!("intervals: {:?}\nmotifs:\n", harmony::intervals(&melody));
    for (seq, count) in harmony::motifs(&melody, 3) {
        s += &format!("  {:?} x{}\n", seq, count);
    }
    s += &format!("transpose +5: {:?}", harmony::transpose(&[60, 62, 64], 5));
    s
}

fn polis_demo() -> String {
    let i = |from: &str, to: &str, w: f32| polis::Interaction {
        from: from.into(),
        to: to.into(),
        weight: w,
    };
    let interactions = vec![
        i("you", "ana", 9.0),
        i("ana", "you", 8.0),
        i("you", "ben", 7.0),
        i("ben", "you", 1.0),
        i("ana", "ben", 4.0),
        i("ben", "ana", 4.0),
    ];
    let ties = polis::ties(&interactions);
    let mut s = String::from("ties:\n");
    for t in &ties {
        s += &format!(
            "  {:<4}-{:<4} strength {:>4.1} recip {:.2}\n",
            t.a, t.b, t.strength, t.reciprocity
        );
    }
    let fade = polis::fading(&ties, 0.5);
    if !fade.is_empty() {
        s += &format!(
            "fading: {}",
            fade.iter()
                .map(|t| format!("{}-{}", t.a, t.b))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use physis_pro::world_object::ObjectKind;

    #[test]
    fn graph_lines_group_typed_edges_and_bound_the_list() {
        let obj = WorldObject::new("project:aion", ObjectKind::Project, "aion")
            .relate(RelationKind::Contains, "module:ui")
            .relate(RelationKind::Contains, "module:deck")
            .relate(RelationKind::DependsOn, "service:physis")
            .contained_in("fs:/home/gio/dev");
        let text = graph_lines(&obj, 10).join("\n");
        assert!(text.contains("graph  project:aion"), "{text}");
        assert!(text.contains("Contains (2)"), "{text}");
        assert!(text.contains("DependsOn (1)"), "{text}");
        assert!(text.contains("inside  fs:/home/gio/dev"), "{text}");

        let bounded = graph_lines(&obj, 1).join("\n");
        assert!(bounded.contains("… and 1 more"), "{bounded}");

        let lone = graph_lines(
            &WorldObject::new("x:1", ObjectKind::Other("x".into()), "x"),
            5,
        )
        .join("\n");
        assert!(lone.contains("stands alone"), "{lone}");
    }

    #[test]
    fn parse_hits_keeps_unjudged_distinct_from_inert() {
        // The engine omits `asserted` entirely for a node nobody has judged.
        // That must stay None: collapsing it to 0.0 would render an unjudged
        // node as "inert", i.e. an opinion the engine never expressed.
        let v = serde_json::json!({
            "results": [
                { "id": "a", "label": "shipped the migration", "score": 0.9, "asserted": 1.0 },
                { "id": "b", "label": "index rebuild",         "score": 0.5, "asserted": 0.0 },
                { "id": "c", "label": "unjudged note",         "score": 0.4 }
            ],
            "count": 3
        });
        let hits = parse_hits(&v);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].asserted, Some(1.0));
        assert_eq!(
            hits[1].asserted,
            Some(0.0),
            "an explicit 0.0 verdict is a judgement"
        );
        assert_eq!(
            hits[2].asserted, None,
            "a missing verdict must not become 0.0"
        );
        assert!((hits[0].density - 0.9).abs() < 1e-6);
    }

    #[test]
    fn parse_hits_drops_unlabeled_and_tolerates_junk() {
        // Unlabeled nodes cannot be browsed by content, and a malformed payload
        // must yield an empty list rather than panic the UI loop.
        let v = serde_json::json!({ "results": [ { "id": "a", "score": 0.9 } ] });
        assert!(
            parse_hits(&v).is_empty(),
            "unlabeled node has no readable handle"
        );
        assert!(parse_hits(&serde_json::json!({})).is_empty());
        assert!(parse_hits(&serde_json::json!({ "results": "not-an-array" })).is_empty());
    }

    #[test]
    fn parse_hits_captures_the_node_id_needed_to_walk() {
        // Without the id the graph walk has no handle to follow, so a row that
        // arrives without one must not silently look walkable.
        let v = serde_json::json!({ "results": [
            { "id": "abc-123", "label": "a node", "score": 0.5 },
            { "label": "no id here", "score": 0.4 }
        ]});
        let hits = parse_hits(&v);
        assert_eq!(hits[0].id, "abc-123");
        assert!(
            hits[1].id.is_empty(),
            "missing id stays empty rather than faking one"
        );
    }

    #[test]
    fn shade_maps_activation_to_density() {
        // An empty cell must be visually distinct from a sparse one: "no
        // ontology covers this" is the finding `discover` exists to surface,
        // and rendering it like a weak cell would hide it.
        assert_eq!(shade(0.0), '·');
        assert_eq!(shade(0.3), '▒');
        assert_eq!(shade(1.0), '▓');
        assert_eq!(shade(2.4), '█');
    }

    #[test]
    fn distinct_preserves_engine_axis_order() {
        // Rows/columns must follow the engine's DOMAIN x MODE order, not an
        // alphabetical one the operator would not recognise.
        let got = distinct(
            ["HEAL", "CONSTRUCT", "HEAL", "FABRICATE", "", "CONSTRUCT"]
                .into_iter()
                .map(String::from),
        );
        assert_eq!(got, vec!["HEAL", "CONSTRUCT", "FABRICATE"]);
    }

    #[test]
    fn truncate_is_char_safe_on_multibyte() {
        // Labels are user text; slicing bytes would panic on a multi-byte
        // boundary and take the whole UI down with it.
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
        assert_eq!(truncate("caffè corretto", 6), "caffè…");
    }

    #[test]
    fn live_rerank_waits_for_a_pause_and_ignores_fragments() {
        // Fires only once typing has settled: every search costs an HTTP hop
        // plus an ONNX embed of the query, so per-keystroke would queue a
        // request behind every letter.
        assert!(should_search(true, 400, 10), "settled typing re-ranks");
        assert!(!should_search(true, 100, 10), "still typing — hold");
        assert!(
            !should_search(false, 999, 10),
            "nothing changed since last search"
        );
        // 1-2 chars match nearly everything; a round-trip to rank noise.
        assert!(
            !should_search(true, 999, 2),
            "fragment too short to mean anything"
        );
        assert!(should_search(true, 999, 3), "3 chars is the floor");
    }

    #[test]
    fn query_encoding_survives_a_real_sentence() {
        // Spaces become '+' (serde_urlencoded decodes that back to a space) and
        // anything that would break the query string is percent-encoded.
        assert_eq!(
            urlencoding_minimal("ate cake at midnight"),
            "ate+cake+at+midnight"
        );
        assert_eq!(urlencoding_minimal("a&b=c"), "a%26b%3Dc");
        assert_eq!(
            urlencoding_minimal("caffè"),
            "caff%C3%A8",
            "multi-byte utf8 encodes per byte"
        );
        assert_eq!(urlencoding_minimal("plain-text_1.0~"), "plain-text_1.0~");
    }

    #[test]
    fn magic_textbox_routes_deterministic_verbs_only() {
        assert_eq!(parse_command("open src"), Command::Open("src".into()));
        assert_eq!(
            parse_command("  OPEN  src/world_object.rs "),
            Command::Open("src/world_object.rs".into()),
            "verb is case-insensitive, path keeps its case"
        );
        assert_eq!(
            parse_command("find retrieval packing"),
            Command::Find("retrieval packing".into())
        );
        assert_eq!(parse_command("view stream"), Command::View(4));
        assert_eq!(parse_command("view WIKI"), Command::View(3));
        assert_eq!(parse_command("graph"), Command::Graph);
        assert_eq!(parse_command("help"), Command::Help);
        assert_eq!(parse_command("   "), Command::Empty);
        assert_eq!(parse_command("open"), Command::Usage("open <path>"));
        assert_eq!(
            parse_command("view nowhere"),
            Command::Usage("view <status|machines|config|wiki|stream>")
        );
        assert_eq!(
            parse_command("agent do the thing"),
            Command::Agent("do the thing".into()),
            "an agent prompt is its own verb, routed as model"
        );
        assert_eq!(parse_command("agent"), Command::Usage("agent <prompt>"));
        assert_eq!(parse_command("rm -rf /"), Command::Unknown("rm".into()));
    }

    #[test]
    fn agent_events_parse_from_sse_data_lines_only() {
        let ev = parse_agent_event(r#"data: {"round":2,"kind":"tool_call","text":"read src"}"#)
            .expect("data line parses");
        assert_eq!(
            ev,
            AgentEventView {
                round: 2,
                kind: "tool_call".into(),
                text: "read src".into(),
            }
        );
        assert_eq!(parse_agent_event(":keep-alive"), None, "comments ignored");
        assert_eq!(parse_agent_event("event: final"), None, "non-data ignored");
        assert_eq!(parse_agent_event("data: not json"), None, "junk dropped");
        assert_eq!(
            parse_agent_event(r#"data: {"kind":"final"}"#),
            Some(AgentEventView {
                round: 0,
                kind: "final".into(),
                text: String::new(),
            }),
            "missing text/round default rather than fail"
        );
    }

    #[test]
    fn the_agent_trace_is_bounded_and_done_sets_the_run_id() {
        let run = Arc::new(Mutex::new(AgentRun::new("go")));
        for i in 0..(MAX_AGENT_EVENTS + 10) {
            push_event(
                &run,
                AgentEventView {
                    round: i as u32,
                    kind: "plan".into(),
                    text: format!("e{i}"),
                },
            );
        }
        {
            let r = run.lock().unwrap();
            assert_eq!(r.events.len(), MAX_AGENT_EVENTS, "trace is capped");
            assert_eq!(r.events.first().unwrap().text, "e10", "oldest dropped");
            assert_eq!(
                r.events.last().unwrap().text,
                format!("e{}", MAX_AGENT_EVENTS + 9)
            );
        }
        push_event(
            &run,
            AgentEventView {
                round: 0,
                kind: "done".into(),
                text: "run-42".into(),
            },
        );
        assert_eq!(run.lock().unwrap().id, "run-42");
    }

    #[test]
    fn stream_agent_run_consumes_sse_into_the_run() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf); // the request; ignored
            let body = "data: {\"round\":0,\"kind\":\"plan\",\"text\":\"thinking\"}\n\n\
                        data: {\"round\":1,\"kind\":\"final\",\"text\":\"answer\"}\n\n\
                        data: {\"round\":0,\"kind\":\"done\",\"text\":\"run-42\"}\n\n";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\
                 Content-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = sock.write_all(resp.as_bytes());
        });

        let run = Arc::new(Mutex::new(AgentRun::new("hi")));
        let result = stream_agent_run(&format!("http://{addr}"), None, "tester", "hi", &run);
        server.join().unwrap();
        assert!(result.is_ok(), "{result:?}");
        let r = run.lock().unwrap();
        assert_eq!(r.events.len(), 3);
        assert_eq!(r.events[0].kind, "plan");
        assert_eq!(r.events[1].text, "answer");
        assert_eq!(r.id, "run-42", "the done event names the run");
    }

    #[test]
    fn agent_is_an_action_but_help_usage_empty_unknown_are_not() {
        assert_eq!(
            command_action(&Command::Agent("go".into())),
            Some(("agent", "go".into()))
        );
        for cmd in [
            Command::Help,
            Command::Usage("agent <prompt>"),
            Command::Empty,
            Command::Unknown("nope".into()),
        ] {
            assert_eq!(command_action(&cmd), None, "{cmd:?} performs no action");
        }
    }

    #[test]
    fn a_successful_action_appends_exactly_one_observation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().join(".physis/system");
        let ws = Workspace::open(dir.path(), &state).expect("workspace opens");

        record_action(
            &ws,
            "deterministic",
            "open",
            "src/world_object.rs",
            Some("fs:src/world_object.rs"),
            "success",
        )
        .expect("recorded");

        let records =
            physis_core::observe::read(&state.join("observations.jsonl")).expect("read log");
        assert_eq!(records.len(), 1, "one action, one record");
        assert_eq!(records[0].source, "human");
        let body: serde_json::Value =
            serde_json::from_str(&records[0].body).expect("structured body");
        assert_eq!(body["actor"], "human");
        assert_eq!(body["route"], "deterministic");
        assert_eq!(body["operator"], "open");
        assert_eq!(body["target"], "src/world_object.rs");
        assert_eq!(body["object"], "fs:src/world_object.rs");
        assert_eq!(body["outcome"], "success");

        // Recoverable through the existing history reader, not just the file.
        let history = ws.history(Some("open"), 10).expect("history reads");
        assert!(
            history["matched_in_window"].as_u64().unwrap_or(0) >= 1,
            "the action is visible to `physis system history`: {history}"
        );
    }

    #[test]
    fn failed_and_empty_commands_do_not_claim_success() {
        // Nothing happened: no action, so nothing may be recorded.
        for cmd in [
            Command::Empty,
            Command::Help,
            Command::Unknown("rm".into()),
            Command::Usage("open <path>"),
        ] {
            assert_eq!(command_action(&cmd), None, "{cmd:?} performs no action");
        }

        // A failed action records the failure, never success.
        let (_, _, payload) =
            action_record("deterministic", "open", "does-not-exist", None, "failure");
        assert_eq!(payload["outcome"], "failure");

        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().join(".physis/system");
        let ws = Workspace::open(dir.path(), &state).expect("workspace opens");
        record_action(
            &ws,
            "deterministic",
            "open",
            "does-not-exist",
            None,
            "failure",
        )
        .expect("recorded");
        let records =
            physis_core::observe::read(&state.join("observations.jsonl")).expect("read log");
        assert_eq!(records.len(), 1);
        let body: serde_json::Value = serde_json::from_str(&records[0].body).unwrap();
        assert_eq!(body["outcome"], "failure");
    }

    #[test]
    fn action_record_names_the_operator_target_and_object() {
        let (source, subject, payload) = action_record(
            "deterministic",
            "open",
            "src/x.rs",
            Some("fs:src/x.rs"),
            "success",
        );
        assert_eq!(source, "human");
        assert_eq!(subject, "open src/x.rs -> fs:src/x.rs");
        assert_eq!(payload["route"], "deterministic");
        assert_eq!(payload["object"], "fs:src/x.rs");

        let (_, subject, payload) =
            action_record("deterministic", "graph", "", Some("fs:src"), "success");
        assert_eq!(subject, "graph -> fs:src");
        assert!(
            payload.get("target").is_none(),
            "no target field when there is none"
        );

        let (_, subject, payload) = action_record("deterministic", "view", "wiki", None, "success");
        assert_eq!(subject, "view wiki");
        assert!(payload.get("object").is_none());

        // An agent action is model-routed, not deterministic.
        let (_, _, payload) = action_record("model", "agent", "fix the build", None, "success");
        assert_eq!(payload["route"], "model");
    }

    #[test]
    fn cycle_wraps_stream_selection_both_ways() {
        assert_eq!(cycle(3, 2, 1), 0, "past the end wraps to the first");
        assert_eq!(cycle(3, 0, -1), 2, "before the start wraps to the last");
        assert_eq!(cycle(3, 1, 1), 2, "inside the list moves by one");
        assert_eq!(cycle(0, 0, 1), 0, "an empty list has no selection to move");
        assert_eq!(cycle(1, 0, 1), 0, "a single child stays selected");
    }
}
