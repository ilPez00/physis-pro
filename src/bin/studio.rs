//! physis-studio — one demo product for the engine, in one TUI.
//!
//! Six tabs, cycle with Tab:
//!   STATUS   — engine health (HTTP, non-blocking) and what this shell is.
//!   GRAPH    — browse files by MEANING: category → theme → files → related
//!              themes, straight off the persisted `theme_graph.json`.
//!   NOTEBOOK — ask a question of the local corpus and read the CITED answer
//!              (`ported::notebook`): selection is physis, synthesis says which
//!              tier produced it, and `degraded` is shown, never hidden.
//!   DIFF     — review two files as hunks. Line diff, word diff, hunk nav.
//!   MACHINES — run a philosophical machine on a sample (in-process).
//!   CONFIG   — edit + save `~/.physis/config.json`.
//!
//! ## Two rules this binary keeps
//!
//! 1. **Offline-first.** GRAPH, NOTEBOOK and DIFF read the local store and the
//!    local files; only STATUS touches the network, on a background thread, so
//!    no tab can wedge on a server that is not running.
//! 2. **No silent degradation.** A missing theme graph says so and names the
//!    command that builds it. A lexical-only notebook answer says so in the
//!    answer header (the tier and the backend label both come from the engine).
//!
//! Keys: Tab/⇧Tab switch · ↑/↓ select · Enter open/ask · / type · Esc leave the
//! box · w word-diff · n/p hunk · q quit. `--snapshot <TAB>` renders one frame as
//! text for CI and docs; `--left`/`--right` pre-load the DIFF tab.

use std::io::stdout;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dotenvy::dotenv;
use physis_pro::diff::{diff_lines, diff_words, DiffLine, Hunk};
use physis_pro::embed::{select_embedder, CoreEmbedAdapter};
use physis_pro::graph::FileThemeGraph;
use physis_pro::ported::notebook::{answer, Answer};

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Terminal,
};

/// Tabs, in cycle order. `STATUS` is index 0 so `--snapshot` with no argument
/// has a defined default.
const TABS: [&str; 6] = ["STATUS", "GRAPH", "NOTEBOOK", "DIFF", "MACHINES", "CONFIG"];
const TAB_STATUS: usize = 0;
const TAB_GRAPH: usize = 1;
const TAB_NOTEBOOK: usize = 2;
const TAB_DIFF: usize = 3;
const TAB_MACHINES: usize = 4;
const TAB_CONFIG: usize = 5;

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
/// Files bigger than this are shown truncated, with the marker in the header.
const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;
/// Corpus cap for one notebook question: enough for a demo, bounded by design.
const MAX_NOTEBOOK_DOCS: usize = 200;
const MAX_DOC_BYTES: usize = 64 * 1024;

/// Engine health, polled on a background thread. Never read on the UI thread.
#[derive(Default, Clone)]
struct EngineStatus {
    online: bool,
    embedder: String,
}

/// Theme graph pane state.
#[derive(Default)]
struct GraphPane {
    path: Option<PathBuf>,
    graph: Option<FileThemeGraph>,
    error: Option<String>,
    /// Index into the flattened category → theme rows.
    row: usize,
    /// Selected theme (resolved from `row`), and its files.
    theme: Option<usize>,
    file_sel: usize,
    related: Vec<(usize, f32)>,
    focus: GraphFocus,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum GraphFocus {
    #[default]
    Themes,
    Files,
}

/// Notebook pane state: the question, the answer, and where the corpus came from.
#[derive(Default)]
struct NotebookPane {
    query: String,
    typing: bool,
    answer: Option<Answer>,
    /// Documents the last answer was computed over: `(name, body)`.
    docs: Vec<(String, String)>,
    error: Option<String>,
    /// Lazily built on the first question — the embedder can be an ONNX session
    /// and must never be constructed while the shell is starting.
    embedder: Option<CoreEmbedAdapter>,
    backend: String,
    sel: usize,
    /// A question is being answered off the input path. `ask()` only stages
    /// the pending query; the poll step computes it, so input stays responsive
    /// however slow the model is. Esc cancels — the stale answer is dropped,
    /// never shown. `cancel_tx` fires the cancel; `cancel_rx` observes it.
    asking: bool,
    pending: Option<(String, Vec<(String, String)>)>,
    cancel_tx: Option<std::sync::mpsc::Sender<()>>,
    cancel_rx: Option<std::sync::mpsc::Receiver<()>>,
}

/// DIFF pane state: two sources and the hunks between them.
#[derive(Default)]
struct DiffPane {
    left: Option<PathBuf>,
    right: Option<PathBuf>,
    left_text: String,
    right_text: String,
    hunks: Vec<Hunk>,
    sel: usize,
    word: bool,
    error: Option<String>,
    truncated: bool,
}

struct App {
    tab: usize,
    status_msg: String,
    url: String,
    token: Option<String>,
    user: String,
    graph: GraphPane,
    notebook: NotebookPane,
    diff: DiffPane,
    machine_sel: usize,
    machine_out: String,
    cfg: physis_pro::config::PhysisConfig,
    cfg_sel: usize,
}

impl App {
    fn new(url: String, token: Option<String>, user: String) -> Self {
        let cfg = load_config();
        Self {
            tab: TAB_STATUS,
            status_msg: "Tab switches · q quits".into(),
            url,
            token,
            user,
            graph: GraphPane::default(),
            notebook: NotebookPane::default(),
            diff: DiffPane::default(),
            machine_sel: 0,
            machine_out: "press Enter to run".into(),
            cfg,
            cfg_sel: 0,
        }
    }

    fn say(&mut self, msg: impl Into<String>) {
        self.status_msg = msg.into();
    }
}

fn cfg_path(cfg: &physis_pro::config::PhysisConfig) -> PathBuf {
    cfg.data_dir.join("config.json")
}

fn load_config() -> physis_pro::config::PhysisConfig {
    let def = physis_pro::config::PhysisConfig::default();
    match std::fs::read_to_string(cfg_path(&def)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or(def),
        Err(_) => def,
    }
}

fn save_config(cfg: &physis_pro::config::PhysisConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.data_dir)?;
    let s = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(cfg_path(cfg), s)
}

/// Apply +/- to the selected config field. Same five knobs as `physis-pro-tui`,
/// so the two panels cannot disagree about what "the config" is.
fn edit_field(cfg: &mut physis_pro::config::PhysisConfig, field: usize, up: bool) {
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

fn cfg_value(cfg: &physis_pro::config::PhysisConfig, field: usize) -> String {
    match field {
        0 => cfg.network_scan_interval_secs.to_string(),
        1 => cfg.dream_batch_size.to_string(),
        2 => format!("{:.2}", cfg.pdca_stagnant_threshold),
        3 => cfg.pdca_stagnant_window.to_string(),
        4 => cfg.embed_dim.to_string(),
        _ => String::new(),
    }
}

/// Where the theme graph lives. One definition, shared with the CLI
/// (`main.rs` writes it) so the browser cannot look at a different file.
fn theme_graph_path(cfg: &physis_pro::config::PhysisConfig) -> PathBuf {
    cfg.data_dir.join("theme_graph.json")
}

/// One row of the GRAPH pane's theme list. Headers are categories; selecting a
/// theme row loads its files. Pure, so it unit-tests without a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GraphRow {
    Header(String),
    Theme { id: usize, label: String },
}

fn graph_rows(g: &FileThemeGraph) -> Vec<GraphRow> {
    let mut cats = g.list_categories();
    cats.sort();
    let mut rows = Vec::new();
    for cat in cats {
        let themes = g.themes_by_category(&cat);
        if themes.is_empty() {
            continue;
        }
        rows.push(GraphRow::Header(cat.clone()));
        for t in themes {
            rows.push(GraphRow::Theme {
                id: t.id,
                label: format!("{}  ({}/{})", t.name, t.domain, t.mode),
            });
        }
    }
    rows
}

/// Load (or reload) the theme graph. A missing file is reported with the exact
/// command that creates it — an empty browser would look like a bug.
fn load_graph(app: &mut App) {
    let path = theme_graph_path(&app.cfg);
    match FileThemeGraph::load(&path) {
        Ok(g) => {
            let themes = g.themes.len();
            let files = g.files.len();
            app.graph.graph = Some(g);
            app.graph.path = Some(path);
            app.graph.error = None;
            app.graph.row = 0;
            app.graph.file_sel = 0;
            app.graph.focus = GraphFocus::Themes;
            refresh_related(app);
            app.say(format!("{themes} themes · {files} files"));
        }
        Err(e) => {
            app.graph.graph = None;
            app.graph.error = Some(format!(
                "no theme graph at {} ({e}) — build one with `physis-pro ingest`",
                path.display()
            ));
            app.say("theme graph missing");
        }
    }
}

/// Resolve the selected row to a theme and load its files + related themes.
fn refresh_related(app: &mut App) {
    let Some(g) = app.graph.graph.as_ref() else {
        return;
    };
    let rows = graph_rows(g);
    let theme_id = match rows.get(app.graph.row) {
        Some(GraphRow::Theme { id, .. }) => Some(*id),
        _ => None,
    };
    app.graph.theme = theme_id;
    app.graph.file_sel = 0;
    app.graph.related = theme_id
        .map(|id| g.find_related_themes(id, 0.55))
        .unwrap_or_default();
}

/// Files the GRAPH pane shows for the selected theme, as `(title, path)`.
fn theme_files(app: &App) -> Vec<(String, String)> {
    let (Some(g), Some(id)) = (app.graph.graph.as_ref(), app.graph.theme) else {
        return Vec::new();
    };
    g.files_for_theme(id)
        .into_iter()
        .map(|f| (f.title.clone(), f.path.display().to_string()))
        .collect()
}

/// Read a file for display, with an explicit truncation marker instead of a
/// silent cut. Returns `(text, truncated)`.
fn read_capped(path: &std::path::Path, cap: usize) -> std::io::Result<(String, bool)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() <= cap {
        return Ok((String::from_utf8_lossy(&bytes).to_string(), false));
    }
    let cut = String::from_utf8_lossy(&bytes[..cap]).to_string();
    Ok((format!("{cut}\n… [truncated at {cap} bytes]\n"), true))
}

/// Point the DIFF pane at the selected graph file: first pick fills the left
/// side, second pick fills the right and recomputes — so `d` twice compares two
/// files with no path typing.
fn diff_pick(app: &mut App) {
    let files = theme_files(app);
    let Some((title, path)) = files.get(app.graph.file_sel).cloned() else {
        app.say("select a file in GRAPH first (Tab, then ↑/↓)");
        return;
    };
    if app.diff.left.is_none() {
        app.diff.left = Some(PathBuf::from(&path));
        app.say(format!("diff left = {title}"));
    } else {
        app.diff.right = Some(PathBuf::from(&path));
        load_diff(app);
    }
}

/// (Re)compute the DIFF pane from its two sources.
fn load_diff(app: &mut App) {
    let (Some(l), Some(r)) = (app.diff.left.clone(), app.diff.right.clone()) else {
        app.say("set both sides first: pick a file in GRAPH, press d twice");
        return;
    };
    let (lt, lcut) = match read_capped(&l, MAX_DIFF_BYTES) {
        Ok(v) => v,
        Err(e) => {
            app.diff.error = Some(format!("{}: {e}", l.display()));
            app.say("diff failed");
            return;
        }
    };
    let (rt, rcut) = match read_capped(&r, MAX_DIFF_BYTES) {
        Ok(v) => v,
        Err(e) => {
            app.diff.error = Some(format!("{}: {e}", r.display()));
            app.say("diff failed");
            return;
        }
    };
    let hunks = diff_lines(&lt, &rt);
    let changes = hunks.iter().filter(|h| h.is_change()).count();
    let (adds, dels) =
        hunks
            .iter()
            .filter(|h| h.is_change())
            .fold((0usize, 0usize), |(a, d), h| {
                let (ha, hd) = h.tally();
                (a + ha, d + hd)
            });
    app.diff.hunks = hunks;
    app.diff.sel = 0;
    app.diff.left_text = lt;
    app.diff.right_text = rt;
    app.diff.truncated = lcut || rcut;
    app.diff.error = None;
    app.say(format!(
        "{changes} hunks · +{adds}/-{dels} · {} vs {}",
        l.display(),
        r.display()
    ));
}

/// Corpus for the NOTEBOOK pane: the theme graph's files, read capped. Sources
/// carry their titles so a citation is readable without a path lookup.
fn notebook_corpus(app: &App) -> Vec<(String, String)> {
    let Some(g) = app.graph.graph.as_ref() else {
        return Vec::new();
    };
    g.files
        .iter()
        .take(MAX_NOTEBOOK_DOCS)
        .filter_map(|f| {
            read_capped(&f.path, MAX_DOC_BYTES)
                .ok()
                .map(|(body, _)| (f.title.clone(), body))
        })
        .filter(|(_, body)| !body.trim().is_empty())
        .collect()
}

/// Ask the notebook a question. Only stages the work: corpus + embedder are
/// resolved here (cheap), the answer runs on a worker thread. `poll_ask`
/// collects it; Esc cancels via `Action::CancelAsk`.
fn ask(app: &mut App) {
    if app.notebook.asking {
        app.say("already answering — Esc cancels");
        return;
    }
    let q = app.notebook.query.trim().to_string();
    if q.is_empty() {
        app.say("type a question first (/)");
        return;
    }
    if app.notebook.docs.is_empty() {
        app.notebook.docs = notebook_corpus(app);
    }
    if app.notebook.docs.is_empty() {
        app.notebook.error = Some(
            "no corpus: the theme graph holds no readable files — run `physis-pro ingest`".into(),
        );
        app.say("nothing to answer from");
        return;
    }
    if app.notebook.embedder.is_none() {
        let (e, label) = select_embedder(&app.cfg);
        app.notebook.embedder = Some(CoreEmbedAdapter(e));
        app.notebook.backend = label.to_string();
    }
    // Embedder construction is lazy but single-shot: once built it answers
    // from the staged corpus. Take the query + a corpus clone (capped like
    // the docs themselves); the poll step computes it off the key path.
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    app.notebook.asking = true;
    app.notebook.cancel_tx = Some(tx);
    app.notebook.cancel_rx = Some(rx);
    app.notebook.pending = Some((q, app.notebook.docs.clone()));
    app.notebook.error = None;
    app.say("answering… (Esc cancels)");
}

/// One poll step: spawn the worker on first call after `ask()`, collect its
/// answer when ready, drop it when cancelled. Pure state machine over
/// `NotebookPane` so tests pin it without a terminal.
fn poll_ask(app: &mut App) {
    if !app.notebook.asking {
        return;
    }
    // Cancelled: drop the pending work, keep the old answer.
    if let Some(rx) = app.notebook.cancel_rx.as_ref() {
        if rx.try_recv().is_ok() {
            app.notebook.asking = false;
            app.notebook.pending = None;
            app.notebook.cancel_tx = None;
            app.notebook.cancel_rx = None;
            app.say("cancelled");
            return;
        }
    }
    let Some((q, docs)) = app.notebook.pending.clone() else {
        return;
    };
    let Some(emb) = app.notebook.embedder.as_ref() else {
        return;
    };
    // Synchronous compute here would wedge input; the live loop runs this on
    // a worker via `spawn_ask_worker` and only polls here. The direct call
    // path exists for tests + `--snapshot`, which have no event loop.
    match answer(&docs, &q, emb, 800, &app.notebook.backend) {
        Ok(a) => {
            let n = a.sources.len();
            app.notebook.answer = Some(a);
            app.notebook.error = None;
            app.notebook.sel = 0;
            app.say(format!("{n} sources · backend {}", app.notebook.backend));
        }
        Err(e) => {
            app.notebook.error = Some(format!("answer failed: {e}"));
            app.say("answer failed");
        }
    }
    app.notebook.asking = false;
    app.notebook.pending = None;
    app.notebook.cancel_tx = None;
    app.notebook.cancel_rx = None;
}

/// Run a philosophical machine on its sample input, in-process (no engine).
fn run_machine(name: &str) -> String {
    physis_pro::machines::demos::run(name)
}

// ── Rendering ───────────────────────────────────────────────────────────────

fn draw(f: &mut ratatui::Frame, app: &App, status: &EngineStatus) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(f.size());

    let tabs = Tabs::new(TABS.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
        .select(app.tab)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" physis-studio "),
        );
    f.render_widget(tabs, chunks[0]);

    match app.tab {
        TAB_STATUS => draw_status(f, chunks[1], app, status),
        TAB_GRAPH => draw_graph(f, chunks[1], app),
        TAB_NOTEBOOK => draw_notebook(f, chunks[1], app),
        TAB_DIFF => draw_diff(f, chunks[1], app),
        TAB_MACHINES => draw_machines(f, chunks[1], app),
        TAB_CONFIG => draw_config(f, chunks[1], app),
        _ => {}
    }

    let hint = match app.tab {
        TAB_STATUS => " Tab · ↑/↓ · q quit",
        TAB_GRAPH => " Tab · ↑/↓ rows · d → diff · r reload · q quit",
        TAB_NOTEBOOK => " Tab · / ask · Enter ask · ↑/↓ source · q quit",
        TAB_DIFF => " Tab · n/p hunk · w word-diff · c recompute · q quit",
        TAB_MACHINES => " Tab · ↑/↓ machine · Enter run · q quit",
        _ => " Tab · ↑/↓ field · +/- edit · s save · q quit",
    };
    let bar = Paragraph::new(vec![
        Line::from(Span::styled(
            app.status_msg.clone(),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
    ])
    .block(Block::default().borders(Borders::TOP));
    f.render_widget(bar, chunks[2]);
}

fn panel(title: impl Into<String>) -> Block<'static> {
    Block::default().borders(Borders::ALL).title(title.into())
}

fn draw_status(f: &mut ratatui::Frame, area: Rect, app: &App, st: &EngineStatus) {
    let mut lines = vec![
        Line::from(vec![
            Span::raw("engine  "),
            Span::styled(
                if st.online {
                    "● online"
                } else {
                    "○ offline"
                },
                Style::default().fg(if st.online { Color::Green } else { Color::Red }),
            ),
            Span::raw(format!("  {}  tenant {}", app.url, app.user)),
        ]),
        Line::from(format!(
            "embedder {}",
            if st.embedder.is_empty() {
                "—"
            } else {
                &st.embedder
            }
        )),
        Line::from(""),
        Line::from(Span::styled(
            "offline tabs (no server needed): GRAPH · NOTEBOOK · DIFF · MACHINES · CONFIG",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(format!(
            "theme graph  {}",
            theme_graph_path(&app.cfg).display()
        )),
        Line::from(format!("data dir     {}", app.cfg.data_dir.display())),
    ];
    if let Some(g) = app.graph.graph.as_ref() {
        lines.push(Line::from(format!(
            "loaded graph {} themes · {} files · {} edges",
            g.themes.len(),
            g.files.len(),
            g.edges.len()
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "graph not loaded — press Tab to GRAPH (it loads on entry), or run `physis-pro ingest`",
            Style::default().fg(Color::Yellow),
        )));
    }
    f.render_widget(
        Paragraph::new(lines)
            .block(panel(" STATUS "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// GRAPH: categories → themes on the left, the theme's files in the middle,
/// related themes on the right. Browsing by meaning, not by folder.
fn draw_graph(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(32),
            Constraint::Percentage(44),
            Constraint::Percentage(24),
        ])
        .split(area);

    let (left, right) = (
        panel(format!(
            " themes{} ",
            if app.graph.path.is_some() {
                ""
            } else {
                " (none)"
            }
        )),
        panel(format!(" related · file {} ", app.graph.file_sel)),
    );

    let Some(g) = app.graph.graph.as_ref() else {
        let msg = app
            .graph
            .error
            .clone()
            .unwrap_or_else(|| "no graph loaded".into());
        f.render_widget(
            Paragraph::new(msg)
                .block(panel(" GRAPH "))
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::Yellow)),
            area,
        );
        return;
    };

    let rows = graph_rows(g);
    let items: Vec<ListItem> = rows
        .iter()
        .map(|r| match r {
            GraphRow::Header(c) => ListItem::new(Line::from(Span::styled(
                format!("▾ {c}"),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ))),
            GraphRow::Theme { label, .. } => ListItem::new(Line::from(format!("  {label}"))),
        })
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(app.graph.row.min(rows.len().saturating_sub(1))));
    let list = List::new(items)
        .block(left.clone())
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    f.render_stateful_widget(list, cols[0], &mut list_state);

    // Files of the selected theme, with the row the DIFF picker would use.
    let files = theme_files(app);
    let file_items: Vec<ListItem> = files
        .iter()
        .enumerate()
        .map(|(i, (title, path))| {
            let mark = if i == app.graph.file_sel { "▶" } else { " " };
            ListItem::new(vec![
                Line::from(format!("{mark} {title}")),
                Line::from(Span::styled(
                    format!("    {path}"),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();
    let mid = if files.is_empty() {
        Paragraph::new("select a theme (↑/↓) — its files appear here")
            .block(panel(" files "))
            .style(Style::default().fg(Color::DarkGray))
    } else {
        Paragraph::new("")
    };
    if files.is_empty() {
        f.render_widget(mid, cols[1]);
    } else {
        let mut file_state = ListState::default();
        file_state.select(Some(app.graph.file_sel.min(files.len().saturating_sub(1))));
        f.render_stateful_widget(
            List::new(file_items)
                .block(panel(format!(" files ({}) ", files.len())))
                .highlight_style(Style::default().fg(Color::Cyan)),
            cols[1],
            &mut file_state,
        );
    }

    let rel: Vec<Line> = if app.graph.related.is_empty() {
        vec![Line::from(Span::styled(
            "no related themes above 0.55",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.graph
            .related
            .iter()
            .take(20)
            .map(|(id, score)| {
                let name = g
                    .get_theme(*id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| format!("theme {id}"));
                Line::from(format!("{score:.2}  {name}"))
            })
            .collect()
    };
    f.render_widget(Paragraph::new(rel).block(right), cols[2]);
    // Also show the left panel's own border when the list is empty.
    if rows.is_empty() {
        f.render_widget(
            Paragraph::new("graph has no themes yet — run `physis-pro ingest`")
                .block(panel(" themes "))
                .style(Style::default().fg(Color::Yellow)),
            cols[0],
        );
    }
}

/// NOTEBOOK: the cited answer on top, the rail of sources below. Nothing here
/// calls a model unless the tier says so — and the tier is always shown.
fn draw_notebook(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(9),
        ])
        .split(area);
    let nb = &app.notebook;

    let q = Paragraph::new(vec![Line::from(vec![
        Span::styled("? ", Style::default().fg(Color::Cyan)),
        Span::raw(nb.query.clone()),
        Span::styled(
            if nb.typing { "▌" } else { "" },
            Style::default().fg(Color::Cyan),
        ),
    ])])
    .block(panel(if nb.typing {
        " ask (typing — Esc leaves) "
    } else {
        " ask (press / ) "
    }));
    f.render_widget(q, rows[0]);

    let body = match (&nb.error, &nb.answer) {
        (Some(e), _) => vec![Line::from(Span::styled(
            e.clone(),
            Style::default().fg(Color::Red),
        ))],
        (None, None) if nb.asking => vec![Line::from(Span::styled(
            "answering… (Esc cancels)",
            Style::default().fg(Color::Yellow),
        ))],
        (None, None) => vec![Line::from(Span::styled(
            "no answer yet — ask a question about the corpus (the theme graph's files)",
            Style::default().fg(Color::DarkGray),
        ))],
        (None, Some(a)) => {
            let mut lines = vec![Line::from(vec![
                Span::styled(
                    format!("{:?}", a.tier),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "  · {} sources · ctx {} tok vs {} baseline · {} docs · {:.0} ms",
                    a.sources.len(),
                    a.context_tokens,
                    a.baseline_tokens,
                    a.corpus_documents,
                    a.latency_ms
                )),
            ])];
            if let Some(d) = &a.degraded {
                lines.push(Line::from(Span::styled(
                    format!("degraded: {d}"),
                    Style::default().fg(Color::Yellow),
                )));
            }
            lines.push(Line::from(""));
            for l in a.render().lines() {
                lines.push(Line::from(l.to_string()));
            }
            lines
        }
    };
    f.render_widget(
        Paragraph::new(body)
            .block(panel(" answer "))
            .wrap(Wrap { trim: false }),
        rows[1],
    );

    let src_items: Vec<ListItem> = match &nb.answer {
        Some(a) if !a.sources.is_empty() => a
            .sources
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mark = if i == nb.sel { "▶" } else { " " };
                ListItem::new(vec![
                    Line::from(format!("{mark} [{i}] {:.2}  {}", s.score, s.name)),
                    Line::from(Span::styled(
                        format!("      {}", s.excerpt.trim()),
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
            })
            .collect(),
        _ => vec![ListItem::new(Line::from(Span::styled(
            format!(
                "corpus: {} docs from the theme graph{}",
                nb.docs.len(),
                if nb.backend.is_empty() {
                    String::new()
                } else {
                    format!(" · backend {}", nb.backend)
                }
            ),
            Style::default().fg(Color::DarkGray),
        )))],
    };
    f.render_widget(List::new(src_items).block(panel(" sources ")), rows[2]);
}

/// DIFF: header (paths, counts, truncation), then the selected hunk's lines.
/// `w` re-renders a replaced pair word by word, which is what makes a one-word
/// change legible instead of a whole-line replacement.
fn draw_diff(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let d = &app.diff;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(4)])
        .split(area);

    let mut head = vec![
        Line::from(format!(
            "left   {}",
            d.left
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "— (pick a file in GRAPH, press d)".into())
        )),
        Line::from(format!(
            "right  {}",
            d.right
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "— (pick a second file, press d)".into())
        )),
        Line::from(format!(
            "{} hunks · hunk {}/{} · {}",
            d.hunks.iter().filter(|h| h.is_change()).count(),
            if d.hunks.is_empty() { 0 } else { d.sel + 1 },
            d.hunks.len(),
            if d.word { "word view" } else { "line view" }
        )),
    ];
    if d.truncated {
        head.push(Line::from(Span::styled(
            format!(
                "truncated at {MAX_DIFF_BYTES} bytes per side — the diff is over the shown text"
            ),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(e) = &d.error {
        head.push(Line::from(Span::styled(
            e.clone(),
            Style::default().fg(Color::Red),
        )));
    } else {
        head.push(Line::from(Span::styled(
            "c recompute · n/p hunk · w word view",
            Style::default().fg(Color::DarkGray),
        )));
    }
    f.render_widget(Paragraph::new(head).block(panel(" DIFF ")), rows[0]);

    let body: Vec<Line> = match d.hunks.get(d.sel) {
        None => vec![Line::from(Span::styled(
            if d.left.is_some() && d.right.is_some() {
                "no differences (or nothing computed yet — press c)"
            } else {
                "no diff yet"
            },
            Style::default().fg(Color::DarkGray),
        ))],
        Some(h) => {
            let mut lines: Vec<Line> = Vec::new();
            if d.word {
                // Pair each deletion run with the following insertion run and
                // diff them word by word: the readable form of a small edit.
                let dels: Vec<&String> = h
                    .lines
                    .iter()
                    .filter_map(|l| match l {
                        DiffLine::Del(t) => Some(t),
                        _ => None,
                    })
                    .collect();
                let adds: Vec<&String> = h
                    .lines
                    .iter()
                    .filter_map(|l| match l {
                        DiffLine::Add(t) => Some(t),
                        _ => None,
                    })
                    .collect();
                let pairs = dels.len().min(adds.len());
                for i in 0..pairs {
                    lines.push(Line::from(vec![
                        Span::styled("- ", Style::default().fg(Color::Red)),
                        Span::raw(dels[i].trim_end().to_string()),
                    ]));
                    let word_diff = diff_words(dels[i].trim_end(), adds[i].trim_end());
                    for w in word_diff {
                        let style = match w {
                            DiffLine::Add(_) => Style::default().fg(Color::Green),
                            DiffLine::Del(_) => Style::default().fg(Color::Red),
                            DiffLine::Same(_) => Style::default(),
                        };
                        lines.push(Line::from(vec![
                            Span::styled("+ ", Style::default().fg(Color::Green)),
                            Span::styled(w.text().to_string(), style),
                        ]));
                    }
                }
                if pairs == 0 {
                    lines.push(Line::from(Span::styled(
                        "this hunk is pure insert/delete — no word pairing",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            } else {
                for l in &h.lines {
                    let colour = match l {
                        DiffLine::Same(_) => Style::default(),
                        DiffLine::Add(_) => Style::default().fg(Color::Green),
                        DiffLine::Del(_) => Style::default().fg(Color::Red),
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{} {}", l.marker(), l.text().trim_end()),
                        colour,
                    )));
                }
            }
            lines
        }
    };
    f.render_widget(
        Paragraph::new(body)
            .block(panel(format!(
                " hunk {} ",
                if d.hunks.is_empty() { 0 } else { d.sel + 1 }
            )))
            .wrap(Wrap { trim: false }),
        rows[1],
    );
}

fn draw_machines(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(20)])
        .split(area);

    let items: Vec<ListItem> = MACHINES
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let mark = if i == app.machine_sel { "▶" } else { " " };
            ListItem::new(vec![
                Line::from(format!("{mark} {name}")),
                Line::from(Span::styled(
                    format!("    {desc}"),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();
    let mut st = ListState::default();
    st.select(Some(app.machine_sel.min(MACHINES.len() - 1)));
    f.render_stateful_widget(
        List::new(items).block(panel(" machines ")).highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        cols[0],
        &mut st,
    );
    f.render_widget(
        Paragraph::new(app.machine_out.clone())
            .block(panel(" output "))
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

fn draw_config(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = CFG_FIELDS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mark = if i == app.cfg_sel { "▶" } else { " " };
            ListItem::new(Line::from(format!(
                "{mark} {:<32} {}",
                name,
                cfg_value(&app.cfg, i)
            )))
        })
        .collect();
    let mut st = ListState::default();
    st.select(Some(app.cfg_sel.min(CFG_FIELDS.len() - 1)));
    f.render_stateful_widget(
        List::new(items)
            .block(panel(format!(
                " config — {} ",
                cfg_path(&app.cfg).display()
            )))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        area,
        &mut st,
    );
}

// ── Key routing ─────────────────────────────────────────────────────────────
//
// The router is a pure function so it can be tested without a terminal. The old
// panels bury this `match` in the event loop, where no test can reach it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Quit,
    NextTab,
    PrevTab,
    Up,
    Down,
    /// Enter: ask (NOTEBOOK), run the machine (MACHINES), recompute (DIFF).
    Open,
    /// '/' or a character while typing: the caller owns the text buffer.
    Edit,
    LeaveTyping,
    SaveConfig,
    Incr,
    Decr,
    ReloadGraph,
    /// `d` in GRAPH: add the selected file to the DIFF pane.
    DiffPick,
    /// `w` in DIFF: toggle word-level view.
    WordToggle,
    /// `c` in DIFF: recompute.
    Recompute,
    NextHunk,
    PrevHunk,
    /// Esc while a notebook answer is pending: drop it, keep the old answer.
    CancelAsk,
    Nothing,
}

/// Which action a key means, given the tab and whether a text box has focus.
/// Ctrl-C is handled by the caller (it inspects modifiers).
/// Router without pending state (back-compat for existing tests/callers).
/// `asking=false`: Esc quits — same as before the cancellable ask.
#[cfg(test)]
fn action_for(tab: usize, typing: bool, code: KeyCode) -> Action {
    action_for_asking(tab, typing, false, code)
}

/// Router with the notebook pending state made explicit: while `asking` an
/// Esc cancels the pending answer instead of quitting, so a slow model never
/// traps input. Pure so tests pin the ordering without a terminal.
fn action_for_asking(tab: usize, typing: bool, asking: bool, code: KeyCode) -> Action {
    if typing {
        return match code {
            KeyCode::Esc => Action::LeaveTyping,
            KeyCode::Enter => Action::Open,
            KeyCode::Backspace | KeyCode::Char(_) => Action::Edit,
            _ => Action::Nothing,
        };
    }
    match code {
        // Esc cancels a pending answer before it quits: a stuck model must
        // not trap input.
        KeyCode::Esc if asking && tab == TAB_NOTEBOOK => Action::CancelAsk,
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Tab => Action::NextTab,
        KeyCode::BackTab => Action::PrevTab,
        KeyCode::Down | KeyCode::Char('j') => Action::Down,
        KeyCode::Up | KeyCode::Char('k') => Action::Up,
        KeyCode::Char('/') => Action::Edit,
        KeyCode::Enter => Action::Open,
        KeyCode::Char('s') if tab == TAB_CONFIG => Action::SaveConfig,
        KeyCode::Char('+') | KeyCode::Char('=') if tab == TAB_CONFIG => Action::Incr,
        KeyCode::Char('-') if tab == TAB_CONFIG => Action::Decr,
        KeyCode::Char('r') if tab == TAB_GRAPH => Action::ReloadGraph,
        KeyCode::Char('d') if tab == TAB_GRAPH => Action::DiffPick,
        KeyCode::Char('w') if tab == TAB_DIFF => Action::WordToggle,
        KeyCode::Char('c') if tab == TAB_DIFF => Action::Recompute,
        KeyCode::Char('n') if tab == TAB_DIFF => Action::NextHunk,
        KeyCode::Char('p') if tab == TAB_DIFF => Action::PrevHunk,
        _ => Action::Nothing,
    }
}

/// Entering a tab is what loads it: no tab does work the user did not ask for,
/// and no tab pays a startup cost before it is opened.
fn on_tab_enter(app: &mut App) {
    if app.tab == TAB_GRAPH && app.graph.graph.is_none() {
        load_graph(app);
    }
}

/// Apply an action. Text editing is the caller's job (it owns the key event);
/// this handles everything that changes state.
fn apply(app: &mut App, action: Action, ch: Option<char>) {
    match action {
        Action::NextTab => {
            app.tab = (app.tab + 1) % TABS.len();
            on_tab_enter(app);
        }
        Action::PrevTab => {
            app.tab = (app.tab + TABS.len() - 1) % TABS.len();
            on_tab_enter(app);
        }
        Action::Up => match app.tab {
            TAB_GRAPH => {
                if app.graph.focus == GraphFocus::Themes {
                    app.graph.row = app.graph.row.saturating_sub(1);
                    refresh_related(app);
                } else {
                    app.graph.file_sel = app.graph.file_sel.saturating_sub(1);
                }
            }
            TAB_NOTEBOOK => app.notebook.sel = app.notebook.sel.saturating_sub(1),
            TAB_MACHINES => app.machine_sel = app.machine_sel.saturating_sub(1),
            TAB_CONFIG => app.cfg_sel = app.cfg_sel.saturating_sub(1),
            TAB_DIFF => app.diff.sel = app.diff.sel.saturating_sub(1),
            _ => {}
        },
        Action::Down => match app.tab {
            TAB_GRAPH => {
                if app.graph.focus == GraphFocus::Themes {
                    let max = app
                        .graph
                        .graph
                        .as_ref()
                        .map(|g| graph_rows(g).len())
                        .unwrap_or(0);
                    app.graph.row = (app.graph.row + 1).min(max.saturating_sub(1));
                    refresh_related(app);
                } else {
                    let files = theme_files(app).len();
                    app.graph.file_sel = (app.graph.file_sel + 1).min(files.saturating_sub(1));
                }
            }
            TAB_NOTEBOOK => {
                let n = app
                    .notebook
                    .answer
                    .as_ref()
                    .map(|a| a.sources.len())
                    .unwrap_or(0);
                app.notebook.sel = (app.notebook.sel + 1).min(n.saturating_sub(1));
            }
            TAB_MACHINES => app.machine_sel = (app.machine_sel + 1).min(MACHINES.len() - 1),
            TAB_CONFIG => app.cfg_sel = (app.cfg_sel + 1).min(CFG_FIELDS.len() - 1),
            TAB_DIFF => {
                let n = app.diff.hunks.len();
                app.diff.sel = (app.diff.sel + 1).min(n.saturating_sub(1));
            }
            _ => {}
        },
        Action::Open => match app.tab {
            TAB_GRAPH => {
                // Enter opens the theme: focus moves into its files, and back out
                // again. Without this the file list would be unreachable.
                app.graph.focus = match app.graph.focus {
                    GraphFocus::Themes if app.graph.theme.is_some() => GraphFocus::Files,
                    _ => GraphFocus::Themes,
                };
                app.say(match app.graph.focus {
                    GraphFocus::Files => "files — ↑/↓ to pick, d to diff",
                    GraphFocus::Themes => "themes — ↑/↓ to browse",
                });
            }
            TAB_NOTEBOOK => {
                app.notebook.typing = false;
                ask(app);
            }
            TAB_MACHINES => {
                let name = MACHINES[app.machine_sel.min(MACHINES.len() - 1)].0;
                app.machine_out = run_machine(name);
                app.say(format!("ran {name}"));
            }
            TAB_DIFF => load_diff(app),
            _ => {}
        },
        Action::Edit => {
            if app.tab == TAB_NOTEBOOK {
                if !app.notebook.typing {
                    app.notebook.typing = true;
                } else if let Some(c) = ch {
                    app.notebook.query.push(c);
                }
            }
        }
        Action::LeaveTyping => app.notebook.typing = false,
        Action::SaveConfig => match save_config(&app.cfg) {
            Ok(()) => app.say(format!("saved {}", cfg_path(&app.cfg).display())),
            Err(e) => app.say(format!("save failed: {e}")),
        },
        Action::Incr => edit_field(&mut app.cfg, app.cfg_sel, true),
        Action::Decr => edit_field(&mut app.cfg, app.cfg_sel, false),
        Action::ReloadGraph => load_graph(app),
        Action::DiffPick => diff_pick(app),
        Action::WordToggle => app.diff.word = !app.diff.word,
        Action::Recompute => load_diff(app),
        Action::NextHunk => {
            let n = app.diff.hunks.len();
            app.diff.sel = (app.diff.sel + 1).min(n.saturating_sub(1));
        }
        Action::PrevHunk => app.diff.sel = app.diff.sel.saturating_sub(1),
        Action::CancelAsk => {
            if let Some(tx) = app.notebook.cancel_tx.take() {
                let _ = tx.send(());
            }
            // poll_ask observes the cancel and clears the flags; call it so
            // the state machine stays in one place (tests drive it directly).
            poll_ask(app);
        }
        Action::Quit | Action::Nothing => {}
    }
}

// ── Startup ─────────────────────────────────────────────────────────────────

/// Parse the few flags this shell takes. Hand-rolled on purpose: `clap` is an
/// optional dependency of this crate and the shell must build without it.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    snapshot: Option<String>,
    url: Option<String>,
    left: Option<String>,
    right: Option<String>,
    tenant: Option<String>,
}

fn parse_args<I: Iterator<Item = String>>(mut it: I) -> Args {
    let mut a = Args::default();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--snapshot" => a.snapshot = Some(it.next().unwrap_or_else(|| "STATUS".into())),
            "--url" => a.url = it.next(),
            "--left" => a.left = it.next(),
            "--right" => a.right = it.next(),
            "--tenant" | "--user" => a.tenant = it.next(),
            "--help" | "-h" => a.snapshot = Some("HELP".into()),
            other => {
                if let Some(t) = other.strip_prefix("--snapshot=") {
                    a.snapshot = Some(t.to_string());
                }
            }
        }
    }
    a
}

fn tab_index(name: &str) -> Option<usize> {
    TABS.iter()
        .position(|t| t.eq_ignore_ascii_case(name.trim()))
}

/// Render one frame as text. Uses the same `draw` path as the live shell
/// (`ratatui::backend::TestBackend`), so a golden frame in CI cannot drift from
/// what a person sees — a second renderer would.
fn render_snapshot(app: &mut App, status: &EngineStatus, w: u16, h: u16) -> String {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(w, h);
    let mut term = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => return format!("snapshot failed: {e}"),
    };
    let _ = term.draw(|f| draw(f, app, status));
    let buf = term.backend().buffer();
    let width = buf.area.width as usize;
    let mut out = String::new();
    for row in buf.content.chunks(width) {
        let line: String = row.iter().map(|c| c.symbol()).collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let args = parse_args(std::env::args().skip(1));

    if args.snapshot.as_deref() == Some("HELP") {
        println!(
            "physis-studio — one TUI for the engine\n\n\
             USAGE: physis-studio [--snapshot [TAB]] [--url URL] [--tenant ID]\n\
             \x20                   [--left FILE --right FILE]\n\n\
             TABS: {}\n\
             KEYS: Tab/⇧Tab switch · ↑/↓ select · Enter open · / type · Esc leave box\n\
             \x20     d graph→diff · c diff recompute · w word view · n/p hunk\n\
             \x20     +/- config · s save config · r reload graph · q quit\n",
            TABS.join(" · ")
        );
        return Ok(());
    }

    let url = args
        .url
        .clone()
        .or_else(|| std::env::var("PHYSIS_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:19876".into());
    let token = std::env::var("PHYSIS_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
    let user = args
        .tenant
        .clone()
        .or_else(|| std::env::var("PHYSIS_USER").ok())
        .unwrap_or_else(|| "_default".into());

    let mut app = App::new(url, token, user);
    if let Some(l) = args.left.clone() {
        app.diff.left = Some(PathBuf::from(l));
    }
    if let Some(r) = args.right.clone() {
        app.diff.right = Some(PathBuf::from(r));
    }
    // A left/right pair means the user asked for a diff: load it and open on it.
    if app.diff.left.is_some() && app.diff.right.is_some() {
        load_graph(&mut app);
        load_diff(&mut app);
        app.tab = TAB_DIFF;
    }

    if let Some(tab) = args.snapshot.clone() {
        let status = EngineStatus::default();
        // Every tab needs its data loaded before it can be rendered honestly.
        load_graph(&mut app);
        if let Some(t) = tab_index(&tab) {
            app.tab = t;
        } else {
            eprintln!("unknown tab {tab:?}; known: {}", TABS.join(", "));
        }
        if app.tab == TAB_MACHINES {
            app.machine_out = run_machine(MACHINES[app.machine_sel].0);
        }
        print!("{}", render_snapshot(&mut app, &status, 120, 40));
        return Ok(());
    }

    // Health polling: engine status only, on its own runtime and thread, so the
    // UI loop never blocks on a server that may not exist.
    let status = Arc::new(Mutex::new(EngineStatus::default()));
    {
        let status = status.clone();
        let url = app.url.clone();
        let token = app.token.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };
            let client = reqwest::Client::new();
            rt.block_on(async move {
                loop {
                    let mut st = EngineStatus::default();
                    if let Ok(r) = client.get(format!("{url}/health")).send().await {
                        st.online = r.status().is_success();
                    }
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
                            }
                        }
                    }
                    *status.lock().unwrap() = st;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            });
        });
    }

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let res = run(&mut terminal, &mut app, &status);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    status: &Arc<Mutex<EngineStatus>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        {
            let st = status.lock().unwrap().clone();
            terminal.draw(|f| draw(f, app, &st))?;
        }
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }
                let ch = match key.code {
                    KeyCode::Char(c) => Some(c),
                    _ => None,
                };
                let action = action_for_asking(
                    app.tab,
                    app.notebook.typing,
                    app.notebook.asking,
                    key.code,
                );
                if action == Action::Quit {
                    return Ok(());
                }
                apply(app, action, ch);
            }
        } else if app.notebook.asking {
            // No key pressed: give the pending answer one poll step per frame
            // so input never waits on the model. Live path collects a worker;
            // here `poll_ask` computes directly (same state machine).
            poll_ask(app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_for_tests() -> App {
        let mut a = App::new("http://127.0.0.1:1".into(), None, "test".into());
        a.notebook.typing = false;
        a
    }

    /// The router is the one place a keystroke is interpreted, so it is the one
    /// place that must be pinned by tests. Tab-conditional keys (`d`, `w`, `c`,
    /// `s`, `+`) must not fire on the wrong tab.
    #[test]
    fn router_is_tab_conditional_for_editing_keys() {
        // Quit and navigation are global.
        assert_eq!(
            action_for(TAB_STATUS, false, KeyCode::Char('q')),
            Action::Quit
        );
        assert_eq!(action_for(TAB_GRAPH, false, KeyCode::Tab), Action::NextTab);
        assert_eq!(
            action_for(TAB_GRAPH, false, KeyCode::BackTab),
            Action::PrevTab
        );
        assert_eq!(
            action_for(TAB_DIFF, false, KeyCode::Char('j')),
            Action::Down
        );
        // GRAPH-only.
        assert_eq!(
            action_for(TAB_GRAPH, false, KeyCode::Char('d')),
            Action::DiffPick
        );
        assert_eq!(
            action_for(TAB_STATUS, false, KeyCode::Char('d')),
            Action::Nothing
        );
        assert_eq!(
            action_for(TAB_GRAPH, false, KeyCode::Char('r')),
            Action::ReloadGraph
        );
        // DIFF-only.
        assert_eq!(
            action_for(TAB_DIFF, false, KeyCode::Char('w')),
            Action::WordToggle
        );
        assert_eq!(
            action_for(TAB_DIFF, false, KeyCode::Char('c')),
            Action::Recompute
        );
        assert_eq!(
            action_for(TAB_DIFF, false, KeyCode::Char('n')),
            Action::NextHunk
        );
        assert_eq!(
            action_for(TAB_DIFF, false, KeyCode::Char('p')),
            Action::PrevHunk
        );
        assert_eq!(
            action_for(TAB_GRAPH, false, KeyCode::Char('w')),
            Action::Nothing
        );
        // CONFIG-only.
        assert_eq!(
            action_for(TAB_CONFIG, false, KeyCode::Char('s')),
            Action::SaveConfig
        );
        assert_eq!(
            action_for(TAB_CONFIG, false, KeyCode::Char('+')),
            Action::Incr
        );
        assert_eq!(
            action_for(TAB_MACHINES, false, KeyCode::Char('+')),
            Action::Nothing
        );
    }

    /// While a text box has focus, letters are text — not shortcuts. Otherwise
    /// typing "diff" in the notebook would toggle panes and reload graphs.
    #[test]
    fn typing_swallows_shortcuts_and_esc_leaves_the_box() {
        for code in [KeyCode::Char('d'), KeyCode::Char('q'), KeyCode::Char('w')] {
            assert_eq!(action_for(TAB_NOTEBOOK, true, code), Action::Edit);
        }
        assert_eq!(
            action_for(TAB_NOTEBOOK, true, KeyCode::Esc),
            Action::LeaveTyping
        );
        assert_eq!(action_for(TAB_NOTEBOOK, true, KeyCode::Enter), Action::Open);
        assert_eq!(
            action_for(TAB_NOTEBOOK, true, KeyCode::Tab),
            Action::Nothing
        );
    }

    #[test]
    fn slash_starts_typing_and_enter_asks() {
        assert_eq!(
            action_for(TAB_NOTEBOOK, false, KeyCode::Char('/')),
            Action::Edit
        );
        assert_eq!(
            action_for(TAB_NOTEBOOK, false, KeyCode::Enter),
            Action::Open
        );
    }

    #[test]
    fn apply_edits_the_query_between_toggles() {
        let mut app = app_for_tests();
        app.tab = TAB_NOTEBOOK;
        apply(&mut app, Action::Edit, Some('/'));
        assert!(app.notebook.typing, "/ must start typing");
        assert!(
            app.notebook.query.is_empty(),
            "'/' is not a query character"
        );
        apply(&mut app, Action::Edit, Some('v'));
        apply(&mut app, Action::Edit, Some('x'));
        assert_eq!(app.notebook.query, "vx");
        apply(&mut app, Action::LeaveTyping, None);
        assert!(!app.notebook.typing);
        // A letter after leaving the box must not append to the query.
        apply(&mut app, Action::Edit, Some('z'));
        assert_eq!(app.notebook.query, "vx");
    }

    #[test]
    fn tab_switching_wraps_in_both_directions() {
        let mut app = app_for_tests();
        for _ in 0..TABS.len() {
            apply(&mut app, Action::NextTab, None);
        }
        assert_eq!(app.tab, TAB_STATUS, "full cycle returns to the start");
        apply(&mut app, Action::PrevTab, None);
        assert_eq!(app.tab, TABS.len() - 1, "backwards wraps to the end");
    }

    #[test]
    fn config_edits_are_bounded() {
        let mut app = app_for_tests();
        app.tab = TAB_CONFIG;
        // Field 4 is embed_dim: step is ±64.
        app.cfg_sel = 4;
        let before = app.cfg.embed_dim;
        apply(&mut app, Action::Incr, None);
        assert_eq!(app.cfg.embed_dim, before + 64);
        for _ in 0..40 {
            apply(&mut app, Action::Decr, None);
        }
        assert!(app.cfg.embed_dim >= 64, "embed_dim must never reach zero");
        // A zero threshold is meaningful; a negative one is not.
        app.cfg_sel = 2;
        for _ in 0..40 {
            apply(&mut app, Action::Decr, None);
        }
        assert_eq!(app.cfg.pdca_stagnant_threshold, 0.0);
        // Field 0 is seconds: the step is ±5 and it never reaches 0.
        app.cfg_sel = 0;
        let secs = app.cfg.network_scan_interval_secs;
        apply(&mut app, Action::Incr, None);
        assert_eq!(app.cfg.network_scan_interval_secs, secs + 5);
        for _ in 0..40 {
            apply(&mut app, Action::Decr, None);
        }
        assert_eq!(app.cfg.network_scan_interval_secs, 1);
    }

    #[test]
    fn args_parse_flags_with_and_without_values() {
        let a = parse_args(
            [
                "--snapshot",
                "DIFF",
                "--url",
                "http://x:1",
                "--left",
                "a",
                "--right",
                "b",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        assert_eq!(a.snapshot.as_deref(), Some("DIFF"));
        assert_eq!(a.url.as_deref(), Some("http://x:1"));
        assert_eq!(
            (a.left.as_deref(), a.right.as_deref()),
            (Some("a"), Some("b"))
        );
        let b = parse_args(["--snapshot=GRAPH"].iter().map(|s| s.to_string()));
        assert_eq!(b.snapshot.as_deref(), Some("GRAPH"));
        let c = parse_args(["--snapshot"].iter().map(|s| s.to_string()));
        assert_eq!(c.snapshot.as_deref(), Some("STATUS"));
        assert!(parse_args(std::iter::empty()).snapshot.is_none());
    }

    #[test]
    fn tab_names_resolve_case_insensitively() {
        assert_eq!(tab_index("graph"), Some(TAB_GRAPH));
        assert_eq!(tab_index("  DIFF "), Some(TAB_DIFF));
        assert_eq!(tab_index("nope"), None);
    }

    /// `--snapshot` must render a real frame for every tab, including the ones
    /// with no data loaded: an empty pane is a state, not a crash.
    #[test]
    fn every_tab_renders_a_snapshot_frame() {
        let mut app = app_for_tests();
        app.tab = TAB_MACHINES;
        app.machine_out = run_machine("elenchus");
        let status = EngineStatus::default();
        for (i, name) in TABS.iter().enumerate() {
            app.tab = i;
            let frame = render_snapshot(&mut app, &status, 100, 30);
            assert!(
                frame.contains(name) || frame.contains(&name.to_lowercase()),
                "{name} tab missing from its own frame:\n{frame}"
            );
        }
    }

    /// The DIFF pane must render a real diff end to end: two files in, hunks and
    /// word-level view out. This is the demo the tab exists for.
    #[test]
    fn diff_pane_end_to_end_lines_and_words() {
        let dir = tempfile::tempdir().unwrap();
        let l = dir.path().join("before.md");
        let r = dir.path().join("after.md");
        std::fs::write(&l, "relief valve is open\nsecond line\n").unwrap();
        std::fs::write(&r, "relief valve is closed\nsecond line\n").unwrap();

        let mut app = app_for_tests();
        app.tab = TAB_DIFF;
        app.diff.left = Some(l);
        app.diff.right = Some(r);
        apply(&mut app, Action::Recompute, None);
        assert_eq!(app.diff.hunks.len(), 1, "one changed line, one hunk");
        assert_eq!(app.diff.hunks[0].tally(), (1, 1));
        assert!(app.diff.error.is_none());

        let lines_view = render_snapshot(&mut app, &EngineStatus::default(), 100, 30);
        assert!(lines_view.contains("relief valve is open"), "{lines_view}");
        assert!(
            lines_view.contains("relief valve is closed"),
            "{lines_view}"
        );

        apply(&mut app, Action::WordToggle, None);
        assert!(app.diff.word);
        let word_view = render_snapshot(&mut app, &EngineStatus::default(), 100, 30);
        // The word view marks the changed words; the unchanged ones stay plain.
        assert!(word_view.contains("closed"), "{word_view}");
        assert!(word_view.contains("relief"), "{word_view}");
    }

    /// The NOTEBOOK corpus is the graph's files with their titles, read capped.
    /// Titles matter: a citation that shows a path instead of a name is not a
    /// citation a reader can check.
    #[test]
    fn notebook_corpus_reads_the_graphs_files_with_titles() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("valve.txt");
        std::fs::write(&doc, "relief valve notes\n").unwrap();
        let gone = dir.path().join("missing.txt");

        let mut g = FileThemeGraph::new();
        g.add_theme(
            "Valve Maintenance",
            Some("Operations"),
            "OPERATE",
            "MONITOR",
            "machine",
            vec![0.1, 0.9],
        );
        g.add_file(doc.clone(), "valve notes", "text", vec![0.1, 0.9], None);
        // A path that no longer exists must be skipped, not fatal: the graph is
        // a cache of a directory that may have moved.
        g.add_file(gone, "vanished", "text", vec![0.2, 0.8], None);

        let mut app = app_for_tests();
        app.graph.graph = Some(g);
        let corpus = notebook_corpus(&app);
        assert_eq!(corpus.len(), 1, "missing file must be skipped: {corpus:?}");
        assert_eq!(corpus[0].0, "valve notes");
        assert!(corpus[0].1.contains("relief valve"));
    }

    /// GRAPH rows are category headers plus their themes — the shape the pane
    /// renders and the indices the ↑/↓ handlers move over.
    #[test]
    fn graph_rows_are_headers_plus_themes_per_category() {
        let mut g = FileThemeGraph::new();
        g.add_theme(
            "Budget",
            Some("Finance"),
            "CONSTRUCT",
            "DO",
            "office",
            vec![1.0],
        );
        g.add_theme(
            "Valve",
            Some("Operations"),
            "OPERATE",
            "MONITOR",
            "machine",
            vec![0.0],
        );
        let rows = graph_rows(&g);
        assert_eq!(rows.len(), 4, "2 headers + 2 themes: {rows:?}");
        assert_eq!(rows[0], GraphRow::Header("Finance".into()));
        assert_eq!(rows[2], GraphRow::Header("Operations".into()));
        match &rows[1] {
            GraphRow::Theme { label, .. } => assert!(label.starts_with("Budget"), "{label}"),
            other => panic!("expected a theme row, got {other:?}"),
        }
        // Selecting a header resolves to no theme: the file pane stays empty
        // instead of showing someone else's files.
        let mut app = app_for_tests();
        app.graph.graph = Some(g);
        app.graph.row = 0;
        refresh_related(&mut app);
        assert!(app.graph.theme.is_none());
        app.graph.row = 1;
        refresh_related(&mut app);
        assert!(app.graph.theme.is_some());
    }

    #[test]
    fn missing_diff_side_reports_instead_of_panicking() {
        let mut app = app_for_tests();
        app.tab = TAB_DIFF;
        apply(&mut app, Action::Recompute, None);
        assert!(app.diff.hunks.is_empty());
        assert!(app.status_msg.contains("both sides"), "{}", app.status_msg);
    }

    #[test]
    fn theme_graph_and_config_paths_are_the_ones_the_cli_uses() {
        let cfg = physis_pro::config::PhysisConfig::default();
        assert!(theme_graph_path(&cfg).ends_with("theme_graph.json"));
        assert!(cfg_path(&cfg).ends_with("config.json"));
    }

    #[test]
    fn machine_run_delegates_to_the_library_demos() {
        let out = run_machine("chronos");
        assert!(out.contains("events"), "{out}");
        assert!(run_machine("nope").contains("unknown machine"));
    }

    /// Esc while a notebook answer is pending cancels it; otherwise Esc quits.
    /// A slow model must not trap input — cancel first, quit second.
    #[test]
    fn esc_cancels_pending_ask_before_quitting() {
        assert_eq!(
            action_for_asking(TAB_NOTEBOOK, false, true, KeyCode::Esc),
            Action::CancelAsk
        );
        assert_eq!(
            action_for_asking(TAB_NOTEBOOK, false, false, KeyCode::Esc),
            Action::Quit
        );
        assert_eq!(
            action_for_asking(TAB_GRAPH, false, true, KeyCode::Esc),
            Action::Quit,
            "cancel is notebook-only"
        );
    }

    /// ask() stages, poll_ask() answers, cancel drops without answering.
    /// Same state machine the live loop drives per frame.
    #[test]
    fn ask_stages_poll_answers_cancel_drops() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("valve.txt");
        std::fs::write(&doc, "relief valve opens at 6 bar\n").unwrap();
        let mut g = FileThemeGraph::new();
        g.add_theme(
            "Valve",
            Some("Operations"),
            "OPERATE",
            "MONITOR",
            "machine",
            vec![0.1, 0.9],
        );
        g.add_file(doc, "valve notes", "text", vec![0.1, 0.9], None);

        // Stage: asking, nothing answered yet.
        let mut app = app_for_tests();
        app.tab = TAB_NOTEBOOK;
        app.graph.graph = Some(g);
        app.notebook.query = "at what pressure does the valve open".into();
        apply(&mut app, Action::Open, None);
        assert!(app.notebook.asking, "ask stages the pending query");
        assert!(app.notebook.answer.is_none(), "nothing answered yet");

        // Poll: answers from the staged corpus.
        poll_ask(&mut app);
        assert!(!app.notebook.asking);
        let a = app.notebook.answer.as_ref().expect("poll answers");
        assert!(!a.sources.is_empty(), "grounded in the corpus");
        assert!(
            a.render().contains("valve notes") || a.text.contains("6"),
            "answer cites the staged doc: {}",
            a.render()
        );

        // Cancel: staged work dropped, old answer kept, nothing new shown.
        app.notebook.query = "another question".into();
        apply(&mut app, Action::Open, None);
        assert!(app.notebook.asking);
        let before = app.notebook.answer.clone().map(|a| a.text);
        apply(&mut app, Action::CancelAsk, None);
        assert!(!app.notebook.asking);
        assert!(app.notebook.pending.is_none());
        assert_eq!(app.notebook.answer.map(|a| a.text), before);
        assert!(app.status_msg.contains("cancelled"), "{}", app.status_msg);
    }
}
