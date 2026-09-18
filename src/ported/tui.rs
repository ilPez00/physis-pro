// provenance: moved from physis-core src/bin/system_tui.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
// ponytail: dead under default features until a `physis-system-tui` bin wires
// `run()` in; delete if no bin ever lands.
#![allow(dead_code)]
//! `physis-system-tui` — navigate the workspace interface with a mouse and keys.
//!
//! The same [`Workspace`] service the CLI calls, rendered as three regions:
//!
//! ```text
//!   ┌ workspace · operation · denominators ─────────────────┐
//!   │ results (list)        │ viewer (text, line numbers)   │
//!   ├───────────────────────┴───────────────────────────────┤
//!   │ > command                                             │
//!   └───────────────────────────────────────────────────────┘
//! ```
//!
//! Keys: `Tab` focus · `↑/↓` `j/k` select · `Enter` open · `PgUp/PgDn` page ·
//! `g/G` top/bottom · `/` find · `p` pack · `i` inspect · `l` list ·
//! `h` history · `:` command · `Esc` leave the command line · `?` help · `q` quit.
//!
//! Mouse: click a row to select it, click again to open it, click a pane to
//! focus it, click the command line to type, wheel to scroll the pane under the
//! pointer.
//!
//! Every query runs through the same service as `physis system …`, so what the
//! screen shows is what the CLI would have printed. Writing operations
//! (`remember`, `run`, `export`) are deliberately absent: this is a reader.
use std::io::{stdout, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use crate::ported::system::{PackStrategy, Workspace};

/// One selectable row. `target` is what `read` is given when it is opened, so
/// the list and the viewer agree on addressing with the CLI.
#[derive(Clone)]
struct Row {
    label: String,
    detail: String,
    target: Option<String>,
}

#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Results,
    Viewer,
    Command,
}

struct App {
    workspace: Workspace,
    rows: Vec<Row>,
    state: ListState,
    viewer: Vec<String>,
    viewer_title: String,
    viewer_scroll: usize,
    status: String,
    banner: String,
    command: String,
    focus: Focus,
    results_area: Rect,
    viewer_area: Rect,
    command_area: Rect,
    last_click: Option<usize>,
    quit: bool,
}

impl App {
    fn new(workspace: Workspace) -> Self {
        let mut app = Self {
            workspace,
            rows: vec![],
            state: ListState::default(),
            viewer: vec![],
            viewer_title: "viewer".into(),
            viewer_scroll: 0,
            status: String::new(),
            banner: String::new(),
            command: String::new(),
            focus: Focus::Results,
            results_area: Rect::default(),
            viewer_area: Rect::default(),
            command_area: Rect::default(),
            last_click: None,
            quit: false,
        };
        app.run_command("inspect");
        app
    }

    fn set_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.state
            .select(if self.rows.is_empty() { None } else { Some(0) });
    }

    fn show(&mut self, title: String, text: &str) {
        self.viewer_title = title;
        self.viewer = text.lines().map(str::to_string).collect();
        self.viewer_scroll = 0;
    }

    /// Run a command line, exactly as the CLI would parse it. Errors land in the
    /// status line rather than ending the session: a mistyped query is the
    /// normal case in an interactive reader.
    fn run_command(&mut self, line: &str) {
        let line = line.trim();
        let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let result = match verb {
            "" => Ok(()),
            "q" | "quit" | "exit" => {
                self.quit = true;
                Ok(())
            }
            "?" | "help" => {
                self.show("help".into(), HELP);
                self.banner = "help".into();
                Ok(())
            }
            "i" | "inspect" => self.op_inspect(),
            "l" | "list" => self.op_list(rest),
            "f" | "find" => self.op_find(rest),
            "p" | "pack" => self.op_pack(rest),
            "r" | "read" | "open" => self.op_read(rest),
            "h" | "history" => self.op_history(rest),
            "c" | "capabilities" => {
                let value = self.workspace.capabilities();
                self.show(
                    "capabilities".into(),
                    &serde_json::to_string_pretty(&value).unwrap_or_default(),
                );
                self.banner = "capabilities".into();
                Ok(())
            }
            other => Err(anyhow::anyhow!(
                "unknown command `{other}` — try ?, find, pack, read, list, history, inspect"
            )),
        };
        match result {
            Ok(()) => {}
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn op_inspect(&mut self) -> Result<()> {
        let data = self.workspace.inspect()?;
        self.banner = format!(
            "inspect · {} files · {} records in window",
            data["files"], data["workspace_records_in_window"]
        );
        self.status = "Enter opens · / find · p pack · ? help".into();
        self.show(
            "inspect".into(),
            &serde_json::to_string_pretty(&data).unwrap_or_default(),
        );
        self.op_list("")
    }

    fn op_list(&mut self, rest: &str) -> Result<()> {
        let limit: usize = rest.parse().unwrap_or(500);
        let inventory = self.workspace.inventory()?;
        let total = inventory.objects.len();
        let rows: Vec<Row> = inventory
            .objects
            .into_iter()
            .take(limit)
            .map(|o| Row {
                label: o.path.clone(),
                detail: format!("{} bytes", o.bytes),
                target: Some(o.path),
            })
            .collect();
        self.banner = format!(
            "list · {} of {total} files · truncated: {}",
            rows.len(),
            inventory.truncated
        );
        self.set_rows(rows);
        Ok(())
    }

    fn op_find(&mut self, query: &str) -> Result<()> {
        anyhow::ensure!(!query.is_empty(), "find needs a query");
        let data = self.workspace.find(query, 40)?;
        let rows: Vec<Row> = data["hits"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|hit| {
                let label = hit["label"].as_str().unwrap_or("").to_string();
                let kind = hit["kind"].as_str().unwrap_or("");
                Row {
                    detail: format!(
                        "{:.2}  {}",
                        hit["score"].as_f64().unwrap_or(0.0),
                        hit["excerpt"].as_str().unwrap_or("").trim()
                    ),
                    target: Some(if kind == "observation" {
                        hit["id"].as_str().unwrap_or("").to_string()
                    } else {
                        label.clone()
                    }),
                    label,
                }
            })
            .collect();
        self.banner = format!(
            "find '{query}' · {} hits over {} documents",
            rows.len(),
            data["documents_ranked"]
        );
        self.status = "BM25 over whole files; `pack` ranks line windows".into();
        self.set_rows(rows);
        Ok(())
    }

    fn op_pack(&mut self, rest: &str) -> Result<()> {
        // `pack <query>` or `pack --budget 2000 <query>`.
        let (budget, query) = match rest.strip_prefix("--budget ") {
            Some(tail) => {
                let (value, query) = tail.split_once(' ').unwrap_or((tail, ""));
                (value.parse().unwrap_or(2000), query.trim())
            }
            None => (2000, rest),
        };
        anyhow::ensure!(!query.is_empty(), "pack needs a query");
        let data = self
            .workspace
            .pack_with(query, budget, 3, PackStrategy::FilesThenWindows, true)?;
        let chunks = data["chunks"].as_array().cloned().unwrap_or_default();
        let mut text = String::new();
        let rows: Vec<Row> = chunks
            .iter()
            .map(|chunk| {
                let path = chunk["path"].as_str().unwrap_or("");
                let (start, end) = (
                    chunk["start_line"].as_u64().unwrap_or(1),
                    chunk["end_line"].as_u64().unwrap_or(1),
                );
                text.push_str(&format!("--- {path}:{start}-{end}\n"));
                text.push_str(chunk["text"].as_str().unwrap_or(""));
                text.push('\n');
                Row {
                    label: format!("{path}:{start}-{end}"),
                    detail: format!(
                        "{} tokens · score {:.2}",
                        chunk["tokens"].as_u64().unwrap_or(0),
                        chunk["score"].as_f64().unwrap_or(0.0)
                    ),
                    target: Some(format!("{path}:{start}-{end}")),
                }
            })
            .collect();
        self.banner = format!(
            "pack '{query}' · {} of {} tokens · {} windows ranked · {} bridged",
            data["used_tokens"], data["budget_tokens"], data["windows_ranked"], data["windows_bridged"]
        );
        self.status = "the bundle is in the viewer; Enter re-reads one region".into();
        self.set_rows(rows);
        self.show(format!("pack: {query}"), &text);
        Ok(())
    }

    fn op_read(&mut self, target: &str) -> Result<()> {
        anyhow::ensure!(!target.is_empty(), "read needs a path, file: ID, or obs:N");
        let data = self.workspace.read(target, 64 * 1024)?;
        if let Some(text) = data["text"].as_str() {
            let path = data["object"]["path"].as_str().unwrap_or(target);
            let start = data["lines"]["start_line"].as_u64().unwrap_or(1);
            self.viewer_title = if data["lines"].is_null() {
                path.to_string()
            } else {
                format!("{path}:{start}-{}", data["lines"]["end_line"])
            };
            // Absolute line numbers, so what is on screen can be handed back to
            // `read path:start-end` without counting from the top of a region.
            self.viewer = text
                .lines()
                .enumerate()
                .map(|(i, line)| format!("{:>6} {line}", start as usize + i))
                .collect();
            self.viewer_scroll = 0;
            self.status = format!(
                "{} lines{}",
                self.viewer.len(),
                if data["truncated"] == true {
                    " · partial read"
                } else {
                    ""
                }
            );
        } else {
            self.show(
                target.to_string(),
                &serde_json::to_string_pretty(&data).unwrap_or_default(),
            );
        }
        Ok(())
    }

    fn op_history(&mut self, query: &str) -> Result<()> {
        let data = self
            .workspace
            .history(Some(query).filter(|q| !q.is_empty()), 100)?;
        let records = data["records"].as_array().cloned().unwrap_or_default();
        let rows: Vec<Row> = records
            .iter()
            .map(|record| Row {
                label: format!(
                    "obs:{}  {}",
                    record["seq"].as_u64().unwrap_or(0),
                    record["source"].as_str().unwrap_or("")
                ),
                detail: record["subject"].as_str().unwrap_or("").to_string(),
                target: Some(format!("obs:{}", record["seq"].as_u64().unwrap_or(0))),
            })
            .collect();
        self.banner = format!("history · {} records in this workspace", rows.len());
        self.set_rows(rows);
        Ok(())
    }

    fn open_selected(&mut self) {
        let Some(index) = self.state.selected() else {
            return;
        };
        let Some(target) = self.rows.get(index).and_then(|r| r.target.clone()) else {
            return;
        };
        if let Err(e) = self.op_read(&target) {
            self.status = format!("error: {e}");
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let current = self.state.selected().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.rows.len() as isize - 1);
        self.state.select(Some(next as usize));
    }

    fn scroll_viewer(&mut self, delta: isize) {
        let max = self.viewer.len().saturating_sub(1);
        let next = (self.viewer_scroll as isize + delta).clamp(0, max as isize);
        self.viewer_scroll = next as usize;
    }
}

const HELP: &str = "\
physis-system-tui — the workspace interface, navigable.

  Tab            move focus: results → viewer → command
  ↑ ↓ / j k      select a row (results) or scroll (viewer)
  PgUp PgDn      page · g / G top / bottom
  Enter          open the selected row; in the command line, run it
  /              start a `find ` command      p   start a `pack ` command
  i l h          inspect · list · history     :   empty command line
  Esc            leave the command line       q   quit
  mouse          click to focus and select, click a selected row to open it,
                 wheel to scroll the pane under the pointer

Commands (same service and arguments as `physis system …`):
  find <query>              rank whole files and remembered work (BM25)
  pack [--budget N] <query> token-budgeted bundle of 40-line windows
  read <path[:start-end] | file:<id or prefix> | obs:N>
  list [limit] · inspect · history [query] · capabilities · help · quit

Reader only: remember, run and export are not bound to any key here.";

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.size());

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", app.workspace.root.display()),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" {}", app.banner)),
        ])),
        chunks[0],
    );

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(chunks[1]);
    app.results_area = panes[0];
    app.viewer_area = panes[1];
    app.command_area = chunks[2];

    let border = |focused: bool| {
        if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let items: Vec<ListItem> = app
        .rows
        .iter()
        .map(|row| {
            ListItem::new(Line::from(vec![
                Span::styled(row.label.clone(), Style::default().fg(Color::White)),
                Span::styled(
                    format!("  {}", row.detail),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border(app.focus == Focus::Results))
                .title(format!(" results ({}) ", app.rows.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, panes[0], &mut app.state);

    let height = panes[1].height.saturating_sub(2) as usize;
    let start = app.viewer_scroll.min(app.viewer.len());
    let visible: Vec<Line> = app.viewer[start..(start + height).min(app.viewer.len())]
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect();
    frame.render_widget(
        Paragraph::new(visible)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border(app.focus == Focus::Viewer))
                    .title(format!(
                        " {} [{}/{}] ",
                        app.viewer_title,
                        start + 1,
                        app.viewer.len().max(1)
                    )),
            )
            .wrap(Wrap { trim: false }),
        panes[1],
    );

    let cursor = if app.focus == Focus::Command { "█" } else { "" };
    frame.render_widget(
        Paragraph::new(format!("> {}{cursor}", app.command)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border(app.focus == Focus::Command))
                .title(" command "),
        ),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", app.status),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[3],
    );
}

fn inside(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}

fn on_mouse(app: &mut App, event: MouseEvent) {
    let (column, row) = (event.column, event.row);
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if inside(app.command_area, column, row) {
                app.focus = Focus::Command;
            } else if inside(app.viewer_area, column, row) {
                app.focus = Focus::Viewer;
            } else if inside(app.results_area, column, row) {
                app.focus = Focus::Results;
                // Row 0 of the pane is its border.
                let offset = row.saturating_sub(app.results_area.y + 1) as usize;
                let index = app.state.offset() + offset;
                if index < app.rows.len() {
                    app.state.select(Some(index));
                    // A click on the row that is already selected opens it, so
                    // a mouse can do the whole errand without the keyboard.
                    if app.last_click == Some(index) {
                        app.open_selected();
                        app.last_click = None;
                    } else {
                        app.last_click = Some(index);
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if inside(app.viewer_area, column, row) {
                app.scroll_viewer(3);
            } else if inside(app.results_area, column, row) {
                app.move_selection(3);
            }
        }
        MouseEventKind::ScrollUp => {
            if inside(app.viewer_area, column, row) {
                app.scroll_viewer(-3);
            } else if inside(app.results_area, column, row) {
                app.move_selection(-3);
            }
        }
        _ => {}
    }
}

fn on_key(app: &mut App, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit = true;
        return;
    }
    if app.focus == Focus::Command {
        match key.code {
            KeyCode::Esc => app.focus = Focus::Results,
            KeyCode::Enter => {
                let line = std::mem::take(&mut app.command);
                app.run_command(&line);
                app.focus = Focus::Results;
            }
            KeyCode::Backspace => {
                app.command.pop();
            }
            KeyCode::Char(c) => app.command.push(c),
            _ => {}
        }
        return;
    }
    let page = app.viewer_area.height.saturating_sub(2).max(1) as isize;
    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Results => Focus::Viewer,
                Focus::Viewer => Focus::Command,
                Focus::Command => Focus::Results,
            }
        }
        KeyCode::Char(':') => {
            app.focus = Focus::Command;
            app.command.clear();
        }
        KeyCode::Char('/') => {
            app.focus = Focus::Command;
            app.command = "find ".into();
        }
        KeyCode::Char('p') => {
            app.focus = Focus::Command;
            app.command = "pack ".into();
        }
        KeyCode::Char('?') => app.run_command("help"),
        KeyCode::Char('i') => app.run_command("inspect"),
        KeyCode::Char('l') => app.run_command("list"),
        KeyCode::Char('h') => app.run_command("history"),
        KeyCode::Enter => app.open_selected(),
        KeyCode::Up | KeyCode::Char('k') => match app.focus {
            Focus::Viewer => app.scroll_viewer(-1),
            _ => app.move_selection(-1),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.focus {
            Focus::Viewer => app.scroll_viewer(1),
            _ => app.move_selection(1),
        },
        KeyCode::PageUp => match app.focus {
            Focus::Viewer => app.scroll_viewer(-page),
            _ => app.move_selection(-page),
        },
        KeyCode::PageDown => match app.focus {
            Focus::Viewer => app.scroll_viewer(page),
            _ => app.move_selection(page),
        },
        KeyCode::Char('g') => match app.focus {
            Focus::Viewer => app.viewer_scroll = 0,
            _ => app.state.select(if app.rows.is_empty() { None } else { Some(0) }),
        },
        KeyCode::Char('G') => match app.focus {
            Focus::Viewer => app.scroll_viewer(app.viewer.len() as isize),
            _ => {
                if !app.rows.is_empty() {
                    app.state.select(Some(app.rows.len() - 1));
                }
            }
        },
        _ => {}
    }
}

struct Screen(Terminal<CrosstermBackend<Stdout>>);

impl Screen {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self(Terminal::new(CrosstermBackend::new(stdout()))?))
    }
}

impl Drop for Screen {
    /// Restoring the terminal in `Drop` rather than at the end of `main` is the
    /// difference between a panic that leaves a usable shell and one that does
    /// not.
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = self.0.show_cursor();
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut root = PathBuf::from(".");
    let mut state: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().context("--root needs a path")?),
            "--state" => state = Some(PathBuf::from(args.next().context("--state needs a path")?)),
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(());
            }
            other => anyhow::bail!("unexpected argument `{other}` (--root, --state, --help)"),
        }
    }
    let root = root.canonicalize().context("workspace root must exist")?;
    let state = state
        .or_else(|| {
            std::env::var_os("PHYSIS_CORE_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| root.join(".physis/system"));
    let workspace = Workspace::open(&root, &state)?;

    let mut screen = Screen::enter()?;
    let mut app = App::new(workspace);
    while !app.quit {
        screen.0.draw(|frame| draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => on_key(&mut app, key),
                Event::Mouse(mouse) => on_mouse(&mut app, mouse),
                _ => {}
            }
        }
    }
    Ok(())
}
