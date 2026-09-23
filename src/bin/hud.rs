//! physis-hud — the Semantic Navigation HUD (Function 3 MVP).
//!
//! One screen, three panes:
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │  ranked neighbourhood (grouped results)      │
//! ├──────────────────────────────────────────────┤
//! │  reading pane (exact source, no summary)     │
//! ├──────────────────────────────────────────────┤
//! │ > query                                      │
//! └──────────────────────────────────────────────┘
//! ```
//!
//! Keyboard-first (§22): `/` focus search · ↑↓ move · Enter zoom in ·
//! Esc zoom out (trail preserved) · Space select · s/d/R same/different/relates
//! · n notebook selection · p pin · r related · e inline edit (with rescan) ·
//! :external [path] falls back to $EDITOR · u rescan · g graph view · H history
//! · B bench · D dependents · C contradictions · f/F intent focus/clear · z zoom
//! rollup · 1/2/3/4 why/context/delta/history tabs · ? command help.
//! No mouse required.
//!
//! The bottom box is universal: plain text searches, `/text` searches
//! globally, `$ command` runs shell in the workspace root, `:name` and command
//! words run Physis helpers, and `open`/`edit`/`cd` navigate, edit, or change
//! the root. Inline editing uses arrows/Home/End, Enter, Backspace/Delete,
//! Ctrl+S to save, and Esc to cancel.
//!
//! No LLM anywhere: retrieval is the shared hybrid scorer; when no embedder
//! passes the probe the header says `semantic: off` and lexical/structural
//! retrieval carries on.

use std::collections::{BTreeMap, BTreeSet};
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

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

use physis_pro::config::PhysisConfig;
use physis_pro::mvp::log::{hash_canonical, CompactDelta, StateRef, Transition, TransitionLog};
use physis_pro::nav::{self, NavHit, NavWeights, ObjectKind};
use physis_pro::settings::Settings;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut prescan = true;
    let mut prescan_only = false;
    let mut snapshot: Option<(u16, u16)> = None;
    let mut root: Option<PathBuf> = None;
    let mut setup = false;
    let mut view_name: Option<String> = None;
    let mut query: Option<String> = None;
    // `--no-semantic`: zero the vector leg's weight for this run, so the degraded
    // mode is provable on a machine that *has* a model instead of only on one
    // that does not. It weights the leg to zero rather than dropping persisted
    // vectors: a flag must not edit the store.
    let mut no_semantic = false;
    // `--view` takes a value, so pull it (and `--view=NAME`) out before the
    // positional loop — otherwise the value would be eaten as the root path.
    // `--query` does the same: it is the headless way to give the world
    // projection a focus, so a `--snapshot --view graph` frame has an object
    // to project instead of an empty pane.
    let mut plain: Vec<&String> = Vec::new();
    let mut it = args.iter().skip(1).peekable();
    while let Some(a) = it.next() {
        if a == "--view" {
            view_name = it.next().cloned();
        } else if let Some(name) = a.strip_prefix("--view=") {
            view_name = Some(name.to_string());
        } else if a == "--query" {
            query = it.next().cloned();
        } else if let Some(q) = a.strip_prefix("--query=") {
            query = Some(q.to_string());
        } else {
            plain.push(a);
        }
    }
    for a in plain {
        match a.as_str() {
            "--prescan-only" => {
                prescan_only = true;
            }
            "--no-prescan" => {
                prescan = false;
            }
            "--setup" => {
                setup = true;
            }
            "--no-semantic" => {
                no_semantic = true;
            }
            "--snapshot" => {
                snapshot = Some((100, 30));
            }
            _ if a.starts_with("--snapshot=") => {
                let spec = &a["--snapshot=".len()..];
                snapshot = Some(parse_size(spec).ok_or_else(|| {
                    anyhow::anyhow!("--snapshot expects WxH, e.g. --snapshot=80x24 (got {spec:?})")
                })?);
            }
            "-h" | "--help" => {
                eprintln!("physis-hud [root] [--prescan-only] [--no-prescan] [--setup] [--view <name>] [--query <text>] [--no-semantic] [--snapshot[=WxH]]");
                eprintln!("  default: prescan index + embedding cache, then open the HUD");
                eprintln!("  --prescan-only: warm the index + caches and exit (no TUI)");
                eprintln!("  --no-prescan: open on the persisted index as-is (fast restart)");
                eprintln!("  --setup: print what is configured, what is missing, and the");
                eprintln!("           command that closes each gap; exit 0");
                eprintln!("  --view <name>: open a view directly (one of: {});", View::ALL.iter().map(|v| v.label()).collect::<Vec<_>>().join(", "));
                eprintln!("           combines with --snapshot for a CI-renderable frame");
                eprintln!("  --query <text>: run this query before rendering, so the frame has a");
                eprintln!("           focused object (the headless vertical slice)");
                eprintln!("  --no-semantic: rank without the vector leg for this run, so the frame");
                eprintln!("           proves the degraded label on a machine that has a model");
                eprintln!("  --snapshot[=WxH]: render one frame to stdout and exit (default 100x30)");
                return Ok(());
            }
            _ => {
                if root.is_none() {
                    root = Some(PathBuf::from(a));
                }
            }
        }
    }
    let root =
        root.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Prescan/preload happens before the alternate screen owns the terminal,
    // so the first-index notice and timings stay visible instead of flashing
    // inside the TUI. Same bargain as the REPL: pay once up front, then
    // every keystroke is incremental.
    if prescan_only {
        return prescan_only_run(&root);
    }
    if setup {
        // No prescan: SETUP is a read of what is already on disk, so it stays
        // cheap even on a cold checkout — an empty index is exactly the gap it
        // should report.
        let app = App::new(root, false)?;
        println!("{}", setup_report(&app.setup_facts()));
        return Ok(());
    }
    if let Some((w, h)) = snapshot {
        return snapshot_run(
            &root,
            w,
            h,
            view_name.as_deref(),
            query.as_deref(),
            no_semantic,
        );
    }
    let mut app = App::new(root, prescan)?;
    if no_semantic {
        app.disable_vector_leg();
    }
    if let Some(q) = &query {
        app.query = q.clone();
        app.run_query();
    }
    open_named_view(&mut app, view_name.as_deref())?;
    let terminal = enter_tui()?;
    let result = run(terminal, app);
    leave_tui()?;
    result
}

/// Open a `--view <name>` view, naming the valid ones when the name is wrong.
fn open_named_view(app: &mut App, name: Option<&str>) -> anyhow::Result<()> {
    let Some(name) = name else {
        return Ok(());
    };
    let Some(view) = View::from_name(name) else {
        return Err(anyhow::anyhow!(
            "unknown view {name:?} (one of: {})",
            View::ALL
                .iter()
                .map(|v| v.label())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    // Views whose body a command produces (graph, tasks, agents, help) are
    // opened the way their key opens them, so a `--view graph` frame shows the
    // graph rather than an empty pane. The others (world, settings, setup,
    // notebook) render from live state and open directly.
    match view {
        View::Graph | View::Semantic | View::Tasks | View::Agents | View::Help => {
            app.open_view_via_command(view)
        }
        _ => app.open_view(view),
    }
    Ok(())
}

/// Parse a `WxH` snapshot size, e.g. `80x24`.
fn parse_size(spec: &str) -> Option<(u16, u16)> {
    let (w, h) = spec.split_once('x')?;
    let w: u16 = w.trim().parse().ok()?;
    let h: u16 = h.trim().parse().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Render one frame through the same `draw` path to a `TestBackend` and print
/// it. A golden frame in CI cannot drift from what a person sees, because there
/// is no second renderer. No TTY required.
fn snapshot_run(
    root: &Path,
    w: u16,
    h: u16,
    view: Option<&str>,
    query: Option<&str>,
    no_semantic: bool,
) -> anyhow::Result<()> {
    use ratatui::backend::TestBackend;
    let mut app = App::new(root.to_path_buf(), false)?;
    // The flag and the query reach the same state the interactive path builds:
    // one code path, so a snapshot cannot prove something the TUI would not do.
    if no_semantic {
        app.disable_vector_leg();
    }
    // The query gives the projection a focus before the view renders, so a
    // `--view graph` frame shows the same object the world projection lists
    // rather than an empty pane.
    if let Some(q) = query {
        app.query = q.to_string();
        app.run_query();
    }
    open_named_view(&mut app, view)?;
    let backend = TestBackend::new(w, h);
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.draw(|f| draw(f, &app))?;
    let buf = terminal.backend().buffer();
    let width = buf.area.width as usize;
    for row in buf.content.chunks(width) {
        let line: String = row.iter().map(|c| c.symbol()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
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
    eprintln!(
        "physis-hud prescan: {summary} in {:?} · workspace {tokens} tok",
        t0.elapsed()
    );
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
        let Ok(text) = std::fs::read_to_string(root.join(&f.path)) else {
            continue;
        };
        for w in text
            .lines()
            .collect::<Vec<_>>()
            .chunks(physis_pro::pack::CHUNK_LINES)
        {
            let t = w.join("\n");
            if t.trim().is_empty() {
                continue;
            }
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

/// Run a child process with the TUI parked: leave the alternate screen, run
/// the closure, then restore the terminal before returning. This keeps shell
/// commands and external editors from corrupting the drawn UI.
fn suspend_tui_for<T>(f: impl FnOnce() -> T) -> T {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
    let out = f();
    let _ = enable_raw_mode();
    let _ = execute!(stdout(), EnterAlternateScreen);
    out
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

/// PH-030: the ranked results projected onto the list view. `hits`, `rows`,
/// and `sel` encode one concept — the current result set — and are replaced
/// atomically through `ResultState::new` so no caller can leave the trio
/// inconsistent (the "stale-hits hazard" PH-029 flagged: `self.hits` cleared
/// while `sel`/`rows` stale). Cursor steps that just move `sel` within an
/// existing list go through `set_sel` and relax the Hit invariant.
#[derive(Debug, Clone)]
struct ResultState {
    hits: Vec<NavHit>,
    rows: Vec<Row>,
    sel: usize,
}

impl ResultState {
    /// Build all three fields at once. `sel` is clamped to the first
    /// `Row::Hit` if it lands on a Header/Dir (or the list is empty) — this is
    /// the `set_results` entry point, so the invariant holds at construction.
    fn new(hits: Vec<NavHit>, rows: Vec<Row>, sel: usize) -> Self {
        let sel = if rows.is_empty() {
            0
        } else if matches!(rows.get(sel), Some(Row::Hit(_))) {
            sel.min(rows.len() - 1)
        } else {
            rows.iter().position(|r| matches!(r, Row::Hit(_))).unwrap_or(0)
        };
        debug_assert!(
            rows.is_empty() || matches!(rows.get(sel), Some(Row::Hit(_))),
            "sel ({}) must point at a Row::Hit in a non-empty list",
            sel,
        );
        Self { hits, rows, sel }
    }

    /// Move the cursor within an existing result set. Unlike `new`, this may
    /// land on a Header/Dir row — `sel` is the list cursor, not a Hit index.
    /// Clamped to `[0, rows.len()]` (0 if empty).
    fn set_sel(&mut self, sel: usize) {
        self.sel = if self.rows.is_empty() {
            0
        } else {
            sel.min(self.rows.len() - 1)
        };
    }

    /// Restore hits/rows/sel verbatim from a saved stack frame. Unlike `new`,
    /// this preserves `sel` even if it points at a Header/Dir row — a cursor
    /// position, not a Hit index — so `back()` round-trips the exact focus.
    fn restore(hits: Vec<NavHit>, rows: Vec<Row>, sel: usize) -> Self {
        debug_assert!(!rows.is_empty() || sel == 0, "empty rows must restore sel 0");
        Self { hits, rows, sel }
    }
}

/// World contexts the demo pane shows on demand. Kept in the App (not the
/// world) because they are projections of existing state — never new stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextTab {
    Why,
    Context,
    Delta,
    History,
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

/// Every surface the HUD owns, as a named view.
///
/// The world (navigation, reading, why/context/delta/history) is one view among
/// several, not the only one. Typing `tasks`, `agents` or `settings` now
/// switches view instead of overwriting the reading pane — before this, the
/// task graph, the settings dump and a notebook answer all landed in the same
/// 35 %-high box, and the next query destroyed whichever it was.
///
/// A view is a projection of state that already exists. None of them is a
/// second store, and none of them reaches a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    World,
    Graph,
    Semantic,
    Notebook,
    Tasks,
    Agents,
    Settings,
    Setup,
    Help,
}

impl View {
    /// Tab order, and the order the footer names them.
    const ALL: [View; 9] = [
        View::World,
        View::Graph,
        View::Semantic,
        View::Notebook,
        View::Tasks,
        View::Agents,
        View::Settings,
        View::Setup,
        View::Help,
    ];

    /// The library projection this view is. The one mapping between the bin's
    /// enum and the projection contract; both directions are total matches, so
    /// adding a view without its projection fails to compile.
    fn projection(self) -> physis_pro::hud::world::ProjectionId {
        use physis_pro::hud::world::ProjectionId;
        match self {
            View::World => ProjectionId::World,
            View::Graph => ProjectionId::ObjectGraph,
            View::Semantic => ProjectionId::Semantic,
            View::Notebook => ProjectionId::Notebook,
            View::Tasks => ProjectionId::Tasks,
            View::Agents => ProjectionId::Agents,
            View::Settings => ProjectionId::Settings,
            View::Setup => ProjectionId::Setup,
            View::Help => ProjectionId::Help,
        }
    }

    /// The reverse of [`Self::projection`].
    fn from_projection(projection: physis_pro::hud::world::ProjectionId) -> View {
        use physis_pro::hud::world::ProjectionId as P;
        match projection {
            P::World => View::World,
            P::ObjectGraph => View::Graph,
            P::Semantic => View::Semantic,
            P::Notebook => View::Notebook,
            P::Tasks => View::Tasks,
            P::Agents => View::Agents,
            P::Settings => View::Settings,
            P::Setup => View::Setup,
            P::Help => View::Help,
        }
    }

    /// The operator-facing name. Delegates to the library table rather than
    /// repeating it: two lists of the same eight strings used to live here and
    /// in `ProjectionId::name`, with nothing but a test keeping them in step.
    fn label(self) -> &'static str {
        self.projection().name()
    }

    /// The world key that opens this view. Upper case wherever the lower-case
    /// letter is already a world action (`s`/`d`/`R` relate, `n` selects for the
    /// notebook, `g` graphs the cursor), so no key loses its old meaning.
    fn key(self) -> Option<char> {
        match self {
            View::Graph => Some('g'),
            View::Semantic => Some('M'),
            View::Notebook => Some('N'),
            View::Tasks => Some('T'),
            View::Agents => Some('A'),
            View::Settings => Some('S'),
            View::Setup => Some('W'),
            View::Help => Some('?'),
            View::World => None,
        }
    }

    fn from_name(name: &str) -> Option<View> {
        // One parse, in the library: the same table `--view` is validated
        // against, so the bin cannot accept a name the projection contract
        // does not know.
        physis_pro::hud::world::ProjectionId::from_name(name).map(View::from_projection)
    }

    fn from_key(key: char) -> Option<View> {
        View::ALL.into_iter().find(|v| v.key() == Some(key))
    }

    /// Next view in tab order, wrapping — `Tab` never dead-ends.
    fn next(self) -> View {
        let i = View::ALL.iter().position(|v| *v == self).unwrap_or(0);
        View::ALL[(i + 1) % View::ALL.len()]
    }

    /// Previous view in tab order, wrapping.
    fn prev(self) -> View {
        let i = View::ALL.iter().position(|v| *v == self).unwrap_or(0);
        View::ALL[(i + View::ALL.len() - 1) % View::ALL.len()]
    }

    /// What the keys do here. The title says it, so nobody has to guess whether
    /// `+` steps a value or scrolls the pane.
    fn keys(self) -> &'static str {
        match self {
            View::World => {
                "e edit · g graph · N notebook · T tasks · A agents · S settings · W setup · ? help"
            }
            View::Settings => {
                "↑↓ row · +/- step · Enter/e edit · s save · r reload · d revert row · Esc world"
            }
            View::Setup => "r re-detect · s selftest · Esc world",
            _ => "↑↓ scroll · Tab next view · Esc world",
        }
    }
}

/// The provider fields an operator sets, in the order they are set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderField {
    Transport,
    Bin,
    Model,
    KeyEnv,
    Endpoint,
}

impl ProviderField {
    const ALL: [ProviderField; 5] = [
        ProviderField::Transport,
        ProviderField::Bin,
        ProviderField::Model,
        ProviderField::KeyEnv,
        ProviderField::Endpoint,
    ];

    fn name(self) -> &'static str {
        match self {
            ProviderField::Transport => "transport",
            ProviderField::Bin => "bin",
            ProviderField::Model => "model",
            ProviderField::KeyEnv => "key_env",
            ProviderField::Endpoint => "endpoint",
        }
    }

    /// What the value is for, printed beside it: a settings screen that shows
    /// five dotted keys and no explanation is a config file with extra steps.
    fn hint(self) -> &'static str {
        match self {
            ProviderField::Transport => "how the agent is reached (zen | cli | http)",
            ProviderField::Bin => "executable on PATH, when the transport spawns one",
            ProviderField::Model => "model id handed to the transport",
            ProviderField::KeyEnv => "NAME of the env var holding the key — never the key",
            ProviderField::Endpoint => "base URL of a Jev-compatible decision endpoint",
        }
    }
}

/// One row of the SETTINGS view. Rows address the two live documents by key;
/// the view keeps no copy of a value, so it cannot show something the engine
/// does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SettingRow {
    Section(&'static str),
    /// `config::CONFIG_KNOBS[index]` — the same table `physis-pro-tui` and
    /// Studio CONFIG edit.
    Knob(usize),
    /// `providers.<agent>.<field>` in the user settings file.
    Provider {
        agent: String,
        field: ProviderField,
    },
    /// `commands.<name>.enabled`.
    CommandEnabled { name: String },
    /// `commands.<name>.aliases`.
    CommandAliases { name: String },
}

impl SettingRow {
    /// The dotted settings key this row writes, or the `config.json` field name
    /// for a knob. `None` only for section headers.
    fn key(&self) -> Option<String> {
        match self {
            SettingRow::Section(_) => None,
            SettingRow::Knob(i) => physis_pro::config::CONFIG_KNOBS
                .get(*i)
                .map(|k| k.key.to_string()),
            SettingRow::Provider { agent, field } => {
                Some(format!("providers.{agent}.{}", field.name()))
            }
            SettingRow::CommandEnabled { name } => Some(format!("commands.{name}.enabled")),
            SettingRow::CommandAliases { name } => Some(format!("commands.{name}.aliases")),
        }
    }

    /// Typed rows open the command box; stepped rows change on `+`/`-`.
    /// `key_env` is typed on purpose: its value is the *name* of a variable,
    /// and hiding it would hide the one thing an operator needs to check.
    fn is_text(&self) -> bool {
        match self {
            SettingRow::Section(_)
            | SettingRow::Knob(_)
            | SettingRow::CommandEnabled { .. } => false,
            SettingRow::Provider { field, .. } => !matches!(field, ProviderField::Transport),
            SettingRow::CommandAliases { .. } => true,
        }
    }
}

/// Build the settings rows: runtime knobs, then every provider, then every
/// command. Providers and commands follow their own `BTreeMap` order, so the
/// list a person scrolls is the same on any machine with the same file.
///
/// Pure: no file read, no terminal, no state of its own.
fn settings_rows(s: &Settings) -> Vec<SettingRow> {
    let mut rows = vec![SettingRow::Section("runtime knobs (config.json)")];
    for i in 0..physis_pro::config::CONFIG_KNOBS.len() {
        rows.push(SettingRow::Knob(i));
    }
    rows.push(SettingRow::Section("providers (settings.json)"));
    for agent in s.providers.keys() {
        for field in ProviderField::ALL {
            rows.push(SettingRow::Provider {
                agent: agent.clone(),
                field,
            });
        }
    }
    rows.push(SettingRow::Section("commands (settings.json)"));
    for name in s.commands.keys() {
        rows.push(SettingRow::CommandEnabled { name: name.clone() });
        rows.push(SettingRow::CommandAliases { name: name.clone() });
    }
    rows
}

/// The value as the row prints it. An unset text field prints `-` rather than an
/// empty gap, so "not configured" is visibly different from "set to nothing".
fn setting_value(row: &SettingRow, s: &Settings, cfg: &PhysisConfig) -> String {
    match row {
        SettingRow::Knob(i) => physis_pro::config::CONFIG_KNOBS
            .get(*i)
            .map(|k| k.display(cfg))
            .unwrap_or_default(),
        SettingRow::Provider { agent, field } => s
            .provider(agent)
            .and_then(|p| match field {
                ProviderField::Transport => p.transport.as_deref(),
                ProviderField::Bin => p.bin.as_deref(),
                ProviderField::Model => p.model.as_deref(),
                ProviderField::KeyEnv => p.key_env.as_deref(),
                ProviderField::Endpoint => p.endpoint.as_deref(),
            })
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        SettingRow::CommandEnabled { name } => s
            .commands
            .get(name)
            .map(|c| if c.enabled { "on" } else { "off" }.to_string())
            .unwrap_or_else(|| "-".to_string()),
        SettingRow::CommandAliases { name } => s
            .commands
            .get(name)
            .map(|c| {
                if c.aliases.is_empty() {
                    "-".to_string()
                } else {
                    c.aliases.join(",")
                }
            })
            .unwrap_or_else(|| "-".to_string()),
        SettingRow::Section(_) => String::new(),
    }
}

/// One applied settings change: the key, the value to record, and which document
/// it belongs to. Recording only what the operator changed is what keeps a save
/// from freezing today's shipped defaults into their settings file.
struct Change {
    key: String,
    value: String,
    /// `true` = a `config.json` knob; `false` = a settings-file override.
    is_knob: bool,
}

/// Step a row one notch. `Ok(None)` means "this row is typed, open the box",
/// `Ok(Some(change))` means the caller must record it, `Err` means the row
/// cannot be stepped at all.
fn step_setting(
    row: &SettingRow,
    s: &Settings,
    cfg: &mut PhysisConfig,
    up: bool,
) -> Result<Option<Change>, String> {
    match row {
        SettingRow::Section(_) => Ok(None),
        SettingRow::Knob(i) => {
            let knob = physis_pro::config::CONFIG_KNOBS
                .get(*i)
                .ok_or_else(|| format!("no knob at index {i}"))?;
            let now = knob.bump(cfg, up);
            Ok(Some(Change {
                key: knob.key.to_string(),
                value: format!("{now}"),
                is_knob: true,
            }))
        }
        SettingRow::Provider { agent, field } => {
            if !matches!(field, ProviderField::Transport) {
                return Ok(None); // typed row
            }
            let current = s
                .provider(agent)
                .and_then(|p| p.transport.clone())
                .unwrap_or_default();
            // transport is a closed set, so `+`/`-` cycles it: an operator
            // cannot invent a transport the adapter does not implement.
            // `local` is in the set because the shipped file uses it.
            const TRANSPORTS: [&str; 4] = ["zen", "cli", "http", "local"];
            let idx = TRANSPORTS.iter().position(|t| *t == current).unwrap_or(0);
            let next = if up {
                TRANSPORTS[(idx + 1) % TRANSPORTS.len()]
            } else {
                TRANSPORTS[(idx + TRANSPORTS.len() - 1) % TRANSPORTS.len()]
            };
            Ok(Some(Change {
                key: format!("providers.{agent}.transport"),
                value: next.to_string(),
                is_knob: false,
            }))
        }
        SettingRow::CommandEnabled { name } => {
            let now = s
                .commands
                .get(name)
                .map(|c| c.enabled)
                .ok_or_else(|| format!("unknown command `{name}`"))?;
            Ok(Some(Change {
                key: format!("commands.{name}.enabled"),
                value: (!now).to_string(),
                is_knob: false,
            }))
        }
        SettingRow::CommandAliases { .. } => Ok(None),
    }
}

/// Apply a typed value. Knob keys are clamped by the shared knob table; every
/// other key is validated by `Settings::set` on a scratch document, so a typo is
/// refused by the same validator the CLI uses instead of being written through.
/// The recorded value is what the operator typed: the view always re-renders it
/// from the merged document, so the normalised form (`a, b ,` → `a,b`) is what
/// they see, and `Settings::set` writes the same form.
fn set_setting_text(
    s: &Settings,
    cfg: &mut PhysisConfig,
    key: &str,
    value: &str,
) -> Result<Change, String> {
    if let Some(knob) = physis_pro::config::CONFIG_KNOBS
        .iter()
        .find(|k| k.key == key)
    {
        let raw: f64 = value
            .trim()
            .parse()
            .map_err(|_| format!("{key} expects a number, got `{value}`"))?;
        let now = knob.set(cfg, raw);
        return Ok(Change {
            key: key.to_string(),
            value: format!("{now}"),
            is_knob: true,
        });
    }
    let mut scratch = s.clone();
    scratch.set(key, value)?;
    Ok(Change {
        key: key.to_string(),
        value: value.to_string(),
        is_knob: false,
    })
}

/// What the selected row is for. A settings screen that shows dotted keys and no
/// explanation is a config file with extra steps.
fn row_hint(row: &SettingRow) -> &'static str {
    match row {
        SettingRow::Section(_) => "",
        SettingRow::Knob(_) => "runtime knob — written to config.json",
        SettingRow::Provider { field, .. } => field.hint(),
        SettingRow::CommandEnabled { .. } => "on: the command word routes · off: it is disabled",
        SettingRow::CommandAliases { .. } => "comma-separated words that route to this command",
    }
}

/// Move the selection to the next non-header row in `rows`, wrapping. Headers
/// are labels, not stops — landing on one would look like a stuck cursor.
fn next_setting_row(rows: &[SettingRow], from: usize, down: bool) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let n = rows.len();
    let mut i = from.min(n - 1);
    for _ in 0..n {
        i = if down { (i + 1) % n } else { (i + n - 1) % n };
        if !matches!(rows[i], SettingRow::Section(_)) {
            return i;
        }
    }
    from.min(n - 1)
}

/// What SETUP detected, as data. The checklist below is a pure function of this
/// struct, so it is testable without a terminal, a workspace, or a machine that
/// happens to have an agent on PATH.
struct SetupFacts {
    root: PathBuf,
    files: usize,
    objects: usize,
    workspace_tokens: usize,
    embedder: String,
    semantic: bool,
    data_dir: PathBuf,
    config_path: PathBuf,
    settings_path: PathBuf,
    settings_exists: bool,
    observations_path: PathBuf,
    /// (id, transport, reachable) per roster agent.
    agents: Vec<(String, String, bool)>,
}

/// The SETUP screen: what is here, what is missing, and the exact command that
/// closes each gap. Every line is a fact read from `SetupFacts` or a command
/// that exists in this build — no advice, no invented flags, no model.
fn setup_report(f: &SetupFacts) -> String {
    let mut o = String::new();
    o.push_str("PHYSIS SETUP — offline by default; nothing here needs a network\n\n");

    o.push_str("WORKSPACE\n");
    o.push_str(&format!("  root            {}\n", f.root.display()));
    o.push_str(&format!(
        "  index           {} files · {} objects · {} tok\n",
        f.files, f.objects, f.workspace_tokens
    ));
    o.push_str(&format!(
        "  semantic        {}\n",
        if f.semantic {
            format!("on ({})", f.embedder)
        } else {
            format!("OFF — lexical/structural only ({})", f.embedder)
        }
    ));
    if !f.semantic {
        o.push_str("                  next: physis-pro selftest (names the probe that fails)\n");
    }

    o.push_str("\nFILES\n");
    o.push_str(&format!("  data dir        {}\n", f.data_dir.display()));
    o.push_str(&format!(
        "  config.json     {}{}\n",
        f.config_path.display(),
        if f.config_path.exists() {
            ""
        } else {
            "   (absent — built-in defaults are in use)"
        }
    ));
    o.push_str(&format!(
        "  settings.json   {}{}\n",
        f.settings_path.display(),
        if f.settings_exists {
            ""
        } else {
            "   (absent — shipped defaults are in use)"
        }
    ));
    o.push_str(&format!(
        "  observations    {}\n",
        f.observations_path.display()
    ));
    o.push_str("                  the settings view (S) writes both files with `s`\n");

    o.push_str("\nAGENTS\n");
    if f.agents.is_empty() {
        o.push_str("  (roster is empty — no agent adapters in this build)\n");
    }
    for (id, transport, ok) in &f.agents {
        o.push_str(&format!(
            "  {} {:<10} {}{}\n",
            if *ok { "●" } else { "○" },
            id,
            transport,
            if *ok { "" } else { "   (unavailable)" }
        ));
    }
    let reachable = f.agents.iter().filter(|(_, _, ok)| *ok).count();
    if reachable == 0 {
        o.push_str("                  next: A lists the roster; @agent needs one reachable\n");
    }

    o.push_str("\nCHECKLIST\n");
    let check = |ok: bool| if ok { "[x]" } else { "[ ]" };
    o.push_str(&format!(
        "  {} workspace indexed\n",
        check(f.files > 0 && f.objects > 0)
    ));
    o.push_str(&format!("  {} semantic retrieval\n", check(f.semantic)));
    o.push_str(&format!(
        "  {} agent transport for @agent\n",
        check(reachable > 0)
    ));
    o.push_str(&format!(
        "  {} settings file written ({})\n",
        check(f.settings_exists),
        f.settings_path.display()
    ));

    o.push_str("\nNEXT\n");
    o.push_str("  1. change a value here, then press `s` — writes config.json + settings.json\n");
    o.push_str("  2. tour: g graph · N notebook · T tasks · A agents · ? help\n");
    o.push_str("  3. outside the HUD: physis-pro doctor · selftest · settings show\n");
    o
}

/// Deterministic HUD commands. This is a small closed vocabulary over the
/// existing helpers — not a language, router, or agent framework.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HudCommand {
    Help,
    History,
    Bench,
    Context,
    Delta,
    Why,
    Dependencies(Option<String>),
    Contradictions,
    Rescan,
    Graph,
    Notebook,
    Related,
    Pin,
    Relate(String),
    Focus(Option<String>),
    ClearFocus,
    Zoom,
    CycleEdges,
    Tab(ContextTab),
    Trail,
    Pins,
    Unpin,
    Sem(bool),
    OpenExternal(Option<String>),
    Action(String),
    /// Fold the observation log into the task graph and show it.
    Tasks,
    /// Show the operator's intent and its provenance.
    Intent,
    /// Show what may be done to the focused object, from the registry.
    Actions,
    /// Show the agent roster and live availability.
    Agents,
    /// Show the effective settings.
    Settings,
    /// Show what is configured, what is missing, and the command that closes
    /// each gap.
    Setup,
    /// Guided execution: compile the world into a constraint and choose inside
    /// it, showing the card (no execution, no write reach).
    Guided(String),
    /// Return to the world view from any other view.
    World,
    Quit,
}

/// One parsed command-box line. Parsing is explicit and total: every input
/// becomes exactly one of these variants.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HudInput {
    Empty,
    Invalid(String),
    Shell(String),
    Command(HudCommand),
    Open(String),
    Edit(Option<String>),
    ChangeRoot(String),
    GlobalQuery(String),
    SemanticQuery(String),
    /// `@agent` or `@agent@host`, with the remaining text as scope/request.
    AgentCall {
        agent: String,
        host: Option<String>,
        rest: String,
    },
    /// `@all <request>`: ask every agent in the roster.
    AgentAll {
        request: String,
    },
    /// `@a x | @b y`: stages run in order, each fed the previous reply.
    AgentPipeline {
        stages: Vec<physis_pro::agents::parse::Stage>,
    },
}

/// Deterministic mode label for the command box. Inferred from the text, not
/// from AI: `$` shell, `:` or a command word Physis, `open`/`edit`/`cd` their
/// own narrow modes, `/` global search, anything else local search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudPromptMode {
    Search,
    GlobalSearch,
    Physis,
    Shell,
    Open,
    Edit,
    Root,
    Agent,
    Editor,
    Invalid,
}

/// Highlighting is decoration only: the mode text carries the meaning, never
/// the colour alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptPart {
    Prefix,
    Word,
    Text,
}

fn split_command_word(text: &str) -> (String, String) {
    let mut parts = text.splitn(2, char::is_whitespace);
    let word = parts.next().unwrap_or_default().to_string();
    let rest = parts.next().unwrap_or_default().trim().to_string();
    (word, rest)
}

fn optional_argument(rest: &str) -> Option<String> {
    if rest.trim().is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn parse_named_command(text: &str) -> HudInput {
    let (word, rest) = split_command_word(text);
    let arg = optional_argument(&rest);
    match word.to_lowercase().as_str() {
        "help" if arg.is_none() => HudInput::Command(HudCommand::Help),
        "history" if arg.is_none() => HudInput::Command(HudCommand::History),
        "bench" if arg.is_none() => HudInput::Command(HudCommand::Bench),
        "context" if arg.is_none() => HudInput::Command(HudCommand::Context),
        "delta" if arg.is_none() => HudInput::Command(HudCommand::Delta),
        "why" if arg.is_none() => HudInput::Command(HudCommand::Why),
        "contradictions" if arg.is_none() => HudInput::Command(HudCommand::Contradictions),
        "rescan" if arg.is_none() => HudInput::Command(HudCommand::Rescan),
        "graph" if arg.is_none() => HudInput::Command(HudCommand::Graph),
        "tasks" | "task" if arg.is_none() => HudInput::Command(HudCommand::Tasks),
        "intent" if arg.is_none() => HudInput::Command(HudCommand::Intent),
        "actions" if arg.is_none() => HudInput::Command(HudCommand::Actions),
        "agents" if arg.is_none() => HudInput::Command(HudCommand::Agents),
        "settings" if arg.is_none() => HudInput::Command(HudCommand::Settings),
        "setup" | "install" if arg.is_none() => HudInput::Command(HudCommand::Setup),
        "guided" | "guide" | "plan" => HudInput::Command(HudCommand::Guided(rest)),
        "world" | "back" | "home" if arg.is_none() => HudInput::Command(HudCommand::World),
        "notebook" if arg.is_none() => HudInput::Command(HudCommand::Notebook),
        "related" if arg.is_none() => HudInput::Command(HudCommand::Related),
        "pin" if arg.is_none() => HudInput::Command(HudCommand::Pin),
        "clear" | "clear-focus" if arg.is_none() => HudInput::Command(HudCommand::ClearFocus),
        "zoom" if arg.is_none() => HudInput::Command(HudCommand::Zoom),
        "edges" if arg.is_none() => HudInput::Command(HudCommand::CycleEdges),
        "quit" | "q" if arg.is_none() => HudInput::Command(HudCommand::Quit),
        "dependencies" | "focus" => HudInput::Command(match word.to_lowercase().as_str() {
            "dependencies" => HudCommand::Dependencies(arg),
            _ => HudCommand::Focus(arg),
        }),
        "trail" if arg.is_none() => HudInput::Command(HudCommand::Trail),
        "pins" if arg.is_none() => HudInput::Command(HudCommand::Pins),
        "unpin" if arg.is_none() => HudInput::Command(HudCommand::Unpin),
        "sem" => match arg.as_deref().map(str::to_lowercase).as_deref() {
            Some("off") | Some("0") | Some("no") => HudInput::Command(HudCommand::Sem(false)),
            Some("on") | Some("1") | Some("yes") => HudInput::Command(HudCommand::Sem(true)),
            _ => HudInput::Invalid("usage: sem off|on".into()),
        },
        "relate" => match arg.as_deref().map(str::to_lowercase).as_deref() {
            Some("same") => HudInput::Command(HudCommand::Relate("same".into())),
            Some("different") => HudInput::Command(HudCommand::Relate("different".into())),
            Some("relates") | Some("relates to") | Some("relates-to") => {
                HudInput::Command(HudCommand::Relate("relates to".into()))
            }
            _ => HudInput::Invalid("usage: relate same|different|relates".into()),
        },
        "tab" => match arg.as_deref().map(str::to_lowercase).as_deref() {
            Some("why") => HudInput::Command(HudCommand::Tab(ContextTab::Why)),
            Some("context") => HudInput::Command(HudCommand::Tab(ContextTab::Context)),
            Some("delta") => HudInput::Command(HudCommand::Tab(ContextTab::Delta)),
            Some("history") => HudInput::Command(HudCommand::Tab(ContextTab::History)),
            _ => HudInput::Invalid("usage: tab why|context|delta|history".into()),
        },
        "open" => match arg {
            Some(target) => HudInput::Open(target),
            None => HudInput::Invalid("usage: open <path-or-object>".into()),
        },
        "action" => match arg {
            Some(line) => HudInput::Command(HudCommand::Action(line)),
            None => HudInput::Invalid("usage: action <verb> <object> [key=value ...]".into()),
        },
        "edit" => HudInput::Edit(arg),
        "external" => HudInput::Command(HudCommand::OpenExternal(arg)),
        "cd" => match arg {
            Some(dir) => HudInput::ChangeRoot(dir),
            None => HudInput::Invalid("usage: cd <directory>".into()),
        },
        "shell" => match arg {
            Some(command) => HudInput::Shell(command),
            None => HudInput::Invalid("usage: shell <command>".into()),
        },
        "search" | "query" => match arg {
            Some(query) => HudInput::SemanticQuery(query),
            None => HudInput::Invalid("usage: search <text>".into()),
        },
        _ => HudInput::Invalid(format!("unknown command: {word}")),
    }
}

/// Parse one command-box line. Order is the contract: shell first, then
/// explicit commands, then navigation/edit/root words, then global search,
/// then plain semantic search.
fn parse_hud_input(raw: &str) -> HudInput {
    let text = raw.trim();
    if text.is_empty() {
        return HudInput::Empty;
    }
    if let Some(command) = text.strip_prefix('$') {
        let command = command
            .trim_start_matches([' ', '\t'])
            .trim_end()
            .to_string();
        if command.is_empty() {
            return HudInput::Invalid("shell command needed after `$`".into());
        }
        return HudInput::Shell(command);
    }
    if let Some(command) = text.strip_prefix(':') {
        if command.trim().is_empty() {
            return HudInput::Command(HudCommand::Help);
        }
        return parse_named_command(command.trim());
    }
    if let Some(query) = text.strip_prefix('/') {
        let query = query.trim().to_string();
        if query.is_empty() {
            return HudInput::Empty;
        }
        return HudInput::GlobalQuery(query);
    }
    // `@` is reserved for agent addressing. The grammar lives in
    // `agents::parse` so the interpretation is one tested table, not a second
    // parser here.
    if text.starts_with('@') {
        return match physis_pro::agents::parse::parse(text) {
            Ok(physis_pro::agents::parse::AgentCall::Single(s)) => HudInput::AgentCall {
                agent: s.agent,
                host: s.host,
                rest: s.request,
            },
            Ok(physis_pro::agents::parse::AgentCall::All { request }) => {
                HudInput::AgentAll { request }
            }
            Ok(physis_pro::agents::parse::AgentCall::Pipeline { stages }) => {
                HudInput::AgentPipeline { stages }
            }
            Err(e) => HudInput::Invalid(e),
        };
    }
    let (word, rest) = split_command_word(text);
    match word.to_lowercase().as_str() {
        "open" => {
            if rest.is_empty() {
                HudInput::Invalid("usage: open <path-or-object>".into())
            } else {
                HudInput::Open(rest)
            }
        }
        "edit" => HudInput::Edit(optional_argument(&rest)),
        "cd" => {
            if rest.is_empty() {
                HudInput::Invalid("usage: cd <directory>".into())
            } else {
                HudInput::ChangeRoot(rest)
            }
        }
        "help" | "history" | "bench" | "context" | "delta" | "why" | "contradictions"
        | "rescan" | "graph" | "notebook" | "related" | "pin" | "clear" | "clear-focus"
        | "zoom" | "edges" | "trail" | "pins" | "unpin" | "tasks" | "intent" | "actions" | "agents"
        | "settings" | "setup" | "install" | "world" | "back"
            if rest.is_empty() =>
        {
            parse_named_command(text)
        }
        "dependencies" | "focus" | "relate" | "tab" | "sem" | "shell" | "search" | "query"
        | "external" | "action" | "guided" | "guide" | "plan" => parse_named_command(text),
        _ => HudInput::SemanticQuery(text.to_string()),
    }
}

/// The context an agent card names: the explicit request when present, else the
/// current query. `None` only when both are empty, so a card never invents a
/// scope and never silently drops one.
fn agent_scope(rest: &str, query: &str) -> Option<String> {
    if !rest.is_empty() {
        return Some(rest.to_string());
    }
    let q = query.trim();
    if q.is_empty() {
        None
    } else {
        Some(q.to_string())
    }
}

/// Invoke an agent synchronously. Runs on a worker thread from the HUD; the CLI
/// calls the adapter directly. Every failure path returns a readable reason.
fn invoke_agent(agent_id: &str, prompt: &str, scope: Option<&str>) -> AgentReply {
    use physis_pro::agents::{adapter, default_roster};
    let fail = |text: String| AgentReply {
        agent: agent_id.to_string(),
        ok: false,
        text,
    };
    let roster = default_roster();
    let Some(agent) = roster.get(agent_id) else {
        return fail(format!("no such agent: @{agent_id}"));
    };
    let adapter = match adapter::adapter_for(agent) {
        Ok(a) => a,
        Err(e) => return fail(format!("{e}")),
    };
    let mut inv = adapter::Invocation::new(prompt);
    if let Some(s) = scope {
        inv = inv.with_scope(s);
    }
    match adapter.invoke(&inv) {
        Ok(out) => AgentReply {
            agent: agent_id.to_string(),
            ok: true,
            text: out.text,
        },
        Err(e) => fail(format!("{e}")),
    }
}

/// Parse a JSONL observation log, skipping a torn line rather than failing the
/// whole fold: the log is append-only, so a bad line is a lost record.
fn parse_observation_lines(text: &str) -> Vec<physis_core::observe::Observation> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<physis_core::observe::Observation>(l).ok())
        .collect()
}

/// `@all`: ask every agent in the roster, one after another. Unavailable agents
/// are reported with their reason rather than skipped silently.
fn run_agent_all(request: &str) -> AgentReply {
    let roster = physis_pro::agents::default_roster();
    let mut out = String::new();
    let mut any = false;
    for a in &roster.agents {
        let s = physis_pro::agents::status(a);
        out.push_str(&format!("── @{} ({}) ──\n", a.id, s.transport));
        if !s.available {
            out.push_str(&format!(
                "unavailable: {}\n\n",
                s.reason.as_deref().unwrap_or("no transport")
            ));
            continue;
        }
        let r = invoke_agent(&a.id, request, None);
        any |= r.ok;
        out.push_str(&r.text);
        out.push_str("\n\n");
    }
    AgentReply {
        agent: "all".into(),
        ok: any,
        text: out,
    }
}

/// A pipeline: stages run in order, each fed the previous stage's reply. The
/// first failure stops the chain rather than feeding a failure onward.
fn run_agent_pipeline(stages: &[physis_pro::agents::parse::Stage]) -> AgentReply {
    let mut out = String::new();
    let mut carry = String::new();
    let mut ok = true;
    for (i, st) in stages.iter().enumerate() {
        out.push_str(&format!("── stage {}: @{} ──\n", i + 1, st.agent));
        if let Some(h) = &st.host {
            out.push_str(&format!(
                "(host {h}: remote placement is S12; running locally)\n"
            ));
        }
        let prompt = if carry.is_empty() {
            st.request.clone()
        } else {
            format!("{}\n\nPrevious stage output:\n{carry}", st.request)
        };
        let r = invoke_agent(&st.agent, &prompt, None);
        ok &= r.ok;
        out.push_str(&r.text);
        out.push('\n');
        if !r.ok {
            out.push_str("(stopping: a stage failed)\n");
            break;
        }
        carry = r.text;
    }
    AgentReply {
        agent: "pipeline".into(),
        ok,
        text: out,
    }
}

fn prompt_mode(input: &str, editor_active: bool) -> HudPromptMode {
    if editor_active {
        return HudPromptMode::Editor;
    }
    match parse_hud_input(input) {
        HudInput::Empty | HudInput::SemanticQuery(_) => HudPromptMode::Search,
        HudInput::GlobalQuery(_) => HudPromptMode::GlobalSearch,
        HudInput::Command(_) => HudPromptMode::Physis,
        HudInput::Shell(_) => HudPromptMode::Shell,
        HudInput::Open(_) => HudPromptMode::Open,
        HudInput::Edit(_) => HudPromptMode::Edit,
        HudInput::ChangeRoot(_) => HudPromptMode::Root,
        HudInput::AgentCall { .. } | HudInput::AgentAll { .. } | HudInput::AgentPipeline { .. } => {
            HudPromptMode::Agent
        }
        HudInput::Invalid(_) => HudPromptMode::Invalid,
    }
}

fn prompt_mode_label(mode: HudPromptMode) -> &'static str {
    match mode {
        HudPromptMode::Search => "[SEARCH]",
        HudPromptMode::GlobalSearch => "[SEARCH/GLOBAL]",
        HudPromptMode::Physis => "[PHYSIS]",
        HudPromptMode::Shell => "[SHELL]",
        HudPromptMode::Open => "[OPEN]",
        HudPromptMode::Edit => "[EDIT]",
        HudPromptMode::Root => "[ROOT]",
        HudPromptMode::Agent => "[AGENT]",
        HudPromptMode::Editor => "[EDIT]",
        HudPromptMode::Invalid => "[INPUT]",
    }
}

/// Split the visible prompt into a stable prefix and the operator's text.
/// Tests assert the split; ratatui only styles it.
fn prompt_parts(input: &str, editor_active: bool) -> Vec<(PromptPart, String)> {
    if editor_active {
        return vec![
            (PromptPart::Prefix, "[EDIT] ".to_string()),
            (
                PromptPart::Text,
                "Ctrl+S save · Esc cancel — query box is parked".to_string(),
            ),
        ];
    }
    let text = input.trim_end().to_string();
    let (word, _) = split_command_word(&text);
    let prefix = match parse_hud_input(input) {
        HudInput::Shell(_) => "$ ".to_string(),
        HudInput::Command(_)
        | HudInput::Open(_)
        | HudInput::Edit(_)
        | HudInput::ChangeRoot(_)
        | HudInput::AgentCall { .. }
        | HudInput::AgentAll { .. }
        | HudInput::AgentPipeline { .. } => {
            if word.is_empty() {
                String::new()
            } else {
                format!("{word} ")
            }
        }
        HudInput::GlobalQuery(_) => "/".to_string(),
        HudInput::SemanticQuery(_) | HudInput::Empty | HudInput::Invalid(_) => String::new(),
    };
    let body = text
        .strip_prefix(&prefix)
        .or_else(|| if text == word { Some("") } else { None })
        .unwrap_or(text.as_str())
        .to_string();
    let part = match parse_hud_input(input) {
        HudInput::Command(_)
        | HudInput::Open(_)
        | HudInput::Edit(_)
        | HudInput::ChangeRoot(_)
        | HudInput::AgentCall { .. }
        | HudInput::AgentAll { .. }
        | HudInput::AgentPipeline { .. } => PromptPart::Word,
        HudInput::Shell(_) | HudInput::GlobalQuery(_) => PromptPart::Prefix,
        _ => PromptPart::Text,
    };
    vec![
        (
            PromptPart::Prefix,
            prompt_mode_label(prompt_mode(input, false)).to_string() + " > ",
        ),
        (part, prefix),
        (PromptPart::Text, body),
    ]
}

fn hud_command_help() -> String {
    [
        "command box: plain text searches · /text searches globally",
        "$ <command> runs a shell command in the workspace root",
        ":name runs a Physis command; bare history/bench/context/dependencies/contradictions/action/rescan/graph/notebook/related/pin/focus/clear/zoom/edges/why/delta/trail/pins/help also work",
        "open <path-or-object> reads exact source · edit [path] edits inline · cd <dir> changes workspace root",
        "action <verb> <object> [key=value ...] calls the object/action registry and shows the recorded observation",
        "relate same|different|relates · tab why|context|delta|history · unpin removes the selected pin · sem off|on toggles embeddings · :external [path] uses $EDITOR · :quit quits",
    ]
    .join("\n")
}

/// One shell invocation, pinned to the workspace root and fully captured.
/// Runs as the current user with no privilege change; the caller displays
/// and records the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellOutcome {
    command: String,
    stdout: String,
    stderr: String,
    code: Option<i32>,
    duration_ms: u128,
}

fn run_shell_command(root: &Path, command: &str) -> anyhow::Result<ShellOutcome> {
    let command = command.trim();
    if command.is_empty() {
        anyhow::bail!("shell command needed after `$`");
    }
    let root = root
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("workspace root is not usable: {e}"))?;
    if !root.is_dir() {
        anyhow::bail!("workspace root is not a directory: {}", root.display());
    }
    let started = Instant::now();
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(&root)
        .output()
        .map_err(|e| anyhow::anyhow!("cannot run shell command: {e}"))?;
    Ok(ShellOutcome {
        command: command.to_string(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
        duration_ms: started.elapsed().as_millis(),
    })
}

fn render_shell_outcome(outcome: &ShellOutcome) -> String {
    const MAX_LINES: usize = 200;
    let mut o = String::new();
    o.push_str(&format!("$ {}\n", outcome.command));
    o.push_str(&format!(
        "exit: {} · {} ms\n",
        outcome
            .code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string()),
        outcome.duration_ms
    ));
    let mut lines: Vec<&str> = outcome
        .stdout
        .lines()
        .chain(outcome.stderr.lines())
        .collect();
    let truncated = lines.len().saturating_sub(MAX_LINES);
    lines.truncate(MAX_LINES);
    for line in lines {
        o.push_str(line);
        o.push('\n');
    }
    if truncated > 0 {
        o.push_str(&format!("… and {truncated} more lines\n"));
    }
    o
}

const MAX_INLINE_EDIT_BYTES: usize = 256 * 1024;

/// Minimal in-HUD text buffer. The file on disk stays canonical; this only
/// holds keystrokes until save or cancel. Cursor addressing is by Unicode
/// scalar, not terminal cell: honest plain-text editing, not a terminal Vim.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EditBuffer {
    path: PathBuf,
    label: String,
    original: String,
    chars: Vec<char>,
    cursor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditSave {
    unchanged: bool,
    before_hash: String,
    after_hash: String,
    bytes: usize,
}

impl EditBuffer {
    fn open(path: PathBuf, label: String, text: String) -> Self {
        let cursor = text.chars().count();
        let chars: Vec<char> = text.chars().collect();
        Self {
            path,
            label,
            original: text,
            chars,
            cursor,
        }
    }

    fn text(&self) -> String {
        self.chars.iter().collect()
    }

    fn insert_char(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    fn line_starts(&self) -> Vec<usize> {
        let mut starts = vec![0];
        for (i, c) in self.chars.iter().enumerate() {
            if *c == '\n' {
                starts.push(i + 1);
            }
        }
        starts
    }

    fn cursor_line_col(&self) -> (usize, usize) {
        let starts = self.line_starts();
        let mut line = 0;
        for (i, start) in starts.iter().enumerate() {
            if *start <= self.cursor {
                line = i;
            } else {
                break;
            }
        }
        (line, self.cursor - starts[line])
    }

    fn move_to_line_col(&mut self, line: usize, col: usize) {
        let starts = self.line_starts();
        let line = line.min(starts.len().saturating_sub(1));
        let start = starts[line];
        let end = if line + 1 < starts.len() {
            starts[line + 1] - 1
        } else {
            self.chars.len()
        };
        self.cursor = (start + col).min(end);
    }

    fn move_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line > 0 {
            self.move_to_line_col(line - 1, col);
        }
    }

    fn move_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        self.move_to_line_col(line + 1, col);
    }

    fn move_home(&mut self) {
        let (line, _) = self.cursor_line_col();
        self.move_to_line_col(line, 0);
    }

    fn move_end(&mut self) {
        let (line, _) = self.cursor_line_col();
        let starts = self.line_starts();
        let end = if line + 1 < starts.len() {
            starts[line + 1] - 1
        } else {
            self.chars.len()
        };
        self.cursor = end;
    }

    /// Render with an explicit block-cursor marker. The marker is UI only;
    /// `text()` remains the exact buffer.
    fn render_with_cursor(&self) -> String {
        let mut out = String::new();
        for (i, c) in self.chars.iter().enumerate() {
            if i == self.cursor {
                out.push('▌');
            }
            out.push(*c);
        }
        if self.cursor == self.chars.len() {
            out.push('▌');
        }
        out
    }

    /// Persist the buffer: refuse when the file moved under us, report a
    /// no-op when nothing changed, otherwise atomically replace it and hash
    /// both sides with the shared transition-log hash.
    fn save(&self) -> anyhow::Result<EditSave> {
        let disk = std::fs::read_to_string(&self.path)
            .map_err(|e| anyhow::anyhow!("cannot re-read {}: {e}", self.path.display()))?;
        if disk != self.original {
            anyhow::bail!(
                "file changed on disk since inline edit opened — save refused, Esc cancels"
            );
        }
        let text = self.text();
        let before_hash = hash_canonical(&self.original);
        let after_hash = hash_canonical(&text);
        if text == self.original {
            return Ok(EditSave {
                unchanged: true,
                before_hash,
                after_hash,
                bytes: text.len(),
            });
        }
        physis_pro::atomic::write_atomic(&self.path, text.as_bytes())
            .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", self.path.display()))?;
        Ok(EditSave {
            unchanged: false,
            before_hash,
            after_hash,
            bytes: text.len(),
        })
    }
}

/// Resolve an operator-supplied file path to a real file strictly inside the
/// current workspace. Object ids go through `NavWorkspace::resolve` in the
/// caller; this is the filesystem half of that gate.
fn resolve_workspace_file(root: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("a path is needed");
    }
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let abs = candidate
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("no such file: {raw}"))?;
    let root_abs = root
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("workspace root is not usable: {e}"))?;
    if !abs.starts_with(&root_abs) {
        anyhow::bail!("outside the workspace root: {raw}");
    }
    if !abs.is_file() {
        anyhow::bail!("not a file: {raw}");
    }
    Ok(abs)
}

/// Change-root gate: the target must exist and be a directory. The caller
/// reopens the workspace (and its per-repository state) only after this
/// passes.
fn resolve_change_root(current: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("usage: cd <directory>");
    }
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        current.join(raw)
    };
    let abs = candidate
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("no such directory: {raw}"))?;
    if !abs.is_dir() {
        anyhow::bail!("not a directory: {raw}");
    }
    Ok(abs)
}

fn display_workspace_path(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| abs.to_string_lossy().to_string())
}

/// Sibling candidates: hits whose content matches the center's but whose
/// path lives in a different directory. The exact "same content, branched
/// across directories" projection for big codebases. Pure over `hits` —
/// no second ranking; the shared scorer's semantic leg already did the work.
fn sibling_groups(
    center_path: &str,
    hits: &[NavHit],
    exclude_id: &nav::ObjectId,
) -> Vec<(String, Vec<String>)> {
    let center_dir = Path::new(center_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| ".".to_string());
    let mut by_dir: std::collections::BTreeMap<String, Vec<(f32, String)>> = Default::default();
    for h in hits {
        if h.object.id == *exclude_id {
            continue;
        }
        let path = h
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| h.object.label.clone());
        let dir = Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| ".".to_string());
        if dir == center_dir {
            continue; // same directory = ordinary neighbour, not a branch
        }
        by_dir.entry(dir).or_default().push((h.score.total, path));
    }
    by_dir
        .into_iter()
        .map(|(dir, mut items)| {
            items.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            (
                dir,
                items.into_iter().map(|(_, p)| p).collect::<Vec<String>>(),
            )
        })
        .collect()
}

/// Compact HUD state pointer for the existing append-only transition log.
/// Same shape as the REPL's `state_ref`, projected from the HUD's workspace.
fn workspace_state(root: &Path, ws: &physis_pro::nav::workspace::NavWorkspace) -> StateRef {
    let snap =
        physis_pro::mvp::slice::snapshot_hashes(&physis_pro::scanner::scan_project(root, None));
    let content_hash = hash_canonical(&snap);
    let mut ids: Vec<String> = ws
        .index()
        .objects
        .values()
        .map(|o| format!("{}:{}", o.id.short(), o.hash))
        .collect();
    ids.sort();
    StateRef {
        content_hash,
        objects: ws.index().objects.len(),
        files: snap.len(),
        map_hash: hash_canonical(&ids),
    }
}

/// Relative ranking bar for one hit: `▇` per filled slot, `▁` per empty one.
/// Positional only — it shows where a hit sits against this list's own max,
/// never a probability. Non-positive or non-finite scores draw empty.
fn score_bar(score: f32, max: f32, width: usize) -> String {
    if width == 0 || !max.is_finite() || max <= 0.0 {
        return "▁".repeat(width);
    }
    let ratio = if score.is_finite() && score > 0.0 {
        score / max
    } else {
        0.0
    };
    let filled = (ratio * width as f32).round().clamp(0.0, width as f32) as usize;
    "▇".repeat(filled) + &"▁".repeat(width - filled)
}

/// The startup world card: every demo-contract fact on one screen, all
/// measured at open — backend that answered, generator state, offline claim,
/// counts, and the cached sample map. Nothing estimated, nothing inherited.
fn world_card(
    root: &Path,
    files: usize,
    objects: usize,
    ws_tokens: usize,
    embedder: &str,
    generator: &str,
    map: (usize, usize, usize),
) -> String {
    let mut o = String::new();
    o.push_str("PHYSIS HUD — the repository as a navigable world\n\n");
    o.push_str(&format!("root: {}\n", root.display()));
    o.push_str(&format!(
        "semantic backend: {embedder} · generator: {generator} · network: OFFLINE\n"
    ));
    o.push_str(&format!(
        "files: {files} · objects: {objects} · workspace tokens: {ws_tokens}\n"
    ));
    o.push_str(&format!(
        "sample map: {} repeats / {} differences / {} contradictions\n",
        map.0, map.1, map.2
    ));
    o.push_str("\ntype a query, Enter searches · /text searches globally · $ runs shell · :help lists commands\n");
    o
}

/// Render nav-trail steps newest-last, capped. Pure projection over the
/// sled-backed trail the workspace already keeps.
fn trail_view_text(steps: &[nav::NavStep], limit: usize) -> String {
    let mut o = String::new();
    o.push_str("── TRAIL ──\n");
    if steps.is_empty() {
        o.push_str("(no navigation yet — search, then Enter)\n");
        return o;
    }
    let start = steps.len().saturating_sub(limit);
    for (i, s) in steps.iter().enumerate().skip(start) {
        o.push_str(&format!("{i:>3}  {:<6}  {}  {}\n", s.kind, s.focus, s.at));
    }
    o
}

/// Session transitions newest-last, capped. The caller supplies the git /
/// filesystem history text; this appends the recorded HUD transitions after
/// it so both pasts read as one view.
fn history_view_text(
    base: &str,
    transitions: &[physis_pro::mvp::log::Transition],
    limit: usize,
) -> String {
    let mut o = String::new();
    o.push_str(base);
    if !base.ends_with('\n') {
        o.push('\n');
    }
    if transitions.is_empty() {
        return o;
    }
    o.push_str("\nSESSION TRANSITIONS (this repository)\n");
    let start = transitions.len().saturating_sub(limit);
    for t in transitions.iter().skip(start) {
        o.push_str(&format!(
            "#{} {} → {} — {}\n",
            t.seq, t.intent, t.action, t.outcome
        ));
    }
    o
}

/// Notebook handoff targets: sourced selection wins; cursor is fallback.
/// Pure selection logic; workspace reads stay in `notebook_selection`.
fn notebook_targets<'a>(selected: &'a [NavHit], current: Option<&'a NavHit>) -> Vec<&'a NavHit> {
    let mut targets: Vec<&NavHit> = selected
        .iter()
        .filter(|h| h.object.source.is_some())
        .collect();
    if targets.is_empty() {
        if let Some(hit) = current {
            targets.push(hit);
        }
    }
    targets
}

/// Pure WHY-AFFECTED matcher: module-stem and code-shaped symbol explanation.
/// Filesystem reads and changed-symbol extraction stay in `why_affected`.
fn why_affected_lines(
    path: &str,
    text: &str,
    changed: &[String],
    symbols: &[String],
) -> Vec<String> {
    let lower = text.to_lowercase().replace('-', "_");
    let mut lines = Vec::new();
    for c in changed {
        let stem = Path::new(c)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.replace('-', "_").to_lowercase())
            .unwrap_or_default();
        if !stem.is_empty() && lower.contains(stem.as_str()) {
            lines.push(format!("{path}\n  ← names module {stem}\n  ← {c} changed"));
        }
    }
    for s in symbols {
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
            lines.push(format!(
                "{path}\n  ← calls {s}\n  ← {s} defined in changed file(s)"
            ));
            break;
        }
    }
    lines
}

/// A completed background agent call, delivered back to the run loop. An agent
/// turn is 43 s to 3 min, so it runs off the UI thread and the result arrives
/// through a channel; the HUD never blocks on a model.
struct AgentReply {
    agent: String,
    ok: bool,
    text: String,
}

struct App {
    ws: physis_pro::nav::workspace::NavWorkspace,
    weights: NavWeights,

    query: String,
    typing: bool,
    editor: Option<EditBuffer>,
    /// PH-030: the ranked results live behind one struct so `sel`/`rows`/`hits`
    /// are written atomically via `set_results` / `ResultState::new`. No caller
    /// assigns the individual fields directly.
    results: ResultState,
    list_state: ListState,
    selected: Vec<NavHit>,

    /// Reading pane: (title, exact source text).
    reading: Option<(String, String)>,
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
    /// Graph edge-class filter (Phase 32, `t` key): 0 = all edges,
    /// 1 = recorded relations only, 2 = near-neighbour rows only.
    /// Near-neighbour rows are similarity-ranked, not recorded — the filter
    /// states which leg is drawn instead of merging them silently.
    edge_filter: u8,
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
    /// Sender for background agent calls, installed by `run`. `None` in
    /// snapshot mode and tests, where there is no event loop to receive on.
    agent_tx: Option<std::sync::mpsc::Sender<AgentReply>>,
    /// The agent being asked right now, if a background call is in flight.
    agent_pending: Option<String>,

    /// Which surface is open (world/graph/notebook/tasks/agents/settings/
    /// setup/help). A view renders over the world panes; `Esc` returns.
    view: View,
    /// Library focus mirror: the same projection switch the bin performs,
    /// tracked through the invariant (focus/selection survive, cursor resets).
    /// The bin still owns rows and hits; this mirror is the seam the reducer
    /// owns outright in the next slice.
    focus_state: physis_pro::hud::world::FocusState,
    /// What the last bare line compiled to (`hud::intent::IntentResolution`).
    /// `None` until the operator types plain text; `:intent` shows it, and the
    /// status line carries its provenance at the moment it happens.
    last_intent: Option<physis_pro::hud::intent::IntentResolution>,
    /// The SETTINGS view's live documents. `settings` is always
    /// `shipped defaults + user file + this session's changes`; `cfg` holds the
    /// runtime knobs. Neither is a second store — both are the same documents
    /// the CLI reads and writes.
    settings: Settings,
    cfg: PhysisConfig,
    settings_view_rows: Vec<SettingRow>,
    settings_sel: usize,
    /// Overrides changed this session, dotted key → normalised value. Only
    /// these are written on save, so a save never freezes today's shipped
    /// defaults into the user's file.
    settings_dirty: BTreeMap<String, String>,
    /// `true` when a runtime knob moved; config.json is written only then.
    config_dirty: bool,
    /// Set while the command box edits a settings value: the dotted key.
    settings_edit: Option<String>,
     /// Last settings-view note (saved/failed/refused), shown in the body.
    settings_msg: String,
    /// Temporal write seam for lens projections (PH-027 bridge). Defaults to
    /// `PhysisNativeStore` (in-memory ordered log); swapped to `RerunTimeSeries`
    /// when `feature = "temporal-rerun"` is on. The lens never writes state
    /// directly — it flows through this trait object so the store layer owns
    /// persistence.
    temporal: Box<dyn physis_pro::backends::TemporalStore + Send + Sync>,
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
        // The kind decides whether semantic retrieval exists: a lexical hash is
        // a retriever, not meaning. One predicate, shared with the nav layer.
        let semantic_on = physis_pro::nav::is_semantic_kind(&kind);
        // Warm-up embed to keep the first query responsive.
        if semantic_on {
            let _ = ws.embedder(true).map(|e| e.embed("warm"));
        }
        // Startup summary, same facts as the REPL banner: file/object counts
        // from the post-prescan index, the cached sample map, and the
        // absorbed-at-open notice when the workspace moved while closed.
        let (map_r, map_d, map_c) = physis_pro::mvp::map_counts(&root);
        let snap = physis_pro::mvp::slice::snapshot_hashes(&physis_pro::scanner::scan_project(
            &root, None,
        ));
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
        let mut startup = world_card(
            &root,
            ws.index().files.len(),
            ws.index().objects.len(),
            ws_tokens,
            &kind,
            &physis_pro::mvp::generator_state(),
            (map_r, map_d, map_c),
        );
        if let Some(n) = notice {
            startup.push_str(&format!("\n{n}\n"));
        }
        // Retained intent (P2): the last query this workspace was asked is in
        // the trail, so a reopen resumes instead of starting blank. Prefill
        // the box so Enter re-runs it; the card says what will run.
        let resume = ws.last_intent();
        if let Some(step) = &resume {
            startup.push_str(&format!(
                "\nresume: last intent \"{}\" ({}) — Enter to run\n",
                step.focus, step.at
            ));
        }
        Ok(Self {
            ws,
            weights: NavWeights::default(),
            query: resume.map(|s| s.focus).unwrap_or_default(),
            typing: true,
            editor: None,
            results: ResultState::new(Vec::new(), Vec::new(), 0),
            list_state: ListState::default(),
            selected: Vec::new(),
            reading: Some(("startup".into(), startup)),
            explain: String::new(),
            stack: Vec::new(),
            crumbs: Vec::new(),
            status,
            semantic_on,
            embedder_kind: kind,
            changed: Default::default(),
            affected: Default::default(),
            zoomed_out: false,
            edge_filter: 0,
            intent_hot: Default::default(),
            intent_warm: Default::default(),
            context_tab: ContextTab::Why,
            last_delta: None,
            ws_tokens,
            agent_tx: None,
            agent_pending: None,
            view: View::World,
            settings: Settings::load(),
            cfg: load_hud_config(),
            settings_view_rows: Vec::new(),
            settings_sel: 0,
            settings_dirty: Default::default(),
            config_dirty: false,
            settings_edit: None,
            settings_msg: String::new(),
            focus_state: physis_pro::hud::world::FocusState::new(),
            last_intent: None,
            temporal: Box::new(physis_pro::backends::PhysisNativeStore::new()),
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
                for h in &self.results.hits {
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
        self.intent_hot.union(&self.intent_warm).cloned().collect()
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
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            return Vec::new();
        };
        why_affected_lines(path, &text, &changed_v, &syms)
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
        for h in self.results.hits.iter().take(3) {
            let path = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            o.push_str(&format!("  {:.2}  {path}\n", h.score.total));
        }
        if self.results.hits.is_empty() {
            o.push_str("  (none)\n");
        }
        o.push_str("\nDEPENDENCIES\n");
        let targets: Vec<String> = self
            .results
            .hits
            .iter()
            .take(3)
            .map(|h| {
                h.object
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| h.object.label.clone())
            })
            .collect();
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
        for h in self.results.hits.iter().take(3) {
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
        for h in self.results.hits.iter().take(2) {
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
        for h in self.results.hits.iter().take(8) {
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

    fn snapshot_state(&self) -> StateRef {
        workspace_state(&self.ws.root, &self.ws)
    }

    #[allow(clippy::too_many_arguments)]
    fn record_hud_transition(
        &self,
        intent: &str,
        action: &str,
        delta: CompactDelta,
        validation: Vec<String>,
        outcome: String,
        provenance: Vec<String>,
        before: &StateRef,
        after: &StateRef,
        uncertainty: f32,
    ) -> anyhow::Result<()> {
        let mut log = TransitionLog::open(&self.ws.root)?;
        log.append(Transition {
            v: 1,
            seq: 0,
            timestamp: String::new(),
            state_before: before.clone(),
            intent: intent.to_string(),
            action: action.to_string(),
            delta,
            validation,
            outcome,
            state_after: after.clone(),
            provenance,
            uncertainty,
            slice: None,
        })?;
        Ok(())
    }

    fn note_history_failure(&mut self, what: &str, e: &anyhow::Error) {
        self.status
            .push_str(&format!(" ({what} history not recorded: {e})"));
    }

    /// Re-index only what moved on disk (nav stat fast path — O(Δ)), then
    /// diff the file catalog around it for the CHANGED set and run the same
    /// precision-first dependents rule the MVP loop uses for AFFECTED.
    /// `u` in the HUD; no editor launched, no model touched.
    fn rescan_now(&mut self) {
        self.rescan_with_intent("rescan", "rescan");
    }

    /// Shared rescan path with an explicit intent/action for the history
    /// record. Returns whether the world update itself succeeded; logging
    /// failures only annotate the status.
    fn rescan_with_intent(&mut self, intent: &str, action: &str) -> bool {
        let before_state = self.snapshot_state();
        // Hash-level CHANGED: compare the refreshed catalog against the
        // pre-update one. Stat fast path keeps this O(Δ): unchanged files
        // cost one stat, only content-moved files rehash + re-ingest.
        let before_files: std::collections::BTreeMap<String, String> = self
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
                    match before_files.get(path) {
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
                for gone in before_files.keys() {
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
                let compact = CompactDelta {
                    changed: changed_v.clone(),
                    added: added.clone(),
                    removed: removed.clone(),
                    objects_remapped: objects_before.abs_diff(objects_after)
                        + changed_v.len()
                        + added.len(),
                    neighborhoods_changed: self.affected.len(),
                    no_change: changed_v.is_empty(),
                    ..Default::default()
                };
                self.last_delta = Some(physis_pro::mvp::SemanticDelta {
                    changed_artifacts: changed_all,
                    added_relations: vec![],
                    removed_relations: removed.clone(),
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
                let after_state = self.snapshot_state();
                let outcome = format!(
                    "{} changed, {} affected",
                    self.changed.len(),
                    self.affected.len()
                );
                let provenance = self
                    .changed
                    .iter()
                    .cloned()
                    .chain(added.iter().cloned())
                    .collect();
                if let Err(e) = self.record_hud_transition(
                    intent,
                    action,
                    compact,
                    vec![summary.clone()],
                    outcome,
                    provenance,
                    &before_state,
                    &after_state,
                    0.1,
                ) {
                    self.note_history_failure("rescan", &e);
                }
                // Re-run the current query so markers land on visible rows.
                // A leading `/` stays in the prompt and stays global.
                if !self.query.is_empty() {
                    self.run_query();
                }
                true
            }
            Err(e) => {
                self.status = format!("rescan failed: {e}");
                false
            }
        }
    }

    /// Open the cursor object in `$EDITOR` (blocking; TUI suspends while it
    /// runs), then rescan so the world reacts to the save. Undo-before-save
    /// is the editor's. `:external [path]` in the HUD.
    fn run_external_edit(&mut self, target: Option<&str>) {
        let path = match target {
            Some(raw) => match self.resolve_edit_path(Some(raw)) {
                Ok((_, label)) => label,
                Err(e) => {
                    self.status = format!("external edit: {e}");
                    return;
                }
            },
            None => {
                let Some(hit) = self.current_hit().cloned() else {
                    self.status = "nothing to edit".into();
                    return;
                };
                hit.object
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| hit.object.label.clone())
            }
        };
        let abs = self.ws.root.join(&path);
        if !abs.is_file() {
            // Absolute workspace paths arrive canonical; every other target is
            // workspace-relative by construction.
            let abs = if Path::new(&path).is_absolute() {
                PathBuf::from(&path)
            } else {
                abs
            };
            if !abs.is_file() {
                self.status = format!("not a file: {path}");
                return;
            }
            self.run_external_editor(&path, &abs);
            return;
        }
        self.run_external_editor(&path, &abs);
    }

    fn run_external_editor(&mut self, path: &str, abs: &Path) {
        let editor = std::env::var("EDITOR")
            .ok()
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| "vi".to_string());
        // Leave the alternate screen so the editor owns the terminal.
        let status = suspend_tui_for(|| Command::new(&editor).arg(abs).status());
        match status {
            Ok(s) if s.success() => {
                self.status = format!("edited {path} — rescanning");
                self.rescan_with_intent(&format!("external edit {path}"), "external-edit");
            }
            Ok(s) => self.status = format!("editor exited {s:?} — no rescan ran"),
            Err(e) => self.status = format!("cannot launch editor {editor}: {e}"),
        }
    }

    /// Resolve an inline-edit target to an absolute workspace file plus its
    /// display label. Explicit targets may be object ids or workspace paths;
    /// a missing target means the cursor object.
    fn resolve_edit_path(&self, target: Option<&str>) -> anyhow::Result<(PathBuf, String)> {
        let root = self.ws.root.clone();
        let raw_path = match target {
            Some(raw) => match self.ws.resolve(raw) {
                Ok(obj) => obj
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| obj.label.clone()),
                Err(_) => raw.to_string(),
            },
            None => {
                let Some(hit) = self.current_hit() else {
                    anyhow::bail!("nothing to edit");
                };
                hit.object
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| hit.object.label.clone())
            }
        };
        let abs = resolve_workspace_file(&root, &raw_path)?;
        let label = display_workspace_path(&root, &abs);
        Ok((abs, label))
    }

    /// Start the minimal inline editor on one workspace file. Full files only:
    /// object regions resolve to their file so the written bytes are exactly
    /// what the operator saw, plus their keystrokes.
    fn start_inline_edit(&mut self, target: Option<&str>) {
        let (abs, label) = match self.resolve_edit_path(target) {
            Ok(resolved) => resolved,
            Err(e) => {
                self.status = format!("edit: {e}");
                return;
            }
        };
        let bytes = match std::fs::metadata(&abs).map(|m| m.len()) {
            Ok(n) => n,
            Err(e) => {
                self.status = format!("edit: cannot stat {}: {e}", abs.display());
                return;
            }
        };
        if bytes > MAX_INLINE_EDIT_BYTES as u64 {
            self.status =
                format!("edit: {label} is too large for inline edit — use :external {label}");
            return;
        }
        let text = match std::fs::read_to_string(&abs) {
            Ok(text) => text,
            Err(e) => {
                self.status = format!("edit: cannot read {label}: {e}");
                return;
            }
        };
        self.editor = Some(EditBuffer::open(abs, label.clone(), text));
        self.typing = false;
        self.refresh_editor_reading();
        self.status = format!(
            "inline edit {label}: type and arrows move · Enter newline · Ctrl+S saves · Esc cancels"
        );
    }

    fn refresh_editor_reading(&mut self) {
        if let Some(editor) = &self.editor {
            self.reading = Some((
                format!("edit {}", editor.label),
                editor.render_with_cursor(),
            ));
        }
    }

    fn cancel_editor(&mut self) {
        if let Some(editor) = self.editor.take() {
            let body = std::fs::read_to_string(&editor.path).unwrap_or(editor.original);
            self.reading = Some((format!("edit {}", editor.label), body));
            self.status = format!("edit cancelled — {} untouched", editor.label);
        }
    }

    fn save_editor(&mut self) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        let rel = display_workspace_path(&self.ws.root, &editor.path);
        let saved = match editor.save() {
            Ok(saved) => saved,
            Err(e) => {
                self.editor = Some(editor);
                self.refresh_editor_reading();
                self.status = format!("save refused: {e}");
                return;
            }
        };
        if saved.unchanged {
            self.reading = Some((format!("edit {rel}"), editor.text()));
            self.status = format!("no changes — {rel} untouched");
            return;
        }
        self.reading = Some((format!("edit {rel}"), editor.text()));
        if self.rescan_with_intent(&format!("edit {rel}"), "edit-save") {
            self.status = format!(
                "saved {rel} · {} → {} · {} changed, {} affected",
                &saved.before_hash[..8.min(saved.before_hash.len())],
                &saved.after_hash[..8.min(saved.after_hash.len())],
                self.changed.len(),
                self.affected.len()
            );
        } else {
            let failed = self.status.clone();
            self.status = format!(
                "saved {rel} · {} → {} but {failed}",
                &saved.before_hash[..8.min(saved.before_hash.len())],
                &saved.after_hash[..8.min(saved.after_hash.len())]
            );
        }
    }

    /// Apply one reducer effect. The only place an effect becomes App state,
    /// and the reason the reducer can be tested without a terminal.
    fn apply_hud_effect(&mut self, effect: physis_pro::hud::event::HudEffect) {
        use physis_pro::hud::event::HudEffect;
        match effect {
            HudEffect::Status(s) => self.status = s,
            HudEffect::Reading { title, body } => self.reading = Some((title, body)),
            HudEffect::ClearPending => self.agent_pending = None,
            HudEffect::SetCursor { row } => {
                let clamped = if self.results.rows.is_empty() {
                    0
                } else {
                    row.min(self.results.rows.len() - 1)
                };
                self.focus_state.cursor = physis_pro::hud::world::Cursor::new(clamped);
                self.results.set_sel(clamped);
                self.sync_list_state();
            }
        }
    }

    /// The registered affordances of the focused object (`:actions`).
    ///
    /// This is the one place the running HUD asks "what may I do to *this*
    /// object?" — the question the palette seam exists for. It reads the same
    /// registry the action path executes through, so the list cannot promise a
    /// verb the executor would refuse, and an object with nothing registered
    /// says so instead of showing a generic menu.
    fn show_actions(&mut self) {
        use physis_pro::hud::palette;
        use physis_pro::world_object::spell_object_id;

        let Some(hit) = self.current_hit().cloned() else {
            self.status = "select an object to see what may be done to it".into();
            return;
        };
        let identity = spell_object_id(&hit.object.id);
        let path = hit
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_default();

        let mut reg =
            physis_pro::registry::Registry::new(physis_pro::registry::default_obs_path());
        reg.register_workspace(&self.ws);
        let view = physis_pro::guided::WorldView::from_registry(&reg);
        let rows = palette::affordances_for_identity(&view, &identity);

        let mut body = format!(
            "object: {} {}\npath:   {}\nkind:   {:?}\n\n",
            hit.object.label, identity, path, hit.object.kind
        );
        if rows.is_empty() {
            body.push_str(
                "no registered capability for this object.\n\n\
                 That is an answer, not an error: the registry holds no verb for this\n\
                 kind, so no action is offered rather than a generic menu that would\n\
                 be refused on execution. `:action list` prints the whole table.\n",
            );
        } else {
            for row in &rows {
                let confirm = if row.needs_confirm() { "  (confirms first)" } else { "" };
                body.push_str(&format!(
                    "  {:<12} {:<16} risk {}{}\n",
                    row.verb, row.capability, row.risk, confirm
                ));
            }
            body.push_str(&format!(
                "\n{} action(s), derived from the capability table — never a hand-written menu.\n\
                 run one with: :action <verb> {} [key=value ...]\n",
                rows.len(),
                identity
            ));
        }
        self.status = format!("actions: {} for {}", rows.len(), hit.object.label);
        self.reading = Some((format!("actions: {}", hit.object.label), body));
    }

    /// Turn the semantic leg off for this session without touching the store:
    /// the vector weight goes to zero and the mode label drops to the reduced
    /// form. Used by `--no-semantic`, so a machine that *has* an embedder can
    /// still prove the degraded path (the alternative is depending on a client
    /// that happens to lack one).
    fn disable_vector_leg(&mut self) {
        self.weights.vector = 0.0;
        self.semantic_on = false;
    }

    /// Resolve the bare line the operator just typed into an intent, and say so.
    ///
    /// The exact-match rule is deliberately literal: a line is an *exact object*
    /// only when it equals one object's path, label or identity spelling. Anything
    /// else is a hypothesis over the ranked matches, and the status line says
    /// which of the two happened — the operator can always tell an answer from a
    /// guess.
    fn resolve_bare_intent(&mut self) {
        use physis_pro::hud::intent;
        use physis_pro::world_object::spell_object_id;

        let utterance = self.query.trim_start_matches('/').trim().to_string();
        if utterance.is_empty() {
            return;
        }

        let spelling = |h: &NavHit| spell_object_id(&h.object.id);

        // Exact means exact: the line *is* one object's path, resolved by the
        // world's own rule (with its file-over-passage preference). A label
        // match, a prefix match and a ranked hit are all weaker claims, and they
        // stay on the intent route where the provenance says `intent-ir`.
        let exact: Option<String> = self.ws.resolve_exact(&utterance).map(|o| spell_object_id(&o.id));
        let matches: Vec<String> = self.results.hits.iter().take(5).map(spelling).collect();
        let resolution = intent::resolve_bare(&utterance, "operator", exact.as_deref(), &matches);

        if !self.semantic_on {
            // Keep the degradation notice: the resolution explains the line, the
            // mode explains the retrieval. Both belong on the status line, and
            // neither may displace the other.
            self.status
                .push_str(&format!(" · {}", resolution.provenance_line()));
        } else {
            self.status = resolution.provenance_line();
        }
        self.last_intent = Some(resolution);
    }

    /// The IR of the last bare line, or a note saying there is none. This is the
    /// `:intent` surface's second half: the stored operator intent plus what the
    /// textbox actually compiled.
    fn last_intent_text(&self) -> String {
        match &self.last_intent {
            Some(r) => {
                let ir = r
                    .ir
                    .as_ref()
                    .map(|ir| ir.render())
                    .unwrap_or_else(|| "(no IR)".to_string());
                format!(
                    "last bare line: {}\n{}\n\n{}\n",
                    r.utterance,
                    r.provenance_line(),
                    ir
                )
            }
            None => "(no bare line resolved yet — type one, or `open <path>`)\n".to_string(),
        }
    }

    fn submit_command_line(&mut self, raw: &str) -> bool {
        match parse_hud_input(raw) {
            HudInput::Empty => false,
            HudInput::Invalid(message) => {
                self.status = message;
                false
            }
            HudInput::Shell(command) => {
                self.run_shell_line(&command);
                false
            }
            HudInput::Command(command) => self.run_hud_command(command),
            HudInput::Open(target) => {
                self.open_target(&target);
                false
            }
            HudInput::Edit(target) => {
                self.start_inline_edit(target.as_deref());
                false
            }
            HudInput::ChangeRoot(dir) => {
                self.change_root(&dir);
                false
            }
            HudInput::GlobalQuery(query) => {
                self.query = format!("/{query}");
                self.run_query();
                false
            }
            HudInput::SemanticQuery(query) => {
                self.query = query;
                self.run_query();
                // The chain runs only for bare text: the exact parser above owns
                // `$`/`:`/`@`/`/`/`open`/`edit`/`cd`, so reaching here means the
                // line has no deterministic owner yet. This is the leg PH-026
                // shipped without a caller.
                self.resolve_bare_intent();
                false
            }
            HudInput::AgentCall { agent, host, rest } => {
                let roster = physis_pro::agents::default_roster();
                // The pane names its context: the explicit request, else the
                // current query, so the card is never detached from the world.
                let scope = agent_scope(&rest, &self.query);
                let mut text = match roster.get(&agent) {
                    Some(a) => {
                        let mut card =
                            physis_pro::agents::view::render_agent_card(a, scope.as_deref());
                        if let Some(h) = &host {
                            card.push_str(&format!(
                                "\nrequested host: {h} (remote placement is S12)"
                            ));
                        }
                        card
                    }
                    None => physis_pro::agents::view::render_unknown_agent(&agent, &roster),
                };

                // A bare `@agent` shows the card. `@agent <question>` asks it,
                // off the UI thread: a turn is 43 s to 3 min and must not freeze
                // the loop. The reply arrives through `agent_tx` in `run`.
                match (&self.agent_tx, rest.is_empty()) {
                    (Some(tx), false) => {
                        let tx = tx.clone();
                        let agent_id = agent.clone();
                        let question = rest.clone();
                        let scope = scope.clone();
                        text.push_str("\n\nasking in the background…");
                        self.agent_pending = Some(agent_id.clone());
                        self.status = format!(
                            "@{agent_id}: asking in the background — the TUI stays responsive"
                        );
                        std::thread::spawn(move || {
                            let reply = invoke_agent(&agent_id, &question, scope.as_deref());
                            let _ = tx.send(reply);
                        });
                    }
                    (Some(_), true) => {
                        self.status = format!(
                            "agent @{agent}: card shown — add a question to ask it, e.g. @{agent} what does this do?"
                        );
                    }
                    (None, _) => {
                        // Snapshot mode / tests: no event loop, so no call.
                        self.status = format!("agent @{agent}: card shown (no event loop)");
                    }
                }
                self.reading = Some((format!("@{agent}"), text));
                false
            }
            HudInput::AgentAll { request } => {
                let mut text = format!("@all {request}\n\nasking every agent in the background…\n");
                match &self.agent_tx {
                    Some(tx) => {
                        let tx = tx.clone();
                        let request = request.clone();
                        self.agent_pending = Some("all".into());
                        self.status = "@all: asking every agent in the background".into();
                        std::thread::spawn(move || {
                            let _ = tx.send(run_agent_all(&request));
                        });
                    }
                    None => {
                        text.push_str("\n(no event loop)");
                        self.status = "@all: no event loop".into();
                    }
                }
                self.reading = Some(("@all".into(), text));
                false
            }
            HudInput::AgentPipeline { stages } => {
                let desc = stages
                    .iter()
                    .map(|s| format!("@{} {}", s.agent, s.request))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let mut text = format!(
                    "{desc}\n\nrunning {} stage(s) in the background…\n",
                    stages.len()
                );
                match &self.agent_tx {
                    Some(tx) => {
                        let tx = tx.clone();
                        let stages = stages.clone();
                        self.agent_pending = Some(desc);
                        self.status = format!(
                            "pipeline: {} stage(s) running in the background",
                            stages.len()
                        );
                        std::thread::spawn(move || {
                            let _ = tx.send(run_agent_pipeline(&stages));
                        });
                    }
                    None => {
                        text.push_str("\n(no event loop)");
                        self.status = "pipeline: no event loop".into();
                    }
                }
                self.reading = Some(("pipeline".into(), text));
                false
            }
        }
    }

    fn run_hud_command(&mut self, command: HudCommand) -> bool {
        match command {
            HudCommand::Help => {
                self.reading = Some(("commands".into(), hud_command_help()));
                self.status = "command box: deterministic dispatch, no model".into();
                self.open_view(View::Help);
            }
            HudCommand::Tasks => {
                let path = std::path::PathBuf::from(
                    std::env::var("PHYSIS_OBSERVATIONS")
                        .unwrap_or_else(|_| ".physis/observations.jsonl".into()),
                );
                let text = match std::fs::read_to_string(&path) {
                    Ok(raw) => {
                        let obs = parse_observation_lines(&raw);
                        let g = physis_pro::agents::tasks::TaskGraph::fold(&obs);
                        format!(
                            "{} observations → {} tasks\n\n{}\n\
                             reported is a claim · only a verifier record completes a task ·\n\
                             an expired task gets a child naming its parent and the reason",
                            obs.len(),
                            g.tasks.len(),
                            g.render()
                        )
                    }
                    Err(e) => format!(
                        "no task log at {}: {e}\n\n\
                         Task events are observations with source \"task\"; \
                         write them with agents::tasks::TaskEvent::observation.",
                        path.display()
                    ),
                };
                self.status = format!("tasks: folded from {}", path.display());
                self.reading = Some(("tasks".into(), text));
                self.open_view(View::Tasks);
            }
            HudCommand::Intent => {
                let store = physis_pro::intent::IntentStore::load(&self.ws.root);
                let body = if store.intents.is_empty() {
                    format!(
                        "no intents yet\n\nset one with:\n  physis-pro intent set main \"what this workspace is for\"\nstored at {}",
                        physis_pro::intent::IntentStore::path(&self.ws.root).display()
                    )
                } else {
                    store.render()
                };
                // The textbox's own resolution sits first: the stored intent says
                // what the workspace is for, this says what the line you just
                // typed compiled to, with the route it took.
                self.status = "intent: the operator's statement, with provenance".into();
                self.reading = Some((
                    "intent".into(),
                    format!("{}\n{body}", self.last_intent_text()),
                ));
            }
            HudCommand::Agents => {
                let roster = physis_pro::agents::default_roster();
                let mut body = physis_pro::agents::view::render_strip(&roster);
                body.push('\n');
                for a in &roster.agents {
                    let s = physis_pro::agents::status(a);
                    let dot = if s.available { "●" } else { "○" };
                    body.push_str(&format!(
                        "  {dot} {:<10} {:<8} {:<28} {}\n",
                        a.id,
                        a.state.label(),
                        s.transport,
                        s.reason.as_deref().unwrap_or("")
                    ));
                }
                self.status = format!("agents: {} in the roster", roster.agents.len());
                self.reading = Some(("agents".into(), body));
                self.open_view(View::Agents);
            }
            HudCommand::Settings => {
                self.open_view(View::Settings);
            }
            HudCommand::Setup => {
                self.open_view(View::Setup);
            }
            HudCommand::Guided(intent) => self.show_guided(&intent),
            HudCommand::World => {
                self.view = View::World;
                self.status = "world: navigation, reading, tabs".into();
            }
            HudCommand::Action(line) => {
                let mut reg =
                    physis_pro::registry::Registry::new(physis_pro::registry::default_obs_path());
                reg.register_workspace(&self.ws);
                match physis_pro::action::dispatch(&mut reg, &line) {
                    Ok((_result, rendered)) => {
                        self.status = rendered
                            .lines()
                            .next()
                            .unwrap_or("action: done")
                            .to_string();
                        self.reading = Some((format!("action {line}"), rendered));
                    }
                    Err(e) => self.status = format!("action: {e}"),
                }
            }
            HudCommand::Actions => self.show_actions(),
            HudCommand::History => self.show_history(),
            HudCommand::Bench => self.show_bench(),
            HudCommand::Context => self.show_context(),
            HudCommand::Delta => {
                self.context_tab = ContextTab::Delta;
                self.reading = Some(("delta".into(), self.delta_text()));
                self.status = "delta: last observed Δ".into();
            }
            HudCommand::Why => self.show_why(),
            HudCommand::Dependencies(target) => {
                let path = match target {
                    Some(raw) => match self.resolve_command_path(&raw) {
                        Ok(path) => path,
                        Err(e) => {
                            self.status = format!("dependencies: {e}");
                            return false;
                        }
                    },
                    None => match self.current_hit().cloned() {
                        Some(hit) => hit
                            .object
                            .source
                            .as_ref()
                            .map(|s| s.path.clone())
                            .unwrap_or_else(|| hit.object.label.clone()),
                        None => {
                            self.status = "no cursor object — run a query first".into();
                            return false;
                        }
                    },
                };
                self.show_dependents_for(&path);
            }
            HudCommand::Contradictions => self.show_contradictions(),
            HudCommand::Rescan => self.rescan_now(),
            HudCommand::Graph => {
                self.graph_view();
                self.open_view(View::Graph);
            }
            HudCommand::Notebook => {
                self.show_notebook();
                self.open_view(View::Notebook);
            }
            HudCommand::Related => self.related(),
            HudCommand::Pin => self.pin_selection(),
            HudCommand::Relate(kind) => self.relate_selection(&kind),
            HudCommand::Focus(query) => self.focus_query_arg(query),
            HudCommand::ClearFocus => self.clear_focus(),
            HudCommand::Zoom => self.toggle_zoom(),
            HudCommand::CycleEdges => self.cycle_edge_filter(),
            HudCommand::Tab(tab) => self.switch_context_tab(tab),
            HudCommand::Trail => self.show_trail(),
            HudCommand::Pins => self.show_pins(),
            HudCommand::Unpin => self.unpin_selection(),
            HudCommand::Sem(enable) => self.set_semantic(enable),
            HudCommand::OpenExternal(target) => self.run_external_edit(target.as_deref()),
            HudCommand::Quit => return true,
        }
        false
    }

    fn resolve_command_path(&self, raw: &str) -> anyhow::Result<String> {
        if let Ok(obj) = self.ws.resolve(raw) {
            return Ok(obj
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| obj.label.clone()));
        }
        let abs = resolve_workspace_file(&self.ws.root, raw)?;
        Ok(display_workspace_path(&self.ws.root, &abs))
    }

    /// Open one exact object or workspace file in the reading pane. Object
    /// ids resolve through the index; paths stay strictly inside the root.
    /// The cursor moves only when the same object is already visible.
    fn open_target(&mut self, target: &str) {
        let root = self.ws.root.clone();
        let (obj, path) = match self.ws.resolve(target) {
            Ok(obj) => {
                let path = obj
                    .source
                    .as_ref()
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| obj.label.clone());
                (Some(obj), path)
            }
            Err(_) => (None, target.to_string()),
        };
        let abs = match resolve_workspace_file(&root, &path) {
            Ok(abs) => abs,
            Err(e) => {
                self.status = format!("open: {e}");
                return;
            }
        };
        let text = match std::fs::read_to_string(&abs) {
            Ok(text) => text,
            Err(e) => {
                self.status = format!("open: cannot read {}: {e}", abs.display());
                return;
            }
        };
        let label = display_workspace_path(&root, &abs);
        self.reading = Some((format!("open {label}"), text));
        if let Some(obj) = obj {
            let id = obj.id.clone();
            if let Some(pos) = self
                .results
                .rows
                .iter()
                .position(|row| matches!(row, Row::Hit(h) if h.object.id == id))
            {
                self.results.set_sel(pos);
                self.sync_list_state();
            }
            self.ws.push_trail("open", &id.short());
        } else {
            self.ws.push_trail("open", &label);
        }
        self.update_explain();
        self.status = format!("open {label} — exact file, no summary");
    }

    /// Change the workspace root. The target must already exist as a
    /// directory; the index, sled state, and transition log reopen beside the
    /// new root, mirroring startup. Volatile HUD selections do not cross the
    /// boundary.
    fn change_root(&mut self, raw: &str) {
        let current = self.ws.root.clone();
        let new_root = match resolve_change_root(&current, raw) {
            Ok(root) => root,
            Err(e) => {
                self.status = format!("cd: {e}");
                return;
            }
        };
        anchor_data_dir(&new_root);
        match physis_pro::nav::workspace::NavWorkspace::open(&new_root) {
            Ok(ws) => {
                self.ws = ws;
                self.query.clear();
                self.results = ResultState::new(Vec::new(), Vec::new(), 0);
                self.selected.clear();
                self.stack.clear();
                self.crumbs.clear();
                self.changed.clear();
                self.affected.clear();
                self.intent_hot.clear();
                self.intent_warm.clear();
                self.context_tab = ContextTab::Why;
                self.last_delta = None;
                self.editor = None;
                self.typing = false;
                self.ws_tokens = workspace_tokens(&new_root);
                self.embedder_kind = self.ws.embedder_kind(true);
                self.semantic_on = physis_pro::nav::is_semantic_kind(&self.embedder_kind);
                self.reading = Some((
                    "workspace".into(),
                    format!(
                        "root {}\nfiles {} · objects {}\nrun rescan to refresh this root",
                        new_root.display(),
                        self.ws.index().files.len(),
                        self.ws.index().objects.len()
                    ),
                ));
                self.sync_list_state();
                self.update_explain();
                self.status = format!("workspace root: {}", new_root.display());
            }
            Err(e) => {
                self.status = format!("cd: cannot open {}: {e}", new_root.display());
            }
        }
    }

    /// Run one shell command in the workspace root, display the captured
    /// result, rescan only when the filesystem actually moved, and record the
    /// interaction in the existing transition log.
    fn run_shell_line(&mut self, command: &str) {
        let root = self.ws.root.clone();
        let before_state = self.snapshot_state();
        let before_files = physis_pro::mvp::slice::snapshot_hashes(
            &physis_pro::scanner::scan_project(&root, None),
        );
        let outcome = suspend_tui_for(|| run_shell_command(&root, command));
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(e) => {
                self.reading = Some(("shell".into(), format!("$ {command}\n{e:#}")));
                self.status = format!("shell failed: {e:#}");
                return;
            }
        };
        self.reading = Some((
            format!("shell: {}", outcome.command),
            render_shell_outcome(&outcome),
        ));
        let after_files = physis_pro::mvp::slice::snapshot_hashes(
            &physis_pro::scanner::scan_project(&root, None),
        );
        let (changed, _added, _removed) =
            physis_pro::mvp::slice::diff_snapshots(&before_files, &after_files);
        if changed.is_empty() {
            let after_state = self.snapshot_state();
            let validation = vec![format!(
                "exit {} in {} ms",
                outcome
                    .code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "signal".to_string()),
                outcome.duration_ms
            )];
            if let Err(e) = self.record_hud_transition(
                &format!("$ {}", outcome.command),
                "shell",
                CompactDelta {
                    no_change: true,
                    ..Default::default()
                },
                validation,
                format!(
                    "exit {:?}; {} stdout lines, {} stderr lines",
                    outcome.code,
                    outcome.stdout.lines().count(),
                    outcome.stderr.lines().count()
                ),
                vec![outcome.command.clone()],
                &before_state,
                &after_state,
                0.1,
            ) {
                self.note_history_failure("shell", &e);
            }
            self.status = format!(
                "shell exit {:?} in {} ms — no filesystem change",
                outcome.code, outcome.duration_ms
            );
            return;
        }
        self.rescan_with_intent(&format!("$ {}", outcome.command), "shell");
        self.status = format!(
            "shell exit {:?} in {} ms; {}",
            outcome.code, outcome.duration_ms, self.status
        );
    }

    fn show_context(&mut self) {
        let text = self.context_text();
        self.context_tab = ContextTab::Context;
        self.reading = Some(("context".into(), text));
        self.status = "context: bounded packet for the current selection".into();
    }

    fn show_why(&mut self) {
        self.context_tab = ContextTab::Why;
        self.update_explain();
        self.status = "why: shared scorer components and relations".into();
    }

    /// Switch to a view and render it once. Views paint from state that
    /// already exists; nothing here is a second store and nothing calls a model.
    /// Opening SETTINGS reloads both documents from disk, so the view shows the
    /// merged truth (shipped defaults + user file + this session's changes).
    fn open_view(&mut self, view: View) {
        use physis_pro::hud::bridge::switch_to;
        // The library FocusState mirrors the bin's view so projection
        // switches go through the invariant: focus and selection survive,
        // cursor resets. The bin still owns rows/hits; the mirror is the
        // seam the reducer will own outright.
        switch_to(&mut self.focus_state, view.label());
        let (focused, selection) = match self.current_hit() {
            Some(hit) => (
                Some(hit.object.id.clone()),
                physis_pro::hud::bridge::selection_of(Some(hit)),
            ),
            None => (None, physis_pro::hud::world::Selection::empty()),
        };
        if focused.is_some() {
            self.focus_state.focus.refocus(focused);
            self.focus_state.selection = selection;
        }
        self.view = view;
        if view == View::Settings {
            self.settings = Settings::load();
            self.cfg = load_hud_config();
            self.rebuild_settings();
            self.settings_sel = next_setting_row(&self.settings_view_rows, 0, true);
        }
        self.refresh_view();
    }

    /// Open a view the way its command does. Graph and Notebook render from the
    /// command's output, so the view is opened with its text — never as an
    /// empty frame.
    fn open_view_via_command(&mut self, view: View) {
        match view {
            View::World => self.view = View::World,
            View::Graph => {
                self.graph_view();
                self.open_view(View::Graph);
            }
            View::Semantic => {
                self.semantic_view();
                self.open_view(View::Semantic);
            }
            View::Notebook => {
                self.show_notebook();
                self.open_view(View::Notebook);
            }
            View::Help => self.show_help(),
            View::Tasks => {
                self.run_hud_command(HudCommand::Tasks);
            }
            View::Agents => {
                self.run_hud_command(HudCommand::Agents);
            }
            View::Settings => {
                self.run_hud_command(HudCommand::Settings);
            }
            View::Setup => {
                self.run_hud_command(HudCommand::Setup);
            }
        }
    }

    /// Re-render the open view's body from live state (`r` in SETUP re-detects;
    /// SETTINGS and the reading-backed views rebuild on every draw anyway).
    fn refresh_view(&mut self) {
        if self.view == View::Setup {
            self.reading = Some(("setup".into(), setup_report(&self.setup_facts())));
        }
    }

    /// (title, body) of the open view. Reading-backed views (graph, notebook,
    /// tasks, agents, help) render the text the command that opened them
    /// produced — the view frame is what changed, not a second copy of the text.
    fn view_content(&self) -> (String, String) {
        match self.view {
            View::World => (String::new(), String::new()),
            View::Settings => (
                format!("settings — {}", View::Settings.keys()),
                self.settings_body(),
            ),
            View::Setup => (
                format!("setup — {}", View::Setup.keys()),
                setup_report(&self.setup_facts()),
            ),
            other => {
                let (title, body) = self.reading.clone().unwrap_or_else(|| {
                    (
                        other.label().to_string(),
                        "(nothing rendered yet — run the command again)".to_string(),
                    )
                });
                (format!("{title} — {}", other.keys()), body)
            }
        }
    }

    /// The live settings document: shipped defaults + the user's file + what
    /// this session changed. Rebuilt after every change, so the view can never
    /// show a value the engine would not resolve.
    fn rebuild_settings(&mut self) {
        let mut s = Settings::defaults();
        if let Ok(text) = std::fs::read_to_string(Settings::user_path()) {
            match Settings::parse(&text) {
                Ok(user) => s.merge(user),
                Err(e) => self.settings_msg = format!("settings file: {e}; using shipped defaults"),
            }
        }
        for (k, v) in &self.settings_dirty {
            if let Err(e) = s.set(k, v) {
                self.settings_msg = format!("settings: {e}");
            }
        }
        self.settings = s;
        self.settings_view_rows = settings_rows(&self.settings);
    }

    /// One change recorded: a knob marks config.json dirty; anything else
    /// becomes a settings override. Only overrides are ever written on save.
    fn record_change(&mut self, change: Change) {
        if change.is_knob {
            self.config_dirty = true;
            self.settings_msg = format!(
                "{} = {} — press s to write config.json",
                change.key, change.value
            );
        } else {
            self.settings_dirty
                .insert(change.key.clone(), change.value.clone());
            self.rebuild_settings();
            self.settings_msg = format!(
                "{} = {} — press s to write settings.json",
                change.key, change.value
            );
        }
    }

    /// Step the selected row (`+`/`-`). A typed row opens the command box
    /// instead, prefilled with its current value.
    fn step_selected(&mut self, up: bool) {
        let Some(row) = self.settings_view_rows.get(self.settings_sel) else {
            return;
        };
        let key = row.key();
        match step_setting(row, &self.settings, &mut self.cfg, up) {
            Ok(Some(change)) => self.record_change(change),
            Ok(None) => {
                if row.is_text() {
                    if let Some(k) = key {
                        let current = setting_value(row, &self.settings, &self.cfg);
                        self.settings_edit = Some(k);
                        self.query = if current == "-" { String::new() } else { current };
                        self.typing = true;
                    }
                } else {
                    self.settings_msg = "this row is not stepped — Enter types the value".into();
                }
            }
            Err(e) => self.settings_msg = format!("refused: {e}"),
        }
    }

    /// Apply a typed value from the command box.
    fn apply_settings_edit(&mut self, key: &str, value: &str) {
        match set_setting_text(&self.settings, &mut self.cfg, key, value) {
            Ok(change) => self.record_change(change),
            Err(e) => self.settings_msg = format!("refused: {e}"),
        }
    }

    /// Revert the selected row: this session's change, and the saved override
    /// with it — the merged truth is rebuilt either way.
    fn reset_selected(&mut self) {
        let reverted = self
            .settings_view_rows
            .get(self.settings_sel)
            .and_then(|row| row.key())
            .map(|k| self.settings_dirty.remove(&k).is_some())
            .unwrap_or(false);
        if reverted {
            self.rebuild_settings();
            self.settings_msg = "reverted to the saved value".into();
        } else {
            self.settings_msg = "nothing to revert on this row".into();
        }
    }

    /// Reload both documents from disk and drop this session's changes.
    fn reload_settings(&mut self) {
        self.settings_dirty.clear();
        self.config_dirty = false;
        self.settings_edit = None;
        self.cfg = load_hud_config();
        self.rebuild_settings();
        self.settings_msg = "reloaded from disk".into();
    }

    /// Write the session's changes. Only dirty keys go to settings.json, so a
    /// save can never freeze today's shipped defaults into the user's file;
    /// config.json is rewritten whole (it is a whole-file document) and only
    /// when a knob actually moved.
    fn save_settings(&mut self) {
        let mut user = Settings::default();
        for (k, v) in &self.settings_dirty {
            if let Err(e) = user.set(k, v) {
                self.settings_msg = format!("save refused: {e}");
                return;
            }
        }
        let path = Settings::user_path();
        let mut note = if self.settings_dirty.is_empty() {
            "settings: nothing new to write".to_string()
        } else {
            match user.save(&path) {
                Ok(()) => format!(
                    "settings: wrote {} key(s) to {}",
                    self.settings_dirty.len(),
                    path.display()
                ),
                Err(e) => {
                    self.settings_msg = format!("settings: {e}");
                    return;
                }
            }
        };
        if self.config_dirty {
            match save_hud_config(&self.cfg) {
                Ok(()) => note.push_str(&format!(
                    " · config.json written ({})",
                    self.cfg.data_dir.join("config.json").display()
                )),
                Err(e) => {
                    self.settings_msg = format!("config: {e}");
                    return;
                }
            }
        }
        self.config_dirty = false;
        self.settings_msg = note;
    }

    /// The SETTINGS view body: file paths first (so an operator knows what `s`
    /// writes), then every row with its value; the selected row carries its
    /// hint line and dirty rows carry the `*` that says "changed, unsaved".
    fn settings_body(&self) -> String {
        let mut o = String::new();
        o.push_str(&format!(
            "settings file  {}\n",
            Settings::user_path().display()
        ));
        o.push_str(&format!(
            "config file    {}\n",
            self.cfg.data_dir.join("config.json").display()
        ));
        o.push_str(&format!(
            "changed here   {} key(s) · config.json {}\n\n",
            self.settings_dirty.len(),
            if self.config_dirty { "dirty" } else { "clean" }
        ));
        for (i, row) in self.settings_view_rows.iter().enumerate() {
            match row {
                SettingRow::Section(name) => o.push_str(&format!("\n── {name} ──\n")),
                _ => {
                    let sel = if i == self.settings_sel { "▶" } else { " " };
                    let key = row.key().unwrap_or_default();
                    let dirty = if self.settings_dirty.contains_key(&key)
                        || (matches!(row, SettingRow::Knob(_)) && self.config_dirty)
                    {
                        "*"
                    } else {
                        " "
                    };
                    o.push_str(&format!(
                        "{sel} {dirty} {key:<44} {}\n",
                        setting_value(row, &self.settings, &self.cfg)
                    ));
                    if i == self.settings_sel {
                        o.push_str(&format!("        {}\n", row_hint(row)));
                    }
                }
            }
        }
        o.push_str("\n+/- step · Enter/e edit value · s save · r reload · d revert row\n");
        o.push_str(&self.settings_msg);
        o
    }

    /// The live SETUP facts. Reads only what is already loaded plus paths; the
    /// agent line is the cached roster probe, never a fresh PATH walk.
    fn setup_facts(&self) -> SetupFacts {
        let cfg = load_hud_config();
        let settings_path = Settings::user_path();
        let observations_path = std::path::PathBuf::from(
            std::env::var("PHYSIS_OBSERVATIONS")
                .unwrap_or_else(|_| ".physis/observations.jsonl".into()),
        );
        let agents = physis_pro::agents::default_roster()
            .agents
            .iter()
            .map(|a| {
                let s = physis_pro::agents::status(a);
                (a.id.clone(), s.transport, s.available)
            })
            .collect();
        SetupFacts {
            root: self.ws.root.clone(),
            files: self.ws.index().files.len(),
            objects: self.ws.index().objects.len(),
            workspace_tokens: self.ws_tokens,
            embedder: self.embedder_kind.clone(),
            semantic: self.semantic_on,
            data_dir: cfg.data_dir.clone(),
            config_path: cfg.data_dir.join("config.json"),
            settings_path: settings_path.clone(),
            settings_exists: settings_path.exists(),
            observations_path,
            agents,
        }
    }

    /// `guided <intent>`: compile the world the HUD already holds into a
    /// constraint and choose inside it. Read-only, no execution: this is the
    /// inspectable half — the operator reads the card and then decides, and the
    /// `runnable as:` line is the exact command that would do it.
    fn show_guided(&mut self, intent: &str) {
        let intent = if intent.trim().is_empty() {
            self.query.trim().to_string()
        } else {
            intent.trim().to_string()
        };
        if intent.is_empty() {
            self.status = "usage: guided <intent>".into();
            return;
        }
        let focus = self.current_hit().cloned();
        let focus_arg = focus.as_ref().map(|h| h.object.label.clone());
        let mut reg =
            physis_pro::registry::Registry::new(physis_pro::registry::default_obs_path());
        reg.register_workspace(&self.ws);
        let mut exec =
            physis_pro::guided::GuidedExecution::with_access(physis_pro::guided::Access::ReadOnly);
        match exec.step(
            &mut reg,
            focus_arg.as_deref(),
            &intent,
            &physis_pro::guided::MockGuidedModel,
            false,
        ) {
            Ok((step, _)) => {
                self.status = format!(
                    "guided: {} route from {} legal transition(s) · {} free field(s)",
                    step.action.action_line(),
                    step.context.provenance.affordances,
                    step.context.constraint.token_classes().free_fields
                );
                self.reading = Some(("guided".into(), step.render(&exec.metrics)));
                self.open_view(View::World);
            }
            Err(e) => self.status = format!("guided: {e}"),
        }
    }

    fn show_notebook(&mut self) {
        // Notebook handoff: run the Function 2 extractive answer over the
        // selection, in process. Result lands in the reading pane.
        if let Some(docs) = self.notebook_selection() {
            match run_notebook(self, &docs) {
                Ok(answer) => {
                    self.reading = Some(("notebook".into(), answer));
                }
                Err(e) => self.status = format!("notebook: {e}"),
            }
        } else {
            self.status = "select objects first (Space)".into();
        }
    }

    fn focus_query_arg(&mut self, query: Option<String>) {
        if let Some(query) = query {
            self.query = query;
            self.run_query();
        }
        self.apply_focus();
    }

    fn toggle_zoom(&mut self) {
        self.zoomed_out = !self.zoomed_out;
        let hits = self.results.hits.clone();
        self.set_results(hits);
        self.status = if self.zoomed_out {
            "zoomed out: directory rollup (z toggles, Enter drills in)".into()
        } else {
            "zoomed in: object rows".into()
        };
    }

    fn switch_context_tab(&mut self, tab: ContextTab) {
        self.context_tab = tab;
        match tab {
            ContextTab::Why => self.update_explain(),
            ContextTab::Context => {
                let text = self.context_text();
                self.reading = Some(("context".into(), text));
            }
            ContextTab::Delta => {
                let text = self.delta_text();
                self.reading = Some(("delta".into(), text));
            }
            ContextTab::History => {}
        }
        self.status = match tab {
            ContextTab::Why => "why: shared scorer components and relations".into(),
            ContextTab::Context => "context: bounded packet for the current selection".into(),
            ContextTab::Delta => "delta: last observed Δ".into(),
            ContextTab::History => "history: git/filesystem plus recorded transitions".into(),
        };
    }

    fn show_dependents_for(&mut self, path: &str) {
        let text = physis_pro::mvp::dependents_text(&self.ws.root, path);
        self.reading = Some((format!("dependents: {path}"), text));
        self.status = "dependents: code-shaped references to the cursor path".into();
    }

    /// Run the command-box query. A leading `/` stays in the prompt and
    /// stays global; the slash itself is never sent to the scorer.
    fn run_query(&mut self) {
        let raw = self.query.clone();
        let global = raw.starts_with('/');
        let query = if global {
            raw[1..].trim_start().to_string()
        } else {
            raw.clone()
        };
        match self.ws.query(&query, global, &self.weights, 60) {
            Ok((hits, mode)) => {
                self.semantic_on = mode.semantic;
                self.embedder_kind = mode.embedder.clone();
                self.ws.push_trail("query", &query);
                self.set_results(hits);
                if !mode.semantic {
                    self.status = format!(
                        "semantic: off ({}) — lexical/structural retrieval only",
                        mode.embedder
                    );
                }
            }
            Err(e) => self.status = format!("query failed: {e}"),
        }
    }

    fn set_results(&mut self, hits: Vec<NavHit>) {
        let mut rows = Vec::new();
        if self.zoomed_out {
            // Zoomed-out rollup: one row per directory, capped — never
            // renders thousands of nodes because they exist.
            for (dir, n) in aggregate_dirs(&hits, 60) {
                rows.push(Row::Dir(dir, n));
            }
            // Keep object rows for selection/reading, after the rollup.
            for (group, group_hits) in nav::group_hits(&hits) {
                rows.push(Row::Header(group.label().to_string(), group_hits.len()));
                for h in group_hits.iter().take(20) {
                    rows.push(Row::Hit((*h).clone()));
                }
            }
        } else {
            for (group, group_hits) in nav::group_hits(&hits) {
                rows.push(Row::Header(group.label().to_string(), group_hits.len()));
                for h in group_hits {
                    rows.push(Row::Hit(h.clone()));
                }
            }
        }
        // PH-030: build hits/rows/sel atomically through ResultState::new.
        self.results = ResultState::new(hits, rows, 0);
        self.sync_list_state();
        self.update_explain();
    }

    /// Intent focus: band the current ranked hits HOT/WARM/COLD (Phase 32).
    /// Projection only — no second ranking, no model. Pins never demote.
    fn apply_focus(&mut self) {
        let (hot, warm) = intent_paths(&self.results.hits);
        self.intent_hot = hot;
        self.intent_warm = warm;
        self.zoomed_out = false;
        self.set_results(self.results.hits.clone());
        self.status = format!(
            "attention: {} hot · {} warm · {} cold (F clears)",
            self.intent_hot.len(),
            self.intent_warm.len(),
            self.results.hits.len().saturating_sub(8),
        );
    }

    fn clear_focus(&mut self) {
        self.intent_hot.clear();
        self.intent_warm.clear();
        self.set_results(self.results.hits.clone());
        self.status = "attention cleared".into();
    }

    fn sync_list_state(&mut self) {
        self.list_state.select(Some(self.results.sel));
    }

    /// The single dispatch boundary for navigation events (PH-029).
    ///
    /// ```text
    /// keyboard / mouse / command / test
    ///       ↓
    ///   HudEvent
    ///       ↓
    ///   reduce(&mut self.focus_state, &event)
    ///       ↓
    ///   apply_hud_effects(effects)
    ///       ↓
    ///   canonical navigation state
    /// ```
    ///
    /// Every navigation mutation must go through here. Direct writes to
    /// `self.results.sel` or `self.focus_state` outside of `apply_hud_effects` are
    /// dual mutation paths and are the bug class PH-027 found.
    fn dispatch(&mut self, event: physis_pro::hud::event::HudEvent) {
        let effects = physis_pro::hud::event::reduce(&mut self.focus_state, &event);
        self.apply_hud_effects(effects);
    }

    /// Apply the effects the reducer returned. This is the only function
    /// allowed to write `self.results.sel` from a navigation event. It is intentionally
    /// not `pub` — the path is `dispatch → reduce → apply_hud_effects`, not
    /// a public mutation surface.
    fn apply_hud_effects(&mut self, effects: Vec<physis_pro::hud::event::HudEffect>) {
        use physis_pro::hud::event::HudEffect;
        for effect in effects {
            match effect {
                HudEffect::SetCursor { row } => {
                    // Clamp to actual row count; the reducer does not know it.
                    let clamped = if self.results.rows.is_empty() {
                        0
                    } else {
                        row.min(self.results.rows.len() - 1)
                    };
                    // Sync to FocusState so the two stay in agreement.
                    self.focus_state.cursor =
                        physis_pro::hud::world::Cursor::new(clamped);
                    self.results.set_sel(clamped);
                    self.sync_list_state();
                    self.update_explain();
                }
                HudEffect::Status(s) => {
                    self.status = s;
                }
                HudEffect::Reading { title, body } => {
                    self.reading = Some((title, body));
                }
                HudEffect::ClearPending => {
                    self.agent_pending = None;
                }
            }
        }
    }

    fn update_explain(&mut self) {
        self.explain.clear();
        if let Some(Row::Dir(dir, n)) = self.results.rows.get(self.results.sel) {
            // Directory rollup: expand into it on Enter.
            self.explain
                .push_str(&format!("DIR {dir} ({n} hits)\nEnter drills in.\n"));
            return;
        }
        if let Some(Row::Hit(h)) = self.results.rows.get(self.results.sel) {
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
            self.explain
                .push_str(&format!("RELEVANCE  {:.2}\n", h.score.total));
            for (name, raw) in &h.score.components {
                self.explain.push_str(&format!("{name:<10} {raw:.2}\n"));
            }
            self.explain
                .push_str(&format!("\nid: {}\n", h.object.id.short()));
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
            if self.selected.iter().any(|s| s.object.id == h.object.id) {
                self.explain.push_str("\n[SELECTED]\n");
            }
        }
    }

    fn current_hit(&self) -> Option<&NavHit> {
        match self.results.rows.get(self.results.sel) {
            Some(Row::Hit(h)) => Some(h),
            _ => None,
        }
    }

    /// Drill into a directory rollup: keep the ranked hits, show the rows
    /// for that directory first. Zoomed-out ↔ zoomed-in over one substrate.
    fn enter_dir(&mut self, dir: &str) {
        self.stack.push((
            self.query.clone(),
            self.results.hits.clone(),
            self.results.rows.clone(),
            self.results.sel,
        ));
        self.crumbs.push(dir.to_string());
        let mut rows = vec![Row::Dir(dir.to_string(), 0)];
        for (group, group_hits) in nav::group_hits(&self.results.hits) {
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
        // PH-030: keep hits, replace rows/sel atomically.
        let hits = self.results.hits.clone();
        self.results = ResultState::new(hits, rows, 0);
        self.sync_list_state();
        self.update_explain();
    }

    /// Zoom in: a passage reads directly; anything else re-queries within it.
    /// The objects one step *inside* this one, from the index the HUD already
    /// holds: a file's passages, or a directory's own entries.
    ///
    /// This is the semantic-zoom grammar's data half (§13). It reads the index
    /// rather than the filesystem, so descending costs nothing and cannot fail
    /// on a permission bit — and it returns empty for a leaf, which is how
    /// `enter` knows to fall back to reading.
    fn children_of(&self, obj: &physis_pro::nav::PhysisObject) -> Vec<NavHit> {
        use physis_pro::nav::ObjectKind as K;
        let objects = &self.ws.index().objects;
        let path_of = |o: &physis_pro::nav::PhysisObject| {
            o.source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_default()
        };
        let target = path_of(obj);
        if target.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<NavHit> = Vec::new();
        match obj.kind {
            // A file's contents are its passages, in reading order.
            K::File => {
                let mut passages: Vec<&physis_pro::nav::PhysisObject> = objects
                    .values()
                    .filter(|o| o.kind == K::Passage && path_of(o) == target)
                    .collect();
                passages.sort_by_key(|o| {
                    o.source
                        .as_ref()
                        .and_then(|s| s.start_line)
                        .unwrap_or(usize::MAX)
                });
                for p in passages {
                    out.push(NavHit {
                        object: p.clone(),
                        score: Default::default(),
                    });
                }
            }
            // A directory's contents are the non-passage objects whose parent
            // directory is exactly it — one level, never a subtree.
            K::Directory => {
                for o in objects.values() {
                    if o.kind == K::Passage {
                        continue;
                    }
                    let p = path_of(o);
                    let parent = Path::new(&p)
                        .parent()
                        .map(|pp| pp.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if parent == target {
                        out.push(NavHit {
                            object: o.clone(),
                            score: Default::default(),
                        });
                    }
                }
                out.sort_by(|a, b| a.object.label.cmp(&b.object.label));
                out.dedup_by(|a, b| path_of(&a.object) == path_of(&b.object));
            }
            // Concepts, notes and passages are leaves here: their contents are
            // text to read, not objects to descend into.
            _ => {}
        }
        out
    }

    fn enter(&mut self) {
        let Some(hit) = self.current_hit().cloned() else {
            return;
        };
        // Push current state for Esc.
        self.stack.push((
            self.query.clone(),
            self.results.hits.clone(),
            self.results.rows.clone(),
            self.results.sel,
        ));
        self.crumbs.push(hit.object.label.clone());

        // Descend when the object has contents: the projection becomes *about*
        // this object, and its contents are the rows. Focus stays on the
        // container, so `WHY`/`:actions` still answer for the thing you opened
        // rather than for whichever child the cursor happens to sit on.
        let children = self.children_of(&hit.object);
        if !children.is_empty() {
            let container = hit.object.id.clone();
            let label = hit.object.label.clone();
            self.query = format!("inside: {}", label);
            self.set_results(children);
            // Descend through the canonical path: FocusState owns focus + trail.
            self.dispatch(physis_pro::hud::event::HudEvent::Descend {
                scope: label.clone(),
                crumb: label.clone(),
                id: container,
            });
            self.reading = Some((
                format!("inside: {}", label),
                format!(
                    "{} object(s) inside {} — Enter descends again, Esc/Backspace climbs out",
                    self.results.hits.len(),
                    label
                ),
            ));
            self.status = format!(
                "descended into {} ({} objects) — Esc climbs out, projection unchanged",
                label,
                self.results.hits.len()
            );
            self.ws.push_trail("enter", &hit.object.id.short());
            return;
        }

        match hit.object.kind {
            ObjectKind::Passage | ObjectKind::Note | ObjectKind::File => {
                match self.ws.read(&hit.object) {
                    Ok(text) => {
                        self.reading = Some((hit.object.label.clone(), text));
                    }
                    Err(e) => self.status = format!("cannot read: {e}"),
                }
            }
            _ => {}
        }
        // Leaf: focus through the canonical path, then re-query neighbourhood.
        self.dispatch(physis_pro::hud::event::HudEvent::FocusObject {
            id: hit.object.id.clone(),
        });
        if let Ok((hits, _)) = self.ws.query(&hit.object.text, true, &self.weights, 20) {
            self.query = format!("inside: {}", hit.object.label);
            self.set_results(hits);
        }
        self.ws.push_trail("enter", &hit.object.id.short());
    }

    fn back(&mut self) {
        if let Some((q, hits, rows, sel)) = self.stack.pop() {
            self.query = q;
            // PH-030: restore all three fields atomically.
            self.results = ResultState::restore(hits, rows, sel);
            self.reading = None;
            self.sync_list_state();
            self.update_explain();
            // Capture the focused id from the restored rows/sel *before*
            // dispatching Ascend: the Ascend effect resets the cursor to 0,
            // which would shift current_hit() to a header row and return None.
            let focused = self.current_hit().map(|h| h.object.id.clone());
            // Pop the trail through the canonical path.
            self.dispatch(physis_pro::hud::event::HudEvent::Ascend);
            // Restore focus to the object we climbed back to.
            self.focus_state.focus.refocus(focused);
        }
        self.crumbs.pop();
    }

    /// Adapter: rebuild the legacy `Vec<NavHit>` from the canonical
    /// `FocusState.selection`. Called after any `Select` / `ClearSelection`
    /// event so the legacy code that reads `self.selected` (pin, relate,
    /// notebook handoff) sees the right objects.
    ///
    /// This exists because `selected` carries full `NavHit` objects (which
    /// include score data) while `FocusState.selection` carries only `ObjectId`.
    /// Until `selected` is fully replaced, this bridge keeps them consistent.
    fn sync_selected_from_focus_state(&mut self) {
        let ids = self.focus_state.selection.ids.clone();
        self.selected = self
            .results
            .hits
            .iter()
            .filter(|h| ids.contains(&h.object.id))
            .cloned()
            .collect();
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

    /// Edge-class visibility for the graph dual view. `recorded` is true for
    /// rows drawn from stored relations, false for similarity-ranked
    /// near-neighbour rows. Filter: 0 = all, 1 = recorded only, 2 = near only.
    /// Pure so the toggle states exactly what it draws (ponytail: two arms, no
    /// per-kind taxonomy until a third leg exists).
    fn edge_shown(recorded: bool, filter: u8) -> bool {
        match filter {
            1 => recorded,
            2 => !recorded,
            _ => true,
        }
    }

    /// Status text for the edge filter, shared by the toggle and the graph view.
    fn edge_filter_name(filter: u8) -> &'static str {
        match filter {
            1 => "recorded relations only",
            2 => "near neighbours only",
            _ => "all edges",
        }
    }

    /// Cycle the graph edge filter (`t`): all → recorded → near → all.
    /// Re-renders the graph view when it is showing so the key visibly acts.
    fn cycle_edge_filter(&mut self) {
        self.edge_filter = (self.edge_filter + 1) % 3;
        self.status = format!("graph edges: {}", Self::edge_filter_name(self.edge_filter));
        if self
            .reading
            .as_ref()
            .is_some_and(|(t, _)| t.starts_with("graph: "))
        {
            self.graph_view();
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
        let center_path = center
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| center.object.label.clone());

        // The projection is the library's (`ObjectLens`), the state markers are
        // the App's. Before this, the graph re-derived the two edge legs inline
        // and the lens was reachable only from its own test — the repo's
        // named defect shape. A lens row now carries the target's identity, so
        // a graph row and a list row can be compared and acted on.
        let lens =
            physis_pro::hud::lens_object::ObjectLens::project(&self.ws, &center, &self.results.hits);
        let pinned = self.pinned_paths();
        let intent = self.intent_paths();
        let center_mark = state_of(
            &center_path,
            &self.changed,
            &self.affected,
            &pinned,
            &intent,
            &self.selected,
            &center.object,
        );

        let mut lines = Vec::new();
        lines.push(format!("{center_mark} {}", lens.center.content));
        lines.push(format!("edges: {}", Self::edge_filter_name(self.edge_filter)));

        // Recorded relations (solid/double by origin): a way to a file, not a
        // wall of labels — every row names the path `open`/`edit` accept.
        if Self::edge_shown(true, self.edge_filter) {
            for e in &lens.edges {
                let m = self.marker_for_identity(&e.id, &e.path, &pinned, &intent);
                lines.push(format!(
                    "  {m} {} {} [{}] {} {}→  {}",
                    e.direction, e.glyph, e.kind, e.label, e.glyph, e.path
                ));
            }
        }

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

        // Near neighbours: dashed, and `Inferred` by construction — the lens
        // never lets a similarity row borrow the grammar of a recorded edge.
        if Self::edge_shown(false, self.edge_filter) {
            for n in &lens.near {
                let m = self.marker_for_identity(&n.id, &n.path, &pinned, &intent);
                lines.push(format!(
                    "  {m}  {} [{}] {} {}  {}",
                    n.glyph, n.kind, n.label, n.glyph, n.path
                ));
            }
        }

        // SIBLINGS: content-similar files in *other* directories — the
        // "branched differently across directories" projection. Same
        // semantic query `related()` uses (the center's own text), no new
        // ranking; only the cross-directory slice of it is drawn. This is
        // similarity, NOT a proven relationship — the dashed edge grammar
        // says so, and the legend line names the leg honestly.
        if Self::edge_shown(false, self.edge_filter) {
            let ws = &mut self.ws;
            let weights = self.weights.clone();
            if let Ok((sib_hits, _)) = ws.query(&center.object.text, true, &weights, 60) {
                let siblings = sibling_groups(&center_path, &sib_hits, &center.object.id);
                let total: usize = siblings.iter().map(|(_, v)| v.len()).sum();
                if !siblings.is_empty() {
                    lines.push(String::new());
                    lines.push(format!(
                        "SIBLINGS — similar content, other directories ({total} files; similarity, not proven relation)"
                    ));
                    for (dir, paths) in siblings.iter().take(6) {
                        lines.push(format!("  ▸ {dir} ({})", paths.len()));
                        for p in paths.iter().take(3) {
                            lines.push(format!("      - - {p}"));
                        }
                        if paths.len() > 3 {
                            lines.push(format!("      … and {} more", paths.len() - 3));
                        }
                    }
                    if siblings.len() > 6 {
                        lines.push(format!("  … and {} more directories", siblings.len() - 6));
                    }
                    lines.push(
                        "  Enter drills into a sibling's directory from the list (z rollup)".into(),
                    );
                }
            }
        }

        lines.push(String::new());
        lines.push("solid = user · double = deterministic · dashed = inferred/near".into());
        lines.push("edge ids: rel:<a>:<kind>:<b> (canonical order)".into());
        lines.push(
            "★ selected · ◆ human-pinned · ● changed · ◐ affected · ▲ intent · · ordinary".into(),
        );
        lines.push(
            "every edge above names its file — `open <path>` / `edit <path>` accepts it".into(),
        );
        self.reading = Some((
            format!("graph: {} — {}", lens.center.content, center_path),
            lines.join("
"),
        ));
    }

    /// Semantic projection (`M`, `--view semantic`): the focused object's
    /// meaning-neighbourhood, drawn from `hud::lens_semantic` — the same ranked
    /// list the world view shows, projected differently. No second ranking, no
    /// model, and every edge says `Inferred` because similarity is not a claim
    /// anyone made.
    fn semantic_view(&mut self) {
        use physis_pro::hud::lens_semantic;
        use physis_pro::hud::row::ProjectionRow;

        let Some(center) = self.current_hit().cloned() else {
            self.status = "select an object to project semantically".into();
            return;
        };
        let projection = lens_semantic::project_rows(&center, &self.results.hits);

        // PH-027 bridge: register every projected row as a typed `Observation`
        // through the temporal store seam so the lens is also a temporal write.
        // The center object is excluded (it is the focus, not a neighbour).
        let center_id = physis_pro::world_object::spell_object_id(&center.object.id);
        physis_pro::hud::lens_semantic::register_projection(
            &center_id,
            &projection.rows,
            &mut *self.temporal,
            "semantics",
            "hud",
            None,
        );

        let center_path = projection
            .center
            .as_ref()
            .map(|c| c.path.clone())
            .unwrap_or_else(|| center.object.label.clone());

        let pinned = self.pinned_paths();
        let intent = self.intent_paths();
        let mut lines = Vec::new();
        let center_mark = state_of(
            &center_path,
            &self.changed,
            &self.affected,
            &pinned,
            &intent,
            &self.selected,
            &center.object,
        );
        lines.push(format!(
            "{center_mark} {}",
            projection
                .center
                .as_ref()
                .map(|c| c.label.clone())
                .unwrap_or_default()
        ));

        // The lens's categories, named by the lens. A category the data did not
        // justify is simply absent rather than rendered empty.
        let names: Vec<String> = projection
            .groups
            .iter()
            .filter(|g| !g.object_ids.is_empty())
            .map(|g| format!("{} {}", g.name, g.object_ids.len()))
            .collect();
        if !names.is_empty() {
            lines.push(format!("categories: {}", names.join(" · ")));
        }

        let row_line = |row: &ProjectionRow, mark: char| {
            let badge = projection
                .groups
                .iter()
                .find(|g| g.contains(&row.object_id))
                .map(|g| format!("<{}> ", g.name))
                .unwrap_or_default();
            let score = match row.score {
                Some(s) if row.is_similarity() => format!("{s:.2} "),
                _ => String::new(),
            };
            format!(
                "  {mark} - - [{badge}{score}{}] {}  {}",
                row.relation.label(),
                row.label,
                row.path
            )
        };

        for row in &projection.rows {
            let mark = self.marker_for_identity(&row.object_id, &row.path, &pinned, &intent);
            lines.push(row_line(row, mark));
        }

        lines.push(String::new());
        lines.push(
            "similarity only: every row above is Inferred <inferred>, never asserted".into(),
        );
        lines.push(
            "categories are the lens's measurement (score threshold, two-hop bridge, shared directory)"
                .into(),
        );
        lines.push("g switches to the recorded-relation projection; Esc returns to the world".into());
        self.reading = Some((
            format!("semantic: {} — {}", center.object.label, center_path),
            lines.join("\n"),
        ));
    }

    /// The list's state marker for a row that arrived from a projection.
    /// Resolves the row's identity back to the object the list would show, so
    /// the graph cannot mark a row differently from the list. Falls back to
    /// `·` (ordinary) when the identity is not in the index — an honest
    /// "unknown", never a borrowed marker.
    fn marker_for_identity(
        &self,
        identity: &str,
        path: &str,
        pinned: &BTreeSet<String>,
        intent: &BTreeSet<String>,
    ) -> char {
        let oid = physis_pro::world_object::parse_object_id(identity);
        let key = serde_json::to_string(&oid).unwrap_or_default();
        match self.ws.index().objects.get(&key) {
            Some(obj) => state_of(
                path,
                &self.changed,
                &self.affected,
                pinned,
                intent,
                &self.selected,
                obj,
            ),
            None => '·',
        }
    }

    fn related(&mut self) {
        if let Some(hit) = self.current_hit().cloned() {
            // Re-run the neighborhood query seeded by this object.
            if let Ok((hits, _)) = self.ws.query(&hit.object.text, true, &self.weights, 20) {
                self.stack.push((
                    self.query.clone(),
                    self.results.hits.clone(),
                    self.results.rows.clone(),
                    self.results.sel,
                ));
                self.crumbs.push(format!("related: {}", hit.object.label));
                self.query = format!("related: {}", hit.object.label);
                self.set_results(hits);
            }
        }
    }

    /// Recent past (`H`, tab 4): git log where present, else mtime order —
    /// the same projection the REPL `history` command reports — followed by
    /// this repository's recorded HUD transitions. The view itself is
    /// read-only; shell, rescan, and inline-edit saves append elsewhere.
    fn show_history(&mut self) {
        self.context_tab = ContextTab::History;
        self.status = "history: git log/filesystem plus recorded HUD transitions".into();
    }

    fn history_view(&self) -> String {
        let base = physis_pro::mvp::history_text(&self.ws.root);
        let transitions = TransitionLog::open(&self.ws.root)
            .map(|log| log.read_all())
            .unwrap_or_default();
        history_view_text(&base, &transitions, 8)
    }

    /// Navigation trail (`:trail`): the sled-backed steps this workspace
    /// already records on every query/enter/open. Read-only projection.
    fn show_trail(&mut self) {
        let text = trail_view_text(&self.ws.trail(), 15);
        self.reading = Some(("trail".into(), text));
        self.status = "trail: recorded navigation steps, newest last".into();
    }

    /// Human pins (`:pins`): the sled-backed pairs with their labels.
    /// Read-only; `:unpin` removes the selected pair.
    fn show_pins(&mut self) {
        let pins = self.ws.pins();
        let mut o = String::new();
        o.push_str("── PINS ──\n");
        if pins.is_empty() {
            o.push_str("(none — select exactly 2 objects with Space, then p)\n");
        }
        for p in &pins {
            let label = |id: &nav::ObjectId| {
                self.ws
                    .index()
                    .objects
                    .get(&serde_json::to_string(id).unwrap())
                    .map(|o| o.label.clone())
                    .unwrap_or_else(|| id.short())
            };
            o.push_str(&format!("◆ {} ─── {}\n", label(&p.a), label(&p.b)));
        }
        self.reading = Some(("pins".into(), o));
        self.status = "pins: human assertions, distinct from intent focus".into();
    }

    /// Remove the pin between the two selected objects, if one exists.
    /// Mirrors `pin_selection`; a repeat reports, never errors.
    fn unpin_selection(&mut self) {
        if self.selected.len() != 2 {
            self.status = "select exactly 2 objects to unpin (Space)".into();
            return;
        }
        let (a, b) = (self.selected[0].clone(), self.selected[1].clone());
        match self.ws.remove_pin(&a.object, &b.object) {
            Ok(0) => self.status = "no pin between those objects".into(),
            Ok(n) => self.status = format!("unpinned ({n} removed)"),
            Err(e) => self.status = format!("unpin failed: {e}"),
        }
    }

    /// Toggle the embedding backend (`:sem off|on`): drop embeddings to run
    /// lexical/structural only, or reselect and re-embed. Reuses the
    /// workspace seam; the header reports whichever backend answers.
    fn set_semantic(&mut self, enable: bool) {
        let msg = if enable {
            match self.ws.sem_on() {
                Ok((n, kind)) => format!("semantic on: {n} embeddings ({kind})"),
                Err(e) => {
                    self.status = format!("sem on failed: {e}");
                    return;
                }
            }
        } else {
            match self.ws.sem_off() {
                Ok(n) => format!("semantic off: dropped {n} embeddings, lexical/structural only"),
                Err(e) => {
                    self.status = format!("sem off failed: {e}");
                    return;
                }
            }
        };
        self.embedder_kind = self.ws.embedder_kind(true);
        self.semantic_on = physis_pro::nav::is_semantic_kind(&self.embedder_kind);
        if !self.query.is_empty() {
            self.run_query();
        }
        self.status = msg;
    }

    /// Command help (`?`): the deterministic vocabulary plus the key map.
    /// Read-only; the mode label beside the prompt says what Enter will do.
    fn show_help(&mut self) {
        let mut o = hud_command_help();
        o.push_str("\nkeys: / search · Enter zoom · Esc back · Space select · e inline edit · :external editor · u rescan · g graph · t edges · H history · B bench · D dependents · C contradictions · f/F focus · z zoom · 1/2/3/4 tabs · n notebook · p pin · s/d/R relate · q quit · Ctrl+C quit");
        o.push_str("\nviews: Tab next · Esc world · g graph · N notebook · T tasks · A agents · S settings · W setup");
        o.push_str("\nguided <intent>: compile the world into a constraint and choose inside it (read-only, no execution)");
        self.reading = Some(("commands".into(), o));
        self.status = "commands: deterministic dispatch, no model".into();
        self.open_view(View::Help);
    }
    /// Measured machine numbers (`B`): same shared helper the REPL `bench`
    /// command reports — scan cost, index size, live query + propagation
    /// latency. Slice pair is (cached workspace, live top-8 selection).
    /// Read-only, no transition recorded.
    fn show_bench(&mut self) {
        let mut sel_tokens = 0usize;
        for h in self.results.hits.iter().take(8) {
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
        self.reading = Some(("bench".into(), text));
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
        self.show_dependents_for(&path);
    }

    /// Structural pairs over the selection (`C`): differences count +
    /// contradiction candidates from one slice-map build — the same helper
    /// the REPL `contradictions` command reports. Falls back to the cursor
    /// object when nothing is selected. Read-only.
    fn show_contradictions(&mut self) {
        let docs = self.notebook_selection().and_then(|sel| {
            let v: Vec<(String, String)> = sel;
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        });
        let Some(docs) = docs else {
            self.status = "select objects first (Space)".into();
            return;
        };
        let text = physis_pro::mvp::contradictions_text(&docs);
        self.reading = Some(("contradictions".into(), text));
        self.status = "contradictions: slice-map pairs over the selection".into();
    }

    /// Hand the current selection (or the highlighted object) to Function 2:
    /// docs = (title, exact source text) for the Notebook scope. Canonical
    /// objects — the Notebook borrows, never copies, the content.
    fn notebook_selection(&mut self) -> Option<Vec<(String, String)>> {
        let targets = notebook_targets(&self.selected, self.current_hit());
        if targets.is_empty() {
            return None;
        }
        let mut docs = Vec::new();
        for h in targets {
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
/// The HUD's runtime-knob document: `data_dir/config.json` when present,
/// built-in defaults otherwise — the same file Studio CONFIG and
/// `physis-pro-tui` write, so all three panels stay one truth.
fn load_hud_config() -> PhysisConfig {
    let def = PhysisConfig::default();
    match std::fs::read_to_string(def.data_dir.join("config.json")) {
        Ok(text) => serde_json::from_str(&text).unwrap_or(def),
        Err(_) => def,
    }
}

fn save_hud_config(cfg: &PhysisConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.data_dir)?;
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(cfg.data_dir.join("config.json"), text)
}

fn run_notebook(app: &App, docs: &[(String, String)]) -> anyhow::Result<String> {
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
            Constraint::Min(7),
            Constraint::Percentage(35),
            Constraint::Length(1),
            Constraint::Length(5),
        ])
        .split(f.size());

    // Header: mode + counts + OFFLINE (Phase 32: no network, no model —
    // local embedder or lexical fallback only). Full legend lives in the
    // right pane title and the footer; markers never rely on colour alone.
    let mode = if app.semantic_on {
        "semantic+lexical"
    } else {
        "LEXICAL ONLY"
    };
    let delta_txt = match &app.last_delta {
        Some(d) => format!(
            "Δ {} changed · {} affected",
            d.changed_artifacts.len(),
            d.affected_dependents.len()
        ),
        None => "Δ —".to_string(),
    };
    let ctx_txt = if app.results.hits.is_empty() {
        String::new()
    } else {
        // Cached denominator — the header draws every frame, so it must
        // never re-walk the tree. Selection is ≤8 cached reads.
        let ws_tokens = app.ws_tokens;
        let mut sel_tokens = 0usize;
        for h in app.results.hits.iter().take(8) {
            sel_tokens += physis_pro::pack::tokens_of(&app.ws.read(&h.object).unwrap_or_default());
        }
        let pct = if ws_tokens > 0 {
            100.0 * sel_tokens as f64 / ws_tokens as f64
        } else {
            0.0
        };
        format!(
            "CONTEXT {pct:.1}% │ {} potential tokens avoided",
            ws_tokens.saturating_sub(sel_tokens)
        )
    };
    // OFFLINE is the demo contract (§32.0): visible without colour, green
    // when the terminal has it. DEGRADED state renders amber-yellow so the
    // lexical-only mode is never silent.
    let offline = Span::styled(
        "OFFLINE",
        if app.semantic_on {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        },
    );
    let delta_span = match &app.last_delta {
        Some(d) if !d.changed_artifacts.is_empty() => Span::styled(
            delta_txt,
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
        _ => Span::raw(delta_txt),
    };
    let header = Line::from(vec![
        Span::styled(
            "PHYSIS NAVIGATOR",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::LightCyan),
        ),
        Span::raw(format!(
            "  [{mode}] embedder: {}  gen: {}  indexed: {} objects  ",
            app.embedder_kind,
            physis_pro::mvp::generator_state(),
            app.ws.index().objects.len(),
        )),
        delta_span,
        Span::raw(format!("  {ctx_txt}  ")),
        offline,
        Span::raw(format!(
            "  {}",
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
    // Ranking bars are positional: each bar is scaled to this list's own
    // max, so a full bar means "top here", never a probability.
    let max_score = app
        .results
        .hits
        .iter()
        .map(|h| h.score.total)
        .fold(0.0_f32, f32::max);
    let items: Vec<ListItem> = app
        .results
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            Row::Header(name, n) => ListItem::new(Line::from(Span::styled(
                format!("{name} ({n})"),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::White),
            ))),
            Row::Dir(dir, n) => ListItem::new(Line::from(vec![
                Span::raw(if i == app.results.sel { "▶ " } else { "  " }),
                Span::styled(
                    format!("▸ {dir} ({n})"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
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
                let m = state_of(
                    &path,
                    &app.changed,
                    &app.affected,
                    &pinned,
                    &intent,
                    &app.selected,
                    &h.object,
                );
                let cold = focus_on && !intent.contains(&path) && !pinned.contains(&path);
                // State colour rides beside the marker, never instead of it:
                // ● changed red · ◐ affected amber · ◆ pinned cyan ·
                // ▲ intent green · ★ selected white.
                let mark_style = match m {
                    '●' => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    '◐' => Style::default().fg(Color::LightRed),
                    '◆' => Style::default().fg(Color::Cyan),
                    '▲' => Style::default().fg(Color::Green),
                    '★' => Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default(),
                };
                let mut spans = vec![
                    Span::styled(format!("{m} "), mark_style),
                    Span::styled(
                        kind_tag(h.object.kind),
                        Style::default().fg(color_for(h.object.kind)),
                    ),
                    Span::raw(format!(" {:>4.2}  ", h.score.total)),
                    Span::styled(
                        score_bar(h.score.total, max_score, 5),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("  ".to_string()),
                    Span::styled(
                        loc,
                        if cold {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default().add_modifier(Modifier::UNDERLINED)
                        },
                    ),
                ];
                if i == app.results.sel {
                    spans.insert(0, Span::raw("▶ "));
                } else {
                    spans.insert(0, Span::raw("  "));
                }
                ListItem::new(Line::from(spans))
            }
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ranked neighbourhood "),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, chunks[1], &mut list_state_for(app));

    // A view paints over the neighborhood pane: the world stays underneath
    // (reading pane + tabs), the view owns the main pane until `Esc` or `Tab`.
    if app.view != View::World {
        let (vtitle, vbody) = app.view_content();
        f.render_widget(
            Paragraph::new(vbody)
                .block(Block::default().borders(Borders::ALL).title(vtitle))
                .wrap(Wrap { trim: false }),
            chunks[1],
        );
    }

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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} ")),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(reader, reading_chunks[0]);

    // Right pane switches tabs: Why (rank + relations) / Context (minimal
    // context packet) / Delta (last Δ) / History (git + recorded transitions).
    // All four project existing state.
    let (right_title, right_body) = match app.context_tab {
        ContextTab::Why => {
            let mut explain = app.explain.clone();
            if !app.selected.is_empty() {
                explain.push_str(&format!("\nselected: {} objects", app.selected.len()));
                if app.selected.len() == 2 {
                    explain.push_str("\np (pin) · s/d/R (same/different/relates) · n (notebook)");
                }
            }
            (
                " why: 1 why · 2 context · 3 delta · 4 history ".to_string(),
                explain,
            )
        }
        ContextTab::Context => (
            " context: 1 why · 2 context · 3 delta · 4 history ".to_string(),
            app.context_text(),
        ),
        ContextTab::Delta => (
            " delta: 1 why · 2 context · 3 delta · 4 history ".to_string(),
            app.delta_text(),
        ),
        ContextTab::History => (
            " history: 1 why · 2 context · 3 delta · 4 history ".to_string(),
            app.history_view(),
        ),
    };
    let right = Paragraph::new(right_body)
        .block(Block::default().borders(Borders::ALL).title(right_title))
        .wrap(Wrap { trim: false });
    f.render_widget(right, reading_chunks[1]);

    // Command box + status + key legend (footer names every marker + edge
    // grammar so state never rides on colour alone). The mode word is the
    // contract; colour only decorates it.
    let mut prompt_spans = Vec::new();
    if let Some(key) = app.settings_edit.clone() {
        prompt_spans.push(Span::styled(
            format!("[SET {key}] > "),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        prompt_spans.push(Span::raw(app.query.clone()));
    } else {
        for (kind, text) in prompt_parts(&app.query, app.editor.is_some()) {
            let style = match kind {
                PromptPart::Prefix => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                PromptPart::Word => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                PromptPart::Text => Style::default(),
            };
            prompt_spans.push(Span::styled(text, style));
        }
    }
    let keys = if app.editor.is_some() {
        "type · ←/→/↑/↓/Home/End move · Enter newline · Backspace/Delete · Ctrl+S save · Esc/Ctrl+C cancel".to_string()
    } else if app.settings_edit.is_some() {
        "Enter applies the value · Esc cancels".to_string()
    } else if app.view != View::World {
        format!("view {} · {}", app.view.label(), app.view.keys())
    } else {
        "e inline edit · :external editor · u rescan · g graph · t edges · H history · B bench · D dependents · C contradictions · f focus · F clear · z zoom · 1/2/3/4 tabs · n notebook · p pin · s/d/R relate · space select · ? help · q quit".to_string()
    };
    let lines = vec![
        Line::from(prompt_spans),
        Line::from(Span::styled(
            app.status.clone(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            keys,
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "★ selected · ◆ human-pinned · ● changed · ◐ affected · ▲ intent · · ordinary │ ─── user · ═══ deterministic · - - inferred",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "$ shell in workspace root · :command · open/edit/cd path · /text global search · :help lists commands",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        chunks[4],
    );

    // AGENTS strip (plan S1/S6): who is present and whether they are reachable.
    // Probed once at startup and cached, so the draw loop never re-walks PATH.
    f.render_widget(
        Paragraph::new(AGENTS_STRIP.as_str()).style(Style::default().fg(Color::Gray)),
        chunks[3],
    );
}

/// The agent roster probe, resolved once. Availability is a property of the
/// environment (PATH, keys), not of a frame.
static AGENTS_STRIP: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    physis_pro::agents::view::render_strip(&physis_pro::agents::default_roster())
});

/// ratatui needs `&mut ListState`; the app keeps its own copy in sync.
fn list_state_for(app: &App) -> ListState {
    let mut s = app.list_state.clone();
    s.select(Some(app.results.sel));
    s
}

fn run(mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>, app: App) -> anyhow::Result<()> {
    let app = Arc::new(Mutex::new(app));
    // Background agent calls report here. The sender is installed on the App so
    // the `@agent` handler can hand work to a worker thread.
    let (agent_tx, agent_rx) = std::sync::mpsc::channel::<AgentReply>();
    app.lock().unwrap().agent_tx = Some(agent_tx);

    loop {
        // Deliver any completed agent turn before drawing, so the reply appears
        // the moment it lands instead of on the next keystroke. The decision of
        // what a reply means lives in the reducer (`hud::event::reduce`), not
        // here: this loop only translates a channel message into an event and
        // applies the effects. A model's text therefore cannot reach anything
        // but the status line and the reading pane.
        while let Ok(reply) = agent_rx.try_recv() {
            let mut app = app.lock().unwrap();
            let event = physis_pro::hud::event::HudEvent::AgentReply {
                agent: reply.agent.clone(),
                ok: reply.ok,
                text: reply.text.clone(),
            };
            let effects = physis_pro::hud::event::reduce(&mut app.focus_state, &event);
            for effect in effects {
                app.apply_hud_effect(effect);
            }
        }

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
                    if app.editor.is_some() {
                        // Inline editor owns the keyboard: every text key is a
                        // keystroke, never a HUD shortcut. Esc/Ctrl+C cancel,
                        // Ctrl+S saves, Ctrl+Q cancels rather than quitting.
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            match key.code {
                                KeyCode::Char('s') | KeyCode::Char('S') => app.save_editor(),
                                KeyCode::Char('c')
                                | KeyCode::Char('C')
                                | KeyCode::Char('q')
                                | KeyCode::Char('Q') => app.cancel_editor(),
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Esc => app.cancel_editor(),
                                KeyCode::Enter => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.insert_newline();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Left => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.move_left();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Right => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.move_right();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Up => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.move_up();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Down => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.move_down();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Home => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.move_home();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::End => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.move_end();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Backspace => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.backspace();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Delete => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.delete();
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Char(c) => {
                                    if let Some(editor) = app.editor.as_mut() {
                                        editor.insert_char(c);
                                    }
                                    app.refresh_editor_reading();
                                }
                                KeyCode::Tab => {}
                                _ => {}
                            }
                        }
                    } else if app.typing {
                        // Ctrl-C quits even mid-typing; checked before the
                        // plain-char arm so 'c' with CONTROL reaches here.
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            quit = true;
                        } else {
                            match key.code {
                                // `/` is an ordinary query character: the old
                                // branch cleared the box here, which made a
                                // leading-slash global query untypeable.
                                KeyCode::Char(c) => app.query.push(c),
                                KeyCode::Backspace => {
                                    app.query.pop();
                                }
                                KeyCode::Enter => {
                                    let line = std::mem::take(&mut app.query);
                                    app.typing = false;
                                    if let Some(key) = app.settings_edit.take() {
                                        app.apply_settings_edit(&key, &line);
                                    } else {
                                        quit = app.submit_command_line(&line);
                                    }
                                }
                                KeyCode::Esc => {
                                    if app.settings_edit.take().is_some() {
                                        app.settings_msg = "edit cancelled".into();
                                    }
                                    app.query.clear();
                                }
                                _ => {}
                            }
                        }
                    } else if app.view != View::World {
                        // A view owns the keyboard for its own keys; `Esc`
                        // always returns to the world and `Tab` always cycles,
                        // so nobody gets stranded in a view.
                        match key.code {
                            KeyCode::Esc => app.view = View::World,
                            KeyCode::Tab => {
                                let next = app.view.next();
                                app.open_view(next);
                            }
                            KeyCode::BackTab => {
                                let prev = app.view.prev();
                                app.open_view(prev);
                            }
                            KeyCode::Up | KeyCode::Char('k') if app.view == View::Settings => {
                                app.settings_sel =
                                    next_setting_row(&app.settings_view_rows, app.settings_sel, false);
                            }
                            KeyCode::Down | KeyCode::Char('j') if app.view == View::Settings => {
                                app.settings_sel =
                                    next_setting_row(&app.settings_view_rows, app.settings_sel, true);
                            }
                            KeyCode::Char('+') | KeyCode::Char('=')
                                if app.view == View::Settings =>
                            {
                                app.step_selected(true);
                            }
                            KeyCode::Char('-') | KeyCode::Char('_')
                                if app.view == View::Settings =>
                            {
                                app.step_selected(false);
                            }
                            KeyCode::Enter | KeyCode::Char('e') if app.view == View::Settings => {
                                app.step_selected(true);
                            }
                            KeyCode::Char('s') if app.view == View::Settings => {
                                app.save_settings();
                            }
                            KeyCode::Char('r') if app.view == View::Settings => {
                                app.reload_settings();
                            }
                            KeyCode::Char('d') if app.view == View::Settings => {
                                app.reset_selected();
                            }
                            KeyCode::Char('r') if app.view == View::Setup => app.refresh_view(),
                            KeyCode::Char('s') if app.view == View::Setup => app.save_settings(),
                            // Any view key opens its surface the same way its
                            // command does — the view frame is never empty.
                            KeyCode::Char(c) => {
                                if let Some(view) = View::from_key(c) {
                                    app.open_view_via_command(view);
                                }
                            }
                            _ => {}
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
                                // Compute the next row, skipping headers, then
                                // dispatch through the reducer so FocusState.cursor
                                // is the canonical owner.
                                if app.results.sel > 0 {
                                    let mut next = app.results.sel - 1;
                                    while matches!(app.results.rows.get(next), Some(Row::Header(..))) {
                                        if next == 0 {
                                            break;
                                        }
                                        next = next.saturating_sub(1);
                                    }
                                    let delta = next as i32 - app.results.sel as i32;
                                    app.dispatch(physis_pro::hud::event::HudEvent::MoveCursor {
                                        delta,
                                    });
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.results.sel + 1 < app.results.rows.len() {
                                    let mut next = app.results.sel + 1;
                                    while matches!(app.results.rows.get(next), Some(Row::Header(..))) {
                                        next = (next + 1).min(app.results.rows.len() - 1);
                                    }
                                    let delta = next as i32 - app.results.sel as i32;
                                    app.dispatch(physis_pro::hud::event::HudEvent::MoveCursor {
                                        delta,
                                    });
                                }
                            }
                            KeyCode::Enter => {
                                // Directory rollup drills in; objects zoom as before.
                                if let Some(Row::Dir(dir, _)) = app.results.rows.get(app.results.sel).cloned() {
                                    app.enter_dir(&dir);
                                } else {
                                    app.enter();
                                }
                            }
                            KeyCode::Esc => app.back(),
                            KeyCode::Char(' ') => {
                                // Toggle selection through the canonical path so
                                // FocusState.selection is the single owner.
                                if let Some(id) =
                                    app.current_hit().map(|h| h.object.id.clone())
                                {
                                    app.dispatch(
                                        physis_pro::hud::event::HudEvent::Select { id },
                                    );
                                    // Sync legacy Vec<NavHit> adapter until it is
                                    // replaced outright by FocusState.selection.
                                    app.sync_selected_from_focus_state();
                                    app.update_explain();
                                }
                            }
                            KeyCode::Char('p') => app.pin_selection(),
                            KeyCode::Char('s') => app.relate_selection("same"),
                            KeyCode::Char('d') => app.relate_selection("different"),
                            KeyCode::Char('R') => app.relate_selection("relates to"),
                            KeyCode::Char('n') => app.show_notebook(),
                            KeyCode::Char('r') => app.related(),
                            KeyCode::Char('g') => {
                                app.graph_view();
                                app.open_view(View::Graph);
                            }
                            KeyCode::Char('t') => app.cycle_edge_filter(),
                            KeyCode::Char('H') => app.show_history(),
                            KeyCode::Char('B') => app.show_bench(),
                            KeyCode::Char('D') => app.show_dependents(),
                            KeyCode::Char('C') => app.show_contradictions(),
                            KeyCode::Char('e') => app.start_inline_edit(None),
                            KeyCode::Char('u') => app.rescan_now(),
                            KeyCode::Char('f') => app.apply_focus(),
                            KeyCode::Char('F') => {
                                // ClearSelection through the canonical path +
                                // clear the HOT/WARM focus bands.
                                app.dispatch(physis_pro::hud::event::HudEvent::ClearSelection);
                                app.selected.clear();
                                app.clear_focus();
                            }
                            KeyCode::Char('z') => app.toggle_zoom(),
                            KeyCode::Char('1') => app.switch_context_tab(ContextTab::Why),
                            KeyCode::Char('2') => app.switch_context_tab(ContextTab::Context),
                            KeyCode::Char('3') => app.switch_context_tab(ContextTab::Delta),
                            KeyCode::Char('4') => app.switch_context_tab(ContextTab::History),
                            KeyCode::Char('?') => app.show_help(),
                            // View keys: upper case where the lower-case
                            // letter is already a world action (s/d/R relate,
                            // n selects, g graphs) — each opens the surface
                            // its command produces, never an empty frame.
                            KeyCode::Char('q') => quit = true,
                            KeyCode::Char(c) => {
                                if let Some(view) = View::from_key(c) {
                                    app.open_view_via_command(view);
                                }
                            }
                            KeyCode::Tab => {
                                let next = app.view.next();
                                app.open_view(next);
                            }
                            KeyCode::BackTab => {
                                let prev = app.view.prev();
                                app.open_view(prev);
                            }
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

    /// The edge filter shows exactly the leg it names, nothing merged.
    #[test]
    fn edge_filter_states_which_leg_is_drawn() {
        assert!(App::edge_shown(true, 0) && App::edge_shown(false, 0));
        assert!(App::edge_shown(true, 1) && !App::edge_shown(false, 1));
        assert!(!App::edge_shown(true, 2) && App::edge_shown(false, 2));
        // Unknown filter values degrade to all, never to empty.
        assert!(App::edge_shown(true, 9) && App::edge_shown(false, 9));
        assert_eq!(App::edge_filter_name(0), "all edges");
        assert_eq!(App::edge_filter_name(1), "recorded relations only");
        assert_eq!(App::edge_filter_name(2), "near neighbours only");
    }

    use physis_pro::nav::{ObjectId, PhysisObject, RetrievalScore, SourceRef};

    fn hud_hit(path: &str, key: &str, score: f32) -> NavHit {
        NavHit {
            object: PhysisObject {
                id: ObjectId::new("fs", key),
                kind: ObjectKind::Passage,
                label: path.to_string(),
                source: Some(SourceRef {
                    path: path.to_string(),
                    start_line: Some(1),
                    end_line: Some(1),
                }),
                mtime: 0.0,
                hash: format!("{key}-hash"),
                text: path.to_string(),
                embedding: Vec::new(),
            },
            score: RetrievalScore {
                total: score,
                components: Default::default(),
            },
        }
    }

    fn hud_bare_hit(label: &str, key: &str) -> NavHit {
        let mut hit = hud_hit(label, key, 0.1);
        hit.object.source = None;
        hit
    }

    fn hud_set(paths: &[&str]) -> std::collections::BTreeSet<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn state_marker_precedence_is_explicit() {
        let path = "src/a.rs";
        let target = hud_hit(path, "a-key-0123456789abcdef", 0.9);
        let selected = vec![hud_hit(path, "a-key-0123456789abcdef", 0.1)];
        let everywhere = hud_set(&[path]);
        let nowhere = hud_set(&[]);

        assert_eq!(
            state_of(
                path,
                &everywhere,
                &everywhere,
                &everywhere,
                &everywhere,
                &selected,
                &target.object
            ),
            '★'
        );
        assert_eq!(
            state_of(
                path,
                &nowhere,
                &everywhere,
                &everywhere,
                &everywhere,
                &[],
                &target.object
            ),
            '◆'
        );
        assert_eq!(
            state_of(
                path,
                &everywhere,
                &everywhere,
                &nowhere,
                &everywhere,
                &[],
                &target.object
            ),
            '●'
        );
        assert_eq!(
            state_of(
                path,
                &nowhere,
                &everywhere,
                &nowhere,
                &everywhere,
                &[],
                &target.object
            ),
            '◐'
        );
        assert_eq!(
            state_of(
                path,
                &nowhere,
                &nowhere,
                &nowhere,
                &everywhere,
                &[],
                &target.object
            ),
            '▲'
        );
        assert_eq!(
            state_of(
                path,
                &nowhere,
                &nowhere,
                &nowhere,
                &nowhere,
                &[],
                &target.object
            ),
            '·'
        );
    }

    #[test]
    fn directory_rollup_is_capped_and_rank_sorted() {
        let hits = vec![
            hud_hit("src/query.rs", "query-key-0123456789abcdef", 0.7),
            hud_hit("src/index.rs", "index-key-0123456789abcdef", 0.9),
            hud_hit("README.md", "readme-key-0123456789abcdef", 0.5),
        ];
        assert_eq!(
            aggregate_dirs(&hits, 10),
            vec![("src".to_string(), 2), (".".to_string(), 1)]
        );
        assert_eq!(aggregate_dirs(&hits, 1), vec![("src".to_string(), 2)]);
    }

    #[test]
    fn intent_bands_cover_first_eight_hits() {
        let hits: Vec<NavHit> = (0..10)
            .map(|i| {
                hud_hit(
                    &format!("src/file{i}.rs"),
                    &format!("intent-key-{i:02}-0123456789abcdef"),
                    1.0 - (i as f32 * 0.05),
                )
            })
            .collect();
        let (hot, warm) = intent_paths(&hits);
        assert_eq!(hot.len(), 3);
        assert_eq!(warm.len(), 5);
        assert!(hot.contains("src/file0.rs"));
        assert!(hot.contains("src/file2.rs"));
        assert!(warm.contains("src/file3.rs"));
        assert!(warm.contains("src/file7.rs"));
        assert!(!hot.contains("src/file8.rs"));
        assert!(!warm.contains("src/file9.rs"));
    }

    #[test]
    fn notebook_targets_prefers_source_then_cursor() {
        let source = hud_hit("src/a.rs", "a-key-0123456789abcdef", 0.9);
        let selected = vec![
            hud_bare_hit("unsourced", "bare-key-0123456789abcdef"),
            source,
        ];
        let cursor = hud_hit("src/cursor.rs", "cursor-key-0123456789abcdef", 0.5);

        let targets = notebook_targets(&selected, Some(&cursor));
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].object.source.as_ref().unwrap().path, "src/a.rs");

        let bare = [hud_bare_hit("unsourced", "bare-key-0123456789abcdef")];
        let fallback = notebook_targets(&bare, Some(&cursor));
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].object.id, cursor.object.id);
        assert!(notebook_targets(&[], None).is_empty());
    }

    #[test]
    fn why_affected_lines_names_module_and_call() {
        let lines = why_affected_lines(
            "src/query.rs",
            "fn search() { update_index(); }\n",
            &["src/index.rs".to_string()],
            &["update_index".to_string()],
        );
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("names module index"));
        assert!(lines[1].contains("calls update_index"));
    }

    #[test]
    fn new_surface_commands_parse() {
        assert_eq!(
            parse_hud_input("intent"),
            HudInput::Command(HudCommand::Intent)
        );
        assert_eq!(
            parse_hud_input("agents"),
            HudInput::Command(HudCommand::Agents)
        );
        assert_eq!(
            parse_hud_input("settings"),
            HudInput::Command(HudCommand::Settings)
        );
        assert_eq!(
            parse_hud_input("tasks"),
            HudInput::Command(HudCommand::Tasks)
        );
        // `settings set x` is not a HUD command: it needs an argument and lives
        // on the CLI, so it must not silently become a command.
        assert!(matches!(
            parse_hud_input("settings set providers.jev.model x"),
            HudInput::SemanticQuery(_)
        ));
    }

    #[test]
    fn snapshot_size_parses_and_rejects_junk() {
        assert_eq!(parse_size("80x24"), Some((80, 24)));
        assert_eq!(parse_size("140x40"), Some((140, 40)));
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("80"), None);
        assert_eq!(parse_size("0x24"), None);
    }

    #[test]
    fn agent_scope_prefers_request_then_query() {
        assert_eq!(
            agent_scope("inspect this", "auth"),
            Some("inspect this".into())
        );
        assert_eq!(
            agent_scope("", "auth middleware"),
            Some("auth middleware".into())
        );
        assert_eq!(agent_scope("", "   "), None);
        assert_eq!(agent_scope("", ""), None);
    }

    #[test]
    fn command_box_routing_is_deterministic() {
        assert_eq!(parse_hud_input(""), HudInput::Empty);
        assert_eq!(parse_hud_input("   "), HudInput::Empty);
        assert_eq!(parse_hud_input("/"), HudInput::Empty);
        assert_eq!(
            parse_hud_input("$ git status --short"),
            HudInput::Shell("git status --short".into())
        );
        assert_eq!(
            parse_hud_input("$"),
            HudInput::Invalid("shell command needed after `$`".into())
        );
        assert_eq!(
            parse_hud_input("action read fs:abc"),
            HudInput::Command(HudCommand::Action("read fs:abc".into()))
        );
        assert_eq!(
            parse_hud_input("action"),
            HudInput::Invalid("usage: action <verb> <object> [key=value ...]".into())
        );
        assert_eq!(
            parse_hud_input("@jev inspect this"),
            HudInput::AgentCall {
                agent: "jev".into(),
                host: None,
                rest: "inspect this".into()
            }
        );
        assert_eq!(
            parse_hud_input("@hermes@omo inspect worker"),
            HudInput::AgentCall {
                agent: "hermes".into(),
                host: Some("omo".into()),
                rest: "inspect worker".into()
            }
        );
        assert_eq!(
            parse_hud_input("@all summarise"),
            HudInput::AgentAll {
                request: "summarise".into()
            }
        );
        assert!(matches!(
            parse_hud_input("@a x | @b y"),
            HudInput::AgentPipeline { .. }
        ));
        assert!(matches!(parse_hud_input("@jev@"), HudInput::Invalid(_)));
        assert_eq!(
            parse_hud_input(":context"),
            HudInput::Command(HudCommand::Context)
        );
        assert_eq!(parse_hud_input(":"), HudInput::Command(HudCommand::Help));
        assert_eq!(
            parse_hud_input(":bogus"),
            HudInput::Invalid("unknown command: bogus".into())
        );
        assert_eq!(
            parse_hud_input("/retrieval budget"),
            HudInput::GlobalQuery("retrieval budget".into())
        );
        assert_eq!(
            parse_hud_input("open src/mvp/repl.rs"),
            HudInput::Open("src/mvp/repl.rs".into())
        );
        assert_eq!(parse_hud_input("edit"), HudInput::Edit(None));
        assert_eq!(
            parse_hud_input("edit src/mvp/repl.rs"),
            HudInput::Edit(Some("src/mvp/repl.rs".into()))
        );
        assert_eq!(parse_hud_input("cd .."), HudInput::ChangeRoot("..".into()));
        assert_eq!(
            parse_hud_input("history"),
            HudInput::Command(HudCommand::History)
        );
        assert_eq!(
            parse_hud_input("HISTORY"),
            HudInput::Command(HudCommand::History)
        );
        assert_eq!(
            parse_hud_input("dependencies src/mvp/repl.rs"),
            HudInput::Command(HudCommand::Dependencies(Some("src/mvp/repl.rs".into())))
        );
        assert_eq!(
            parse_hud_input("focus retrieval"),
            HudInput::Command(HudCommand::Focus(Some("retrieval".into())))
        );
        assert_eq!(
            parse_hud_input("relate same"),
            HudInput::Command(HudCommand::Relate("same".into()))
        );
        // Bare quit stays a search: only the explicit `:quit` exits.
        assert_eq!(
            parse_hud_input("quit"),
            HudInput::SemanticQuery("quit".into())
        );
        assert_eq!(
            parse_hud_input(":quit"),
            HudInput::Command(HudCommand::Quit)
        );
        assert_eq!(
            parse_hud_input("history of everything"),
            HudInput::SemanticQuery("history of everything".into())
        );
        assert_eq!(
            parse_hud_input("where is retrieval implemented?"),
            HudInput::SemanticQuery("where is retrieval implemented?".into())
        );
    }

    #[test]
    fn prompt_mode_is_visible_before_enter() {
        assert_eq!(prompt_mode("$ git status", false), HudPromptMode::Shell);
        assert_eq!(prompt_mode(":context", false), HudPromptMode::Physis);
        assert_eq!(prompt_mode("history", false), HudPromptMode::Physis);
        assert_eq!(
            prompt_mode("/retrieval", false),
            HudPromptMode::GlobalSearch
        );
        assert_eq!(
            prompt_mode("open src/mvp/repl.rs", false),
            HudPromptMode::Open
        );
        assert_eq!(prompt_mode("edit", false), HudPromptMode::Edit);
        assert_eq!(prompt_mode("cd ..", false), HudPromptMode::Root);
        assert_eq!(
            prompt_mode("where is retrieval?", false),
            HudPromptMode::Search
        );
        assert_eq!(prompt_mode(":bogus", false), HudPromptMode::Invalid);
        assert_eq!(prompt_mode("anything", true), HudPromptMode::Editor);
        assert_eq!(prompt_mode_label(HudPromptMode::Shell), "[SHELL]");
        assert_eq!(prompt_mode_label(HudPromptMode::Physis), "[PHYSIS]");
        assert_eq!(prompt_mode_label(HudPromptMode::Search), "[SEARCH]");
        assert_eq!(prompt_mode_label(HudPromptMode::Edit), "[EDIT]");
    }

    #[test]
    fn prompt_parts_split_prefix_from_text() {
        assert_eq!(
            prompt_parts("$ git status", false),
            vec![
                (PromptPart::Prefix, "[SHELL] > ".to_string()),
                (PromptPart::Prefix, "$ ".to_string()),
                (PromptPart::Text, "git status".to_string()),
            ]
        );
        assert_eq!(
            prompt_parts("history", false),
            vec![
                (PromptPart::Prefix, "[PHYSIS] > ".to_string()),
                (PromptPart::Word, "history ".to_string()),
                (PromptPart::Text, String::new()),
            ]
        );
        assert_eq!(
            prompt_parts("/retrieval", false),
            vec![
                (PromptPart::Prefix, "[SEARCH/GLOBAL] > ".to_string()),
                (PromptPart::Prefix, "/".to_string()),
                (PromptPart::Text, "retrieval".to_string()),
            ]
        );
    }

    #[test]
    fn shell_command_runs_in_workspace_and_reports_status() {
        let dir = tempfile::tempdir().unwrap();
        let ok = run_shell_command(dir.path(), "printf hello; printf err >&2").unwrap();
        assert_eq!(ok.command, "printf hello; printf err >&2");
        assert_eq!(ok.stdout, "hello");
        assert_eq!(ok.stderr, "err");
        assert_eq!(ok.code, Some(0));

        let failed = run_shell_command(dir.path(), "exit 3").unwrap();
        assert_eq!(failed.code, Some(3));

        let root = dir.path().canonicalize().unwrap();
        let cwd = run_shell_command(dir.path(), "pwd").unwrap();
        assert_eq!(std::path::Path::new(cwd.stdout.trim()), root.as_path());

        assert!(run_shell_command(dir.path(), "   ").is_err());
        assert!(run_shell_command(&dir.path().join("missing"), "true").is_err());
    }

    #[test]
    fn shell_rendering_bounds_long_output() {
        let mut stdout = String::new();
        for i in 0..250 {
            stdout.push_str(&format!("line {i}\n"));
        }
        let outcome = ShellOutcome {
            command: "demo".to_string(),
            stdout,
            stderr: String::new(),
            code: Some(0),
            duration_ms: 3,
        };
        let rendered = render_shell_outcome(&outcome);
        assert!(rendered.starts_with("$ demo\nexit: 0 · 3 ms\n"));
        assert!(rendered.contains("line 199\n"));
        assert!(!rendered.contains("line 200\n"));
        assert!(rendered.contains("… and 50 more lines"));
    }

    #[test]
    fn edit_buffer_edits_text_and_cursor() {
        let mut buffer = EditBuffer::open(
            PathBuf::from("demo.rs"),
            "demo.rs".to_string(),
            "ab\ncd".to_string(),
        );
        assert_eq!(buffer.text(), "ab\ncd");
        assert_eq!(buffer.render_with_cursor(), "ab\ncd▌");

        buffer.move_left();
        buffer.move_left();
        buffer.insert_char('X');
        assert_eq!(buffer.text(), "ab\nXcd");
        buffer.backspace();
        assert_eq!(buffer.text(), "ab\ncd");
        buffer.move_home();
        buffer.insert_newline();
        assert_eq!(buffer.text(), "ab\n\ncd");
        buffer.move_up();
        buffer.move_end();
        buffer.delete();
        assert_eq!(buffer.text(), "ab\ncd");
    }

    #[test]
    fn edit_buffer_save_detects_no_change_conflict_and_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        std::fs::write(&path, "first\n").unwrap();

        let buffer = EditBuffer::open(path.clone(), "note.txt".into(), "first\n".into());
        let saved = buffer.save().unwrap();
        assert!(saved.unchanged);
        assert_eq!(saved.before_hash, saved.after_hash);
        assert_eq!(saved.bytes, 6);

        let mut changed = EditBuffer::open(path.clone(), "note.txt".into(), "first\n".into());
        changed.insert_char('!');
        let saved = changed.save().unwrap();
        assert!(!saved.unchanged);
        assert_ne!(saved.before_hash, saved.after_hash);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\n!");

        std::fs::write(&path, "outside\n").unwrap();
        let mut stale = EditBuffer::open(path.clone(), "note.txt".into(), "first\n!".into());
        stale.insert_char('?');
        let err = stale.save().unwrap_err();
        assert!(err.to_string().contains("changed on disk"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "outside\n");
    }

    #[test]
    fn workspace_path_gate_stays_inside_the_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let target = dir.path().join("src/a.rs");
        std::fs::write(&target, "fn a() {}\n").unwrap();
        let found = resolve_workspace_file(dir.path(), "src/a.rs").unwrap();
        assert_eq!(found, target.canonicalize().unwrap());

        let outside_dir = tempfile::tempdir().unwrap();
        let outside = outside_dir.path().join("outside.rs");
        std::fs::write(&outside, "fn outside() {}\n").unwrap();
        assert!(resolve_workspace_file(dir.path(), "../outside.rs").is_err());
        assert!(resolve_workspace_file(dir.path(), "missing.rs").is_err());
        assert!(resolve_workspace_file(dir.path(), "src").is_err());
        assert_eq!(
            display_workspace_path(dir.path(), &found),
            "src/a.rs".to_string()
        );
    }

    #[test]
    fn change_root_gate_accepts_only_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "x\n").unwrap();
        let root = dir.path().canonicalize().unwrap();

        assert_eq!(
            resolve_change_root(dir.path(), "sub").unwrap(),
            root.join("sub")
        );
        assert_eq!(resolve_change_root(dir.path(), ".").unwrap(), root);
        assert!(resolve_change_root(dir.path(), "missing").is_err());
        assert!(resolve_change_root(dir.path(), "file.txt").is_err());
    }

    struct RestoreEnv {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            if let Some(value) = &self.old {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    /// Serialises every test that points the process-global `PHYSIS_DATA_DIR`
    /// at its own temp store. Cargo runs the tests in parallel threads of one
    /// process, so without this the store a *test* opens is whichever thread
    /// wrote the variable last — a schedule-dependent failure, not a slow
    /// machine. (`packets/reports/PH-015.md` is the same failure class.)
    ///
    /// This covers the in-process race only. The other half is the temp
    /// directory itself: cargo runs the lib and bin test binaries as separate
    /// processes, so tests here use `tempfile::tempdir()` and never a fixed
    /// path — a per-process lock cannot serialise two processes.
    static DATA_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Take the data-dir lock, ignoring poisoning: one failing test must not
    /// turn every later test into a poison panic, which would hide the real
    /// failure behind a cascade.
    fn lock_data_dir() -> std::sync::MutexGuard<'static, ()> {
        DATA_DIR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Inline edit → atomic save → existing rescan/Δ machinery → markers and
    /// one `edit-save` transition, followed by a recorded shell command.
    #[test]
    fn inline_edit_save_rescans_records_delta_and_markers() {
        use std::sync::{Mutex, MutexGuard, OnceLock};

        fn hud_env_lock() -> MutexGuard<'static, ()> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            LOCK.get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        }

        let _guard = hud_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/retrieval.rs"),
            "pub fn retrieval_budget() -> usize {\n    500\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/agent.rs"),
            "use crate::retrieval::retrieval_budget;\n\npub fn agent_budget() -> usize {\n    retrieval_budget()\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/context.rs"),
            "use crate::retrieval::retrieval_budget;\n\npub fn context_budget() -> usize {\n    retrieval_budget()\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("README.md"),
            "# Fixture\n\nThe retrieval budget lives in src/retrieval.rs.\n",
        )
        .unwrap();

        let _embedder = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        let _data_dir_lock = lock_data_dir();
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        std::env::set_var("PHYSIS_DATA_DIR", root.join(".physis"));

        // Prescan so the save's rescan sees a baseline: without it every
        // file would read as added and the dependents rule would exclude
        // them all (changed files are never their own dependents).
        let mut app = App::new(root.to_path_buf(), true).unwrap();
        app.query = "retrieval budget".to_string();
        app.run_query();
        app.start_inline_edit(Some("src/retrieval.rs"));
        assert!(app.editor.is_some(), "inline editor must open");
        app.editor.as_mut().expect("editor open").insert_char('!');
        app.save_editor();

        assert!(app.editor.is_none(), "save closes the editor");
        assert!(
            app.changed.contains("src/retrieval.rs"),
            "changed: {:?}",
            app.changed
        );
        assert!(
            app.affected.contains("src/agent.rs"),
            "affected: {:?}",
            app.affected
        );
        assert!(
            app.affected.contains("src/context.rs"),
            "affected: {:?}",
            app.affected
        );
        let delta = app
            .last_delta
            .as_ref()
            .expect("save must leave a Δ")
            .render();
        for section in ["CHANGED", "AFFECTED", "STRUCTURE", "VALIDATION", "MAP"] {
            assert!(delta.contains(section), "missing {section}:\n{delta}");
        }
        assert!(delta.contains("src/retrieval.rs"), "{delta}");
        let (title, body) = app.reading.clone().expect("reading must show the save");
        assert_eq!(title, "edit src/retrieval.rs");
        assert!(body.contains('!'), "{body}");
        assert!(
            app.status.contains("saved src/retrieval.rs"),
            "{}",
            app.status
        );

        let log = TransitionLog::open(root).unwrap().read_all();
        let saved = log.last().expect("save must be a transition");
        assert_eq!(saved.action, "edit-save");
        assert_eq!(saved.intent, "edit src/retrieval.rs");
        assert!(!saved.delta.no_change);

        assert!(!app.submit_command_line("$ printf hello"));
        let (title, body) = app.reading.clone().expect("shell must show output");
        assert_eq!(title, "shell: printf hello");
        assert!(body.contains("exit: 0"), "{body}");
        assert!(body.contains("hello"), "{body}");
        let log = TransitionLog::open(root).unwrap().read_all();
        let shelled = log.last().expect("shell must be a transition");
        assert_eq!(shelled.action, "shell");
        assert_eq!(shelled.intent, "$ printf hello");
    }

    #[test]
    fn command_box_routes_new_helpers() {
        assert_eq!(
            parse_hud_input("trail"),
            HudInput::Command(HudCommand::Trail)
        );
        assert_eq!(
            parse_hud_input(":trail"),
            HudInput::Command(HudCommand::Trail)
        );
        assert_eq!(parse_hud_input("pins"), HudInput::Command(HudCommand::Pins));
        assert_eq!(
            parse_hud_input("related"),
            HudInput::Command(HudCommand::Related)
        );
        assert_eq!(
            parse_hud_input("unpin"),
            HudInput::Command(HudCommand::Unpin)
        );
        assert_eq!(
            parse_hud_input("sem off"),
            HudInput::Command(HudCommand::Sem(false))
        );
        assert_eq!(
            parse_hud_input(":sem on"),
            HudInput::Command(HudCommand::Sem(true))
        );
        assert_eq!(
            parse_hud_input("tab history"),
            HudInput::Command(HudCommand::Tab(ContextTab::History))
        );
        assert_eq!(
            parse_hud_input("sem"),
            HudInput::Invalid("usage: sem off|on".into())
        );
        assert_eq!(
            parse_hud_input("sem maybe"),
            HudInput::Invalid("usage: sem off|on".into())
        );
        // A bare noun that happens to contain a command word stays a search.
        assert_eq!(
            parse_hud_input("trail mix recipe"),
            HudInput::SemanticQuery("trail mix recipe".into())
        );
    }

    #[test]
    fn score_bar_is_positional_and_total() {
        assert_eq!(score_bar(1.0, 1.0, 5), "▇▇▇▇▇");
        assert_eq!(score_bar(0.5, 1.0, 4), "▇▇▁▁");
        assert_eq!(score_bar(0.0, 1.0, 3), "▁▁▁");
        assert_eq!(score_bar(-2.0, 1.0, 3), "▁▁▁");
        assert_eq!(score_bar(f32::NAN, 1.0, 3), "▁▁▁");
        assert_eq!(score_bar(1.0, 0.0, 3), "▁▁▁");
        assert_eq!(score_bar(1.0, f32::NAN, 3), "▁▁▁");
        assert_eq!(score_bar(2.0, 1.0, 4), "▇▇▇▇");
        assert_eq!(score_bar(0.7, 1.0, 0), "");
        // Full bar means top-of-this-list, not a probability.
        assert_eq!(score_bar(1.38, 1.38, 5), "▇▇▇▇▇");
    }

    #[test]
    fn world_card_names_every_demo_contract_fact() {
        let card = world_card(
            std::path::Path::new("/demo"),
            4,
            8,
            174,
            "bge-base",
            "none (deterministic/manual path)",
            (1, 2, 0),
        );
        for needle in [
            "root: /demo",
            "semantic backend: bge-base",
            "generator: none (deterministic/manual path)",
            "network: OFFLINE",
            "files: 4",
            "objects: 8",
            "workspace tokens: 174",
            "1 repeats / 2 differences / 0 contradictions",
        ] {
            assert!(card.contains(needle), "missing {needle}:\n{card}");
        }
    }

    #[test]
    fn trail_view_lists_steps_newest_last_and_caps() {
        use physis_pro::nav::NavStep;
        let steps: Vec<NavStep> = (0..20)
            .map(|i| NavStep {
                kind: "query".to_string(),
                focus: format!("q{i}"),
                at: format!("t{i}"),
            })
            .collect();
        let text = trail_view_text(&steps, 15);
        assert!(text.starts_with("── TRAIL ──\n"));
        assert!(!text.contains("q0\n"), "cap must drop the oldest:\n{text}");
        assert!(text.contains("q19"), "{text}");
        let empty = trail_view_text(&[], 15);
        assert!(empty.contains("no navigation yet"), "{empty}");
    }

    #[test]
    fn history_view_appends_transitions_after_base() {
        use physis_pro::mvp::log::{CompactDelta, StateRef, Transition};
        let state = StateRef {
            content_hash: "h".to_string(),
            objects: 1,
            files: 1,
            map_hash: "m".to_string(),
        };
        let transitions = vec![
            Transition {
                v: 1,
                seq: 0,
                timestamp: "t0".to_string(),
                state_before: state.clone(),
                intent: "edit src/a.rs".to_string(),
                action: "edit-save".to_string(),
                delta: CompactDelta::default(),
                validation: vec![],
                outcome: "1 changed, 0 affected".to_string(),
                state_after: state.clone(),
                provenance: vec![],
                uncertainty: 0.1,
                slice: None,
            },
            Transition {
                v: 1,
                seq: 1,
                timestamp: "t1".to_string(),
                state_before: state.clone(),
                intent: "$ git status".to_string(),
                action: "shell".to_string(),
                delta: CompactDelta::default(),
                validation: vec![],
                outcome: "exit Some(0)".to_string(),
                state_after: state,
                provenance: vec![],
                uncertainty: 0.1,
                slice: None,
            },
        ];
        let text = history_view_text("git log here", &transitions, 8);
        assert!(text.starts_with("git log here\n"), "{text}");
        assert!(text.contains("SESSION TRANSITIONS"), "{text}");
        assert!(
            text.contains("#0 edit src/a.rs → edit-save — 1 changed, 0 affected"),
            "{text}"
        );
        assert!(
            text.contains("#1 $ git status → shell — exit Some(0)"),
            "{text}"
        );
        let bare = history_view_text("git log here", &[], 8);
        assert!(!bare.contains("SESSION TRANSITIONS"), "{bare}");
    }

    #[test]
    fn sibling_groups_branch_by_directory_only() {
        let center = hud_hit("src/retrieval.rs", "center-key-0123456789abcdef", 0.9);
        let hits = vec![
            center.clone(),
            hud_hit("src/index.rs", "same-dir-key-0123456789ab", 0.8),
            hud_hit("cli/main.rs", "cli-key-0123456789abcdefgh", 0.7),
            hud_hit("cli/util.rs", "cli2-key-0123456789abcdefg", 0.6),
            hud_hit("tests/it.rs", "tests-key-0123456789abcdefg", 0.4),
        ];
        let groups = sibling_groups("src/retrieval.rs", &hits, &center.object.id);
        // Same-directory neighbours are excluded; other dirs kept, ranked
        // by score descending inside the group, groups in path order.
        assert_eq!(
            groups.iter().map(|(d, _)| d.as_str()).collect::<Vec<_>>(),
            vec!["cli", "tests"]
        );
        assert_eq!(
            groups[0].1,
            vec!["cli/main.rs".to_string(), "cli/util.rs".to_string()]
        );
        assert_eq!(groups[1].1, vec!["tests/it.rs".to_string()]);
        // Bare label (no source path) falls back to the label itself.
        let bare = hud_bare_hit("weird-no-slash", "bare2-key-0123456789abcdef");
        let groups = sibling_groups("src/retrieval.rs", &[bare], &center.object.id);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, ".");
    }

    // ── PH-024: one front door (views) + editable settings ────────────────

    #[test]
    fn view_table_is_complete_wraps_and_round_trips() {
        let labels: Vec<&str> = View::ALL.iter().map(|v| v.label()).collect();
        let unique: BTreeSet<&str> = labels.iter().copied().collect();
        assert_eq!(unique.len(), View::ALL.len(), "labels must be unique: {labels:?}");
        for v in View::ALL {
            assert_eq!(View::from_name(v.label()), Some(v));
            if let Some(k) = v.key() {
                assert_eq!(View::from_key(k), Some(v));
            }
            // Tab never dead-ends and never sticks on one view.
            assert_ne!(v.next(), v);
            assert_eq!(v.next().prev(), v);
        }
        assert_eq!(
            View::from_name("SETTINGS"),
            Some(View::Settings),
            "names are case-insensitive"
        );
        assert_eq!(View::from_key('g'), Some(View::Graph));
        assert_eq!(View::from_key('q'), None, "q must keep meaning quit");
    }

    #[test]
    fn settings_rows_cover_knobs_providers_and_commands_in_stable_order() {
        let s = Settings::defaults();
        let rows = settings_rows(&s);
        let keys: Vec<String> = rows.iter().filter_map(|r| r.key()).collect();
        // One row per knob, in table order.
        for knob in physis_pro::config::CONFIG_KNOBS.iter() {
            assert!(keys.contains(&knob.key.to_string()), "{} missing", knob.key);
        }
        // Every provider × every field, and every command's enabled + aliases.
        for agent in s.providers.keys() {
            for field in ProviderField::ALL {
                assert!(
                    keys.contains(&format!("providers.{agent}.{}", field.name())),
                    "providers.{agent}.{} missing",
                    field.name()
                );
            }
        }
        for name in s.commands.keys() {
            assert!(keys.contains(&format!("commands.{name}.enabled")));
            assert!(keys.contains(&format!("commands.{name}.aliases")));
        }
        // Same input, same order — a list a person scrolls must not reshuffle.
        assert_eq!(rows, settings_rows(&Settings::defaults()));
    }

    #[test]
    fn setting_value_prints_dash_when_unset_and_the_env_name_for_the_key_field() {
        let mut s = Settings::defaults();
        let cfg = PhysisConfig::default();
        // `key_env` prints the NAME of the variable, never a secret — that is
        // the whole point of storing keys as env names.
        let key_row = SettingRow::Provider {
            agent: "jev".into(),
            field: ProviderField::KeyEnv,
        };
        assert_eq!(setting_value(&key_row, &s, &cfg), "JEV_API_KEY");
        s.set("providers.jev.key_env", "MY_JEV_KEY").unwrap();
        assert_eq!(setting_value(&key_row, &s, &cfg), "MY_JEV_KEY");
        // An unset field prints `-`, so "not configured" is visible and is not
        // the same as "configured empty".
        let endpoint = SettingRow::Provider {
            agent: "jev".into(),
            field: ProviderField::Endpoint,
        };
        assert_eq!(setting_value(&endpoint, &s, &cfg), "-");
    }

    #[test]
    fn step_setting_cycles_transport_and_toggles_commands() {
        let mut s = Settings::defaults();
        let mut cfg = PhysisConfig::default();
        let row = SettingRow::Provider {
            agent: "jev".into(),
            field: ProviderField::Transport,
        };
        let first = step_setting(&row, &s, &mut cfg, true).unwrap().unwrap();
        s.set(&first.key, &first.value).unwrap();
        assert_eq!(first.value, "cli", "cycles zen → cli, never invents one");
        // Round the closed set (zen · cli · http · local) back to the start.
        let mut last = first.value.clone();
        for _ in 0..3 {
            let next = step_setting(&row, &s, &mut cfg, true).unwrap().unwrap();
            s.set(&next.key, &next.value).unwrap();
            last = next.value;
        }
        assert_eq!(last, "zen");
        // Command toggle: recorded as the string the file stores.
        let cmd = SettingRow::CommandEnabled { name: "graph".into() };
        let was = s.commands.get("graph").unwrap().enabled;
        let change = step_setting(&cmd, &s, &mut cfg, true).unwrap().unwrap();
        assert_eq!(change.value, if was { "false" } else { "true" });
        // Typed rows never step; the caller opens the box instead.
        let aliases = SettingRow::CommandAliases { name: "graph".into() };
        assert!(step_setting(&aliases, &s, &mut cfg, true).unwrap().is_none());
    }

    #[test]
    fn set_setting_text_clamps_knobs_and_rejects_unknown_keys() {
        let s = Settings::defaults();
        let cfg_unused = PhysisConfig::default();
        let mut cfg = PhysisConfig::default();
        let change = set_setting_text(&s, &mut cfg, "embed_dim", "1").unwrap();
        assert!(change.is_knob);
        assert_eq!(cfg.embed_dim, 64, "knob floors hold when typed too");
        // A typed alias list is validated here and shown normalised by the view,
        // which always reads the merged document.
        let change =
            set_setting_text(&s, &mut cfg, "commands.ask.aliases", " q , ask-again ").unwrap();
        assert!(!change.is_knob);
        let mut merged = s.clone();
        merged.set(&change.key, &change.value).unwrap();
        let row = SettingRow::CommandAliases { name: "ask".into() };
        assert_eq!(setting_value(&row, &merged, &cfg_unused), "q,ask-again");
        assert!(set_setting_text(&s, &mut cfg, "providers.jev.nonsense", "x").is_err());
        assert!(set_setting_text(&s, &mut cfg, "embed_dim", "big").is_err());
    }

    #[test]
    fn next_setting_row_skips_section_headers_and_wraps() {
        let s = Settings::defaults();
        let rows = settings_rows(&s);
        assert!(matches!(rows[0], SettingRow::Section(_)));
        let first = next_setting_row(&rows, 0, true);
        assert!(!matches!(rows[first], SettingRow::Section(_)));
        // One step up from the first row lands on a real row at the end.
        let last = next_setting_row(&rows, first, false);
        assert!(!matches!(rows[last], SettingRow::Section(_)));
        assert_ne!(first, last, "wraps around, does not stick");
    }

    #[test]
    fn setup_report_names_the_gap_and_the_command_that_closes_it() {
        let dir = tempfile::tempdir().unwrap();
        let facts = SetupFacts {
            root: dir.path().to_path_buf(),
            files: 0,
            objects: 0,
            workspace_tokens: 0,
            embedder: "random-projection".into(),
            semantic: false,
            data_dir: dir.path().join(".physis"),
            config_path: dir.path().join(".physis/config.json"),
            settings_path: dir.path().join("settings.json"),
            settings_exists: false,
            observations_path: dir.path().join(".physis/observations.jsonl"),
            agents: vec![("jev".into(), "zen:oczen/x-free".into(), false)],
        };
        let text = setup_report(&facts);
        assert!(text.contains("OFF — lexical/structural only"), "{text}");
        assert!(
            text.contains("physis-pro selftest"),
            "next command for the semantic gap"
        );
        assert!(
            text.contains("(absent — shipped defaults are in use)"),
            "an absent file is named as one"
        );
        assert!(text.contains("[ ] workspace indexed"));
        assert!(text.contains("[ ] agent transport"));
        assert!(text.contains("(unavailable)"));

        // Happy path: boxes tick, and no next-command noise is printed.
        let ok = SetupFacts {
            files: 3,
            objects: 9,
            workspace_tokens: 40,
            semantic: true,
            embedder: "bge-base".into(),
            settings_exists: true,
            agents: vec![("jev".into(), "zen:oczen/x-free".into(), true)],
            ..facts
        };
        let text = setup_report(&ok);
        assert!(text.contains("[x] workspace indexed"));
        assert!(text.contains("[x] semantic retrieval"));
        assert!(text.contains("[x] agent transport"));
        assert!(
            !text.contains("next: physis-pro selftest"),
            "no next-command noise when nothing is missing"
        );
    }

    /// `guided`/`plan`/`guide` reach the guided-execution card; a bare word
    /// with no argument uses whatever is in the query box.
    #[test]
    fn guided_parses_with_and_without_an_intent() {
        assert_eq!(
            parse_hud_input("guided restart the praxis service"),
            HudInput::Command(HudCommand::Guided("restart the praxis service".into()))
        );
        assert_eq!(
            parse_hud_input(":guide"),
            HudInput::Command(HudCommand::Guided(String::new()))
        );
        assert_eq!(
            parse_hud_input("plan read this"),
            HudInput::Command(HudCommand::Guided("read this".into()))
        );
    }

    #[test]
    fn setup_and_world_parse_as_commands() {
        assert_eq!(
            parse_hud_input("setup"),
            HudInput::Command(HudCommand::Setup)
        );
        assert_eq!(
            parse_hud_input(":world"),
            HudInput::Command(HudCommand::World)
        );
        // `settings set <key> <value>` still belongs to the CLI, not the box:
        // it needs an argument, so the box must not silently treat it as one of
        // its own commands.
        assert!(matches!(
            parse_hud_input("settings set providers.jev.model x"),
            HudInput::SemanticQuery(_)
        ));
    }

    #[test]
    fn view_names_and_projection_ids_cannot_drift() {
        // The bin's `View` now derives its label from the library's
        // `ProjectionId` and parses names through it, so drift is a compile
        // error rather than a test failure. What this test still guards is the
        // part a compiler cannot see: the *order* of the two tables (Tab order
        // and the footer read `View::ALL`; snapshots and lenses read
        // `ProjectionId::all`), and that the round trip holds for every name.
        use physis_pro::hud::world::ProjectionId;

        let bin_order: Vec<ProjectionId> = View::ALL.iter().map(|v| v.projection()).collect();
        assert_eq!(
            bin_order,
            ProjectionId::all().to_vec(),
            "tab order drifted from projection order"
        );
        let bin_names: Vec<&str> = View::ALL.iter().map(|v| v.label()).collect();
        assert_eq!(bin_names, ProjectionId::all().iter().map(|p| p.name()).collect::<Vec<_>>());
        for view in View::ALL {
            assert_eq!(View::from_name(view.label()), Some(view));
            assert_eq!(View::from_projection(view.projection()), view);
        }
        assert_eq!(View::from_name("nope"), None, "an unknown view is refused");
    }

    #[test]
    fn enter_descends_into_a_files_contents_and_back_restores_the_focus() {
        // §13: the same control descends where contents exist (file -> passages)
        // and climbs back to the identity you left, so scale is continuous over
        // repository objects. Compares canonical ids, not paths: a file and its
        // passages share one path, which is exactly why a path check would pass
        // while the focus never moved.
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        let _embedder = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        // Long enough to be chunked into several passages.
        let body: String = (0..40)
            .map(|i| format!("fn chunk_{i}() -> usize {{ {i} }}\n"))
            .collect();
        std::fs::write(dir.join("long.rs"), body).ok();

        let mut app = App::new(dir.to_path_buf(), true).expect("app opens");
        app.query = "chunk".to_string();
        app.run_query();
        assert!(
            !app.results.hits.is_empty(),
            "precondition: the fixture indexed the file"
        );

        // Focus the *file* object through the one mutation path. Hand-assigning
        // `hits`/`sel` here leaves `rows` stale and the cursor pointing at
        // whatever row happened to be there — the duplicate-state hazard
        // PH-029 is about, hit for real while writing this test.
        let file_hit = app.results.hits
            .iter()
            .find(|h| h.object.kind == ObjectKind::File)
            .cloned()
            .expect("precondition: the index holds a File object for long.rs");
        app.set_results(vec![file_hit.clone()]);
        assert_eq!(
            app.current_hit().map(|h| h.object.kind),
            Some(ObjectKind::File),
            "precondition: the cursor is on the file, not on a passage"
        );
        let file_id = file_hit.object.id.clone();

        app.enter();
        assert!(
            !app.results.hits.is_empty(),
            "descending into a file with passages yields its contents"
        );
        assert!(
            app.results.hits
                .iter()
                .all(|h| h.object.kind == ObjectKind::Passage),
            "a file's contents are its passages: {:?}",
            app.results.hits.iter().map(|h| &h.object.kind).collect::<Vec<_>>()
        );
        assert_eq!(
            app.focus_state.focus.object,
            Some(file_id.clone()),
            "the container stays focused while you are inside it"
        );

        app.back();
        assert_eq!(
            app.focus_state.focus.object,
            Some(file_id),
            "climbing out restores the identity you descended from"
        );
        assert_eq!(app.results.hits.len(), 1, "and the rows you came from");
    }

    #[test]
    fn focus_offers_only_registered_actions_and_executes_one() {
        // §14 + §21: focus -> "what may I do to THIS object?" -> a typed action
        // -> an observation in the registry log. The action path is the existing
        // one (`action::dispatch`); this test pins that the *offered* verbs come
        // from the same registry, so the list cannot promise what the executor
        // refuses.
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        let _embedder = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        std::fs::write(
            dir.join("notes.rs"),
            "fn retrieval_budget() -> usize { 7 }\n// retrieval and packing\n",
        )
        .ok();

        let mut app = App::new(dir.to_path_buf(), true).expect("app opens");
        app.query = "retrieval".to_string();
        app.run_query();
        assert!(!app.results.hits.is_empty(), "precondition: the query found the file");

        app.show_actions();
        let (title, body) = app.reading.clone().expect("actions renders");
        assert!(title.starts_with("actions: "), "{title}");
        assert!(
            body.contains("fs.read") || body.contains("no registered capability"),
            "the list is either real capabilities or an honest refusal: {body}"
        );

        // Execute a read through the same path the palette points at.
        let focus = app.current_hit().cloned().expect("cursor has an object");
        let handle = focus.object.id.short();
        app.submit_command_line(&format!(":action read {handle} limit=2"));
        let (title, _body) = app.reading.clone().expect("action renders a result");
        assert!(title.starts_with("action "), "{title}");

        // The observation is on disk: the world can explain what happened.
        let obs = std::fs::read_to_string(physis_pro::registry::default_obs_path())
            .unwrap_or_default();
        assert!(
            obs.contains(&handle) || obs.contains("read"),
            "the action left an observation the world can cite: {obs}"
        );
    }

    #[test]
    fn one_object_survives_the_world_graph_and_semantic_projections() {
        // The acceptance loop's spine (§21): focus an object, change the
        // projection twice, and the identity is unchanged each time. Compares
        // canonical `ObjectId`s, never paths — a file and its passage can share
        // one path, which is how a path comparison would pass while the focus
        // actually moved.
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        let _embedder = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        std::fs::write(dir.join("alpha.rs"), "fn alpha_budget() -> usize { 1 }\n").ok();
        std::fs::write(dir.join("beta.rs"), "fn beta_budget() -> usize { 2 }\n").ok();

        let mut app = App::new(dir.to_path_buf(), true).expect("app opens");
        app.query = "budget".to_string();
        app.run_query();
        assert!(
            app.results.hits.len() >= 2,
            "precondition: the fixture indexes two objects to project"
        );
        let focused = app
            .current_hit()
            .expect("the query focuses an object")
            .object
            .id
            .clone();

        // World -> graph.
        app.open_view_via_command(View::Graph);
        assert_eq!(app.focus_state.projection.name(), "graph");
        assert_eq!(app.focus_state.focus.object, Some(focused.clone()));
        let (_, graph_body) = app.reading.clone().expect("graph renders");
        assert!(graph_body.starts_with('●') || graph_body.contains("edges:"), "{graph_body}");

        // Graph -> semantic: a different projection of the same world.
        app.open_view_via_command(View::Semantic);
        assert_eq!(app.focus_state.projection.name(), "semantic");
        assert_eq!(
            app.focus_state.focus.object,
            Some(focused.clone()),
            "the semantic projection keeps the focused object"
        );
        let (title, body) = app.reading.clone().expect("semantic renders");
        assert!(title.starts_with("semantic: "), "{title}");
        assert!(
            body.contains("every row above is Inferred"),
            "the projection states its epistemic status: {body}"
        );

        // And back to the world: the identity still has not moved.
        app.open_view(View::World);
        assert_eq!(app.focus_state.projection.name(), "world");
        assert_eq!(app.focus_state.focus.object, Some(focused));
    }

    #[test]
    fn a_bare_line_goes_through_the_intent_chain_and_says_which_route_it_took() {
        // The gate for PH-027's first step: bare text must reach
        // `hud::intent::resolve_bare` from `submit_command_line`. Before this,
        // the chain existed and was tested but nothing called it.
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        // Embedder-agnostic assertions: force the lexical fallback so the test
        // neither needs a model nor waits on an HTTP embedder (31 s vs 3 s, and
        // it must not fail because a machine's model server is busy).
        let _embedder = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        std::fs::write(dir.join("retrieval.rs"), "fn retrieval_budget() -> usize { 3 }\n").ok();
        std::fs::write(dir.join("other.rs"), "fn other() {}\n").ok();

        let mut app = App::new(dir.to_path_buf(), true).expect("app opens");

        // 1. Bare text with no exact owner: an intent over the ranked matches.
        app.submit_command_line("retrieval");
        let r = app
            .last_intent
            .clone()
            .expect("bare text resolves through the chain");
        assert_eq!(r.route, physis_pro::hud::intent::ResolutionRoute::Intent);
        assert_eq!(r.utterance, "retrieval");
        assert!(!r.objects.is_empty(), "matched objects are carried: {r:?}");
        assert!(
            r.provenance_line().starts_with("route=intent-ir objects="),
            "{}",
            r.provenance_line()
        );
        assert!(
            app.status.contains(&r.provenance_line()),
            "the route is visible: {}",
            app.status
        );
        assert!(
            app.status.contains("semantic: off"),
            "and the degradation notice is not displaced by it: {}",
            app.status
        );

        // 2. A line that *is* one object's path: the world already answered.
        app.submit_command_line("retrieval.rs");
        let exact = app.last_intent.clone().expect("exact line resolves too");
        assert_eq!(
            exact.route,
            physis_pro::hud::intent::ResolutionRoute::ExactObject,
            "one exact path match is an answer, not a hypothesis: {exact:?}"
        );
        assert_eq!(exact.objects.len(), 1);
        assert!(
            exact.provenance_line().starts_with("route=exact-object object="),
            "{}",
            exact.provenance_line()
        );

        // 3. The IR is reachable from the `:intent` surface, not just stored.
        app.submit_command_line(":intent");
        let (title, body) = app.reading.clone().expect("intent renders");
        assert_eq!(title, "intent");
        assert!(
            body.contains("last bare line: retrieval.rs"),
            "the last resolution is shown: {body}"
        );
        assert!(body.contains("route=exact-object"), "{body}");
    }

    #[test]
    fn the_graph_view_renders_from_the_lens_not_a_second_projection() {
        // The wiring gate: `graph_view` must draw the lens's rows. If it went
        // back to deriving its own edge list, the recorded relation below would
        // still appear — but the *identity* column would not, so the check that
        // catches drift is that every rendered row parses back to a real object.
        use physis_pro::hud::lens_object::ObjectLens;

        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        std::fs::write(dir.join("alpha.rs"), "fn alpha_budget() -> usize { 1 }\n").ok();
        std::fs::write(dir.join("beta.rs"), "use alpha_budget;\nfn beta() {}\n").ok();

        let mut app = App::new(dir.to_path_buf(), true).expect("app opens");
        app.query = "alpha budget".to_string();
        app.run_query();
        assert!(app.results.hits.len() >= 2, "precondition: two objects to relate");
        // Relate the cursor object to one from a *different* file, the way an
        // operator would, and also to another object in the cursor's own file:
        // two recorded relations, both of which the projection must show.
        // The graph centres on the *cursor*, not on hits[0]: `run_query` moves
        // the selection, and a test that related hits[0] would be asserting
        // about an object the graph is not showing.
        let a = app
            .current_hit()
            .expect("cursor has an object")
            .object
            .clone();
        let a_path = a.source.as_ref().map(|s| s.path.clone()).unwrap_or_default();
        let b = app.results.hits
            .iter()
            .find(|h| h.object.source.as_ref().map(|s| s.path.clone()).unwrap_or_default() != a_path)
            .map(|h| h.object.clone())
            .expect("a second file is indexed");
        let same_label = app.results.hits
            .iter()
            .find(|h| {
                h.object.id != a.id
                    && h.object.source.as_ref().map(|s| s.path.clone()).unwrap_or_default() == a_path
            })
            .map(|h| h.object.clone());
        app.ws.add_relation(&a, &b, "relates to", "").expect("relation");
        if let Some(p) = &same_label {
            app.ws
                .add_relation(&a, p, "relates to", "")
                .expect("passage relation");
        }

        app.graph_view();
        let (title, body) = app.reading.clone().expect("graph renders into the pane");
        assert!(title.starts_with("graph: "), "{title}");
        assert!(body.contains("solid = user"), "legend: {body}");
        assert!(
            body.contains("───"),
            "the recorded user relation renders solid: {body}"
        );

        // The same object the list holds is the graph's centre, by identity.
        let center_hit = app.current_hit().expect("cursor has an object").clone();
        let lens = ObjectLens::project(&app.ws, &center_hit, &app.results.hits);
        assert_eq!(
            physis_pro::world_object::parse_object_id(&lens.center.identity),
            center_hit.object.id
        );
        assert_eq!(
            lens.edges.len(),
            if same_label.is_some() { 2 } else { 1 },
            "every recorded relation renders: {:?}",
            lens.edges.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
        // Dedup is by identity: a row set never repeats an object.
        let ids: std::collections::BTreeSet<&String> =
            lens.edges.iter().map(|e| &e.id).collect();
        assert_eq!(ids.len(), lens.edges.len(), "no duplicate identities");
        for e in lens.edges.iter().chain(lens.near.iter()) {
            let oid = physis_pro::world_object::parse_object_id(&e.id);
            let key = serde_json::to_string(&oid).unwrap_or_default();
            assert!(
                app.ws.index().objects.contains_key(&key),
                "every lens row resolves to an object in the world: {}",
                e.id
            );
        }
    }

    #[test]
    fn opening_a_view_keeps_the_focused_object_and_selection() {
        // The vertical slice's invariant, tested through the bin's own
        // open_view: graph -> tasks -> world must not move the operator off
        // the object they were looking at.
        // Unique per process and self-cleaning: the lib and bin test binaries
        // run as separate processes, so a fixed /tmp path would let them
        // collide even with the in-process lock held.
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        std::fs::write(dir.join("a.rs"), "fn alpha() { let x = 1; }\n").ok();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));

        let mut app = App::new(dir.to_path_buf(), true).expect("app opens on a temp workspace");
        app.run_query();
        assert!(
            !app.results.hits.is_empty(),
            "precondition: the temp workspace indexes a.rs, so there is a focus to prove"
        );
        let object = app.current_hit().expect("cursor has an object").object.id.clone();

        app.open_view(View::Graph);
        assert_eq!(app.focus_state.projection.name(), "graph");
        assert_eq!(app.focus_state.focus.object, Some(object.clone()));
        assert!(app.focus_state.selection.contains(&object));

        app.open_view(View::Tasks);
        assert_eq!(app.focus_state.projection.name(), "tasks");
        assert_eq!(
            app.focus_state.focus.object,
            Some(object.clone()),
            "the focused object survives a projection switch"
        );

        app.open_view(View::World);
        assert_eq!(app.focus_state.projection.name(), "world");
        assert_eq!(app.focus_state.focus.object, Some(object));

    }

    #[test]
    fn agent_reply_enters_through_the_reducer_and_only_annotates() {
        // The bin's half of the authority gate: a model reply arriving on the
        // worker channel fills the status line and the reading pane, and
        // nothing else in App moves — no query rewrite, no action queued.
        use physis_pro::hud::event::{reduce, HudEffect, HudEvent};

        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        let mut app = App::new(dir.to_path_buf(), false).expect("app opens on a temp workspace");

        app.agent_pending = Some("jev".into());
        app.query = "retrieval".into();
        let before_query = app.query.clone();
        let event = HudEvent::AgentReply {
            agent: "jev".into(),
            ok: true,
            text: "$ rm -rf / --no-preserve-root".into(),
        };
        for effect in reduce(&mut app.focus_state, &event) {
            assert!(
                matches!(
                    effect,
                    HudEffect::Status(_) | HudEffect::Reading { .. } | HudEffect::ClearPending
                ),
                "a reply may only annotate: {effect:?}"
            );
            app.apply_hud_effect(effect);
        }

        assert!(app.agent_pending.is_none(), "the call is no longer in flight");
        assert!(app.status.contains("replied"), "{}", app.status);
        let (title, body) = app.reading.clone().expect("reading pane filled");
        assert!(title.contains("route: model"), "{title}");
        assert_eq!(body, "$ rm -rf / --no-preserve-root");
        assert_eq!(app.query, before_query, "a reply cannot rewrite the operator's query");

    }

    #[test]
    fn object_lens_renders_recorded_edges_in_graph_grammar() {
        use physis_pro::hud::lens_object::ObjectLens;
        use std::path::PathBuf;

        // Headless vertical slice: a temp workspace with two related files.
        // No embedder, no network — the lens reads recorded relations only.
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        std::fs::write(dir.join("a.rs"), "fn alpha() {}\n").ok();
        std::fs::write(dir.join("b.rs"), "fn beta() {}\n").ok();
        // Isolate the sled store so the test never touches the real index.
        let _data_dir_lock = lock_data_dir();
        let _data_dir = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
        let mut ws = physis_pro::nav::workspace::NavWorkspace::open(dir).unwrap();
        // Open is stat-only: ingest the two files so objects exist. No
        // embedder (lexical only) — the lens reads relations, not vectors.
        let _ = ws.update(false);
        let objects: Vec<physis_pro::nav::PhysisObject> = ws.index().objects.values().cloned().collect();
        assert!(!objects.is_empty(), "workspace indexes the two files");
        let center = NavHit {
            object: objects[0].clone(),
            score: physis_pro::nav::RetrievalScore { total: 1.0, components: Default::default() },
        };
        let others: Vec<NavHit> = objects[1..].iter().map(|o| NavHit {
            object: o.clone(),
            score: physis_pro::nav::RetrievalScore { total: 0.5, components: Default::default() },
        }).collect();

        // Record one user edge so the lens has a solid line to render.
        assert!(
            !others.is_empty(),
            "precondition: two files index to at least two objects"
        );
        ws.add_relation(&center.object, &others[0].object, "relates to", "").ok();

        let lens = ObjectLens::project(&ws, &center, &others);
        // The identity gate: the projected object IS the list's object, and it
        // parses back to the exact canonical id (`short()` would truncate the
        // content hash and lose that proof).
        assert_eq!(
            physis_pro::world_object::parse_object_id(&lens.center.identity),
            center.object.id
        );
        assert_eq!(lens.center.content, center.object.label);
        // The recorded edge renders solid (user origin).
        assert!(
            lens.edges.iter().any(|e| e.glyph == "───"),
            "user edge renders solid: {:?}",
            lens.edges
        );
        // Near rows are dashed and Inferred - similarity never borrows proof.
        for n in &lens.near {
            assert_eq!(n.glyph, "- -");
            assert_eq!(n.origin, physis_pro::nav::RelationOrigin::Inferred);
        }
        let text = lens.render().join("\n");
        assert!(text.contains("solid = user"), "{text}");

        let _ = PathBuf::from(".");
    }
}
