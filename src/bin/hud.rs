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

use physis_pro::mvp::log::{hash_canonical, CompactDelta, StateRef, Transition, TransitionLog};
use physis_pro::nav::{self, NavHit, NavWeights, ObjectKind};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut prescan = true;
    let mut prescan_only = false;
    let mut snapshot: Option<(u16, u16)> = None;
    let mut root: Option<PathBuf> = None;
    for a in args.iter().skip(1) {
        match a.as_str() {
            "--prescan-only" => {
                prescan_only = true;
            }
            "--no-prescan" => {
                prescan = false;
            }
            "--snapshot" => {
                snapshot = Some((100, 30));
            }
            "-h" | "--help" => {
                eprintln!("physis-hud [root] [--prescan-only] [--no-prescan] [--snapshot [WxH]]");
                eprintln!("  default: prescan index + embedding cache, then open the HUD");
                eprintln!("  --prescan-only: warm the index + caches and exit (no TUI)");
                eprintln!("  --no-prescan: open on the persisted index as-is (fast restart)");
                eprintln!(
                    "  --snapshot [WxH]: render one frame to stdout and exit (no TTY, CI-safe)"
                );
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
    if let Some((w, h)) = snapshot {
        return snapshot_run(&root, w, h);
    }
    let app = App::new(root, prescan)?;
    let terminal = enter_tui()?;
    let result = run(terminal, app);
    leave_tui()?;
    result
}

/// Render one frame through the same `draw` path to a `TestBackend` and print
/// it. A golden frame in CI cannot drift from what a person sees, because there
/// is no second renderer. No TTY required.
fn snapshot_run(root: &Path, w: u16, h: u16) -> anyhow::Result<()> {
    use ratatui::backend::TestBackend;
    let app = App::new(root.to_path_buf(), false)?;
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
        | "zoom" | "edges" | "trail" | "pins" | "unpin"
            if rest.is_empty() =>
        {
            parse_named_command(text)
        }
        "dependencies" | "focus" | "relate" | "tab" | "sem" | "shell" | "search" | "query"
        | "external" | "action" => parse_named_command(text),
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
    hits: Vec<NavHit>,
    rows: Vec<Row>,
    sel: usize,
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
            hits: Vec::new(),
            rows: Vec::new(),
            sel: 0,
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
        let targets: Vec<String> = self
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
            HudCommand::Graph => self.graph_view(),
            HudCommand::Notebook => self.show_notebook(),
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
                .rows
                .iter()
                .position(|row| matches!(row, Row::Hit(h) if h.object.id == id))
            {
                self.sel = pos;
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
                self.hits.clear();
                self.rows.clear();
                self.sel = 0;
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
                self.semantic_on = self.embedder_kind != "random-projection";
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
        let hits = self.hits.clone();
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
        self.sel = self
            .rows
            .iter()
            .position(|r| matches!(r, Row::Hit(_)))
            .unwrap_or(0);
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
            self.explain
                .push_str(&format!("DIR {dir} ({n} hits)\nEnter drills in.\n"));
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
        self.sel = self
            .rows
            .iter()
            .position(|r| matches!(r, Row::Hit(_)))
            .unwrap_or(0);
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
                        self.reading = Some((hit.object.label.clone(), text));
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
            self.sync_list_state();
            self.update_explain();
        }
        self.crumbs.pop();
    }

    fn toggle_select(&mut self) {
        if let Some(hit) = self.current_hit().cloned() {
            if let Some(pos) = self
                .selected
                .iter()
                .position(|s| s.object.id == hit.object.id)
            {
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
        let label_of = |id: &nav::ObjectId| {
            self.ws
                .index()
                .objects
                .get(&serde_json::to_string(id).unwrap())
                .map(|o| o.label.clone())
                .unwrap_or_else(|| id.short())
        };
        // Every graph edge names the file it points at, so the graph is a way
        // to a file and not a wall of labels: the path shown is the one
        // `open`/`edit` accept.
        let path_of = |id: &nav::ObjectId| {
            self.ws
                .index()
                .objects
                .get(&serde_json::to_string(id).unwrap())
                .and_then(|o| o.source.as_ref())
                .map(|s| s.path.clone())
                .unwrap_or_default()
        };
        let mut lines = Vec::new();
        lines.push(format!("● {}", center.object.label));
        lines.push(format!(
            "edges: {}",
            Self::edge_filter_name(self.edge_filter)
        ));
        // Related objects from relations touching the center, deduped.
        let mut seen = std::collections::BTreeSet::new();
        for r in self.ws.relations_of(&center.object) {
            if !Self::edge_shown(true, self.edge_filter) {
                continue;
            }
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
            let other_path = path_of(other);
            lines.push(format!(
                "  {dir} {origin} [{rel}] {name} {origin}→  {other_path}"
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
            if !Self::edge_shown(false, self.edge_filter) {
                continue;
            }
            if h.object.id == center.object.id || !seen.insert(h.object.label.clone()) {
                continue;
            }
            let p = h
                .object
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| h.object.label.clone());
            let m = state_of(
                &p,
                &self.changed,
                &self.affected,
                &pinned,
                &intent,
                &self.selected,
                &h.object,
            );
            lines.push(format!(
                "  {m}  - - [near {:.2}] {} - -  {p}",
                h.score.total, h.object.label
            ));
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
                        "SIBLINGS — similar content, other directories ({total} files, {}/{} shown; similarity, not proven relation)",
                        siblings.len(),
                        siblings.len()
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
            format!("graph: {} — {}", center.object.label, center_path),
            lines.join("\n"),
        ));
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
        self.semantic_on = self.embedder_kind != "random-projection";
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
        self.reading = Some(("commands".into(), o));
        self.status = "commands: deterministic dispatch, no model".into();
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
        .hits
        .iter()
        .map(|h| h.score.total)
        .fold(0.0_f32, f32::max);
    let items: Vec<ListItem> = app
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
                Span::raw(if i == app.sel { "▶ " } else { "  " }),
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
                if i == app.sel {
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
                .title(" semantic neighborhood "),
        )
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
    let keys = if app.editor.is_some() {
        "type · ←/→/↑/↓/Home/End move · Enter newline · Backspace/Delete · Ctrl+S save · Esc/Ctrl+C cancel"
    } else {
        "e inline edit · :external editor · u rescan · g graph · t edges · H history · B bench · D dependents · C contradictions · f focus · F clear · z zoom · 1/2/3/4 tabs · n notebook · p pin · s/d/R relate · space select · ? help · q quit"
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
    s.select(Some(app.sel));
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
        // the moment it lands instead of on the next keystroke.
        while let Ok(reply) = agent_rx.try_recv() {
            let mut app = app.lock().unwrap();
            app.agent_pending = None;
            app.status = if reply.ok {
                format!(
                    "@{} replied ({} chars) — reading pane shows the reply",
                    reply.agent,
                    reply.text.len()
                )
            } else {
                format!(
                    "@{} failed: {}",
                    reply.agent,
                    reply.text.lines().next().unwrap_or("no detail")
                )
            };
            app.reading = Some((format!("@{} reply", reply.agent), reply.text));
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
                                    quit = app.submit_command_line(&line);
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
                            KeyCode::Char('n') => app.show_notebook(),
                            KeyCode::Char('r') => app.related(),
                            KeyCode::Char('g') => app.graph_view(),
                            KeyCode::Char('t') => app.cycle_edge_filter(),
                            KeyCode::Char('H') => app.show_history(),
                            KeyCode::Char('B') => app.show_bench(),
                            KeyCode::Char('D') => app.show_dependents(),
                            KeyCode::Char('C') => app.show_contradictions(),
                            KeyCode::Char('e') => app.start_inline_edit(None),
                            KeyCode::Char('u') => app.rescan_now(),
                            KeyCode::Char('f') => app.apply_focus(),
                            KeyCode::Char('F') => app.clear_focus(),
                            KeyCode::Char('z') => app.toggle_zoom(),
                            KeyCode::Char('1') => app.switch_context_tab(ContextTab::Why),
                            KeyCode::Char('2') => app.switch_context_tab(ContextTab::Context),
                            KeyCode::Char('3') => app.switch_context_tab(ContextTab::Delta),
                            KeyCode::Char('4') => app.switch_context_tab(ContextTab::History),
                            KeyCode::Char('?') => app.show_help(),
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
}
