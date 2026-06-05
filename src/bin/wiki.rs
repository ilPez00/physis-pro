//! physis-pro-wiki — interactive TUI to browse the UberWiki by **content**.
//!
//! Type what you remember of a thing's meaning; the engine embeds the query and
//! ranks nodes by cosine similarity (`/api/v1/reconstruct`), so you find data
//! whose name/type you've forgotten. `/api/v1/classify` adds the DOMAIN×MODE cell
//! for context. Pure HTTP client — no engine changes; works local or remote.
//!
//! Env:
//!   PHYSIS_URL        base URL (default http://127.0.0.1:19876)
//!   PHYSIS_API_TOKEN  Bearer token (sent if set)
//!   PHYSIS_USER       X-Physis-User tenant (sent if set)
//!
//! Keys: type to edit query · Enter search · ↑/↓ or j/k select · h heatmap ·
//!       Esc/Ctrl-C quit.

use std::io::stdout;
use std::time::Duration;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde::Deserialize;

// ── API response shapes (mirror src/bin/web_server.rs) ──────────────────────

#[derive(Deserialize, Clone)]
struct Neighbor {
    id: String,
    cosine_similarity: f32,
    coherence_score: f32,
}

#[derive(Deserialize)]
struct Reconstruction {
    neighbors: Vec<Neighbor>,
    #[allow(dead_code)]
    count: usize,
}

#[derive(Deserialize, Clone, Default)]
struct CellScore {
    domain: String,
    mode: String,
    score: f32,
    entries: Vec<String>,
}

#[derive(Deserialize)]
struct ClassifyResponse {
    top: CellScore,
}

#[derive(Deserialize)]
struct HeatmapResponse {
    table: String,
}

// ── HTTP client ─────────────────────────────────────────────────────────────

struct Api {
    base: String,
    token: Option<String>,
    user: Option<String>,
    http: reqwest::Client,
}

impl Api {
    fn from_env() -> Self {
        Api {
            base: std::env::var("PHYSIS_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:19876".into()),
            token: std::env::var("PHYSIS_API_TOKEN").ok().filter(|t| !t.is_empty()),
            user: std::env::var("PHYSIS_USER").ok().filter(|u| !u.is_empty()),
            http: reqwest::Client::new(),
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut rb = rb;
        if let Some(t) = &self.token {
            rb = rb.bearer_auth(t);
        }
        if let Some(u) = &self.user {
            rb = rb.header("X-Physis-User", u);
        }
        rb
    }

    async fn reconstruct(&self, input: &str) -> anyhow::Result<Vec<Neighbor>> {
        let url = format!("{}/api/v1/reconstruct", self.base);
        let r: Reconstruction = self
            .auth(self.http.post(url).json(&serde_json::json!({ "input": input })))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(r.neighbors)
    }

    async fn classify(&self, text: &str) -> anyhow::Result<CellScore> {
        let url = format!("{}/api/v1/classify", self.base);
        let r: ClassifyResponse = self
            .auth(self.http.post(url).json(&serde_json::json!({ "text": text })))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(r.top)
    }

    async fn heatmap(&self) -> anyhow::Result<String> {
        let url = format!("{}/api/v1/semiotic/heatmap", self.base);
        let r: HeatmapResponse = self
            .auth(self.http.get(url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(r.table)
    }
}

// ── App state ───────────────────────────────────────────────────────────────

struct App {
    query: String,
    results: Vec<Neighbor>,
    top_cell: CellScore,
    list: ListState,
    status: String,
    heatmap: Option<String>, // Some => overlay shown
}

impl App {
    fn new() -> Self {
        let mut list = ListState::default();
        list.select(Some(0));
        App {
            query: String::new(),
            results: Vec::new(),
            top_cell: CellScore::default(),
            list,
            status: "type a content query · Enter to search".into(),
            heatmap: None,
        }
    }

    fn selected(&self) -> Option<&Neighbor> {
        self.list.selected().and_then(|i| self.results.get(i))
    }

    fn move_sel(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let len = self.results.len() as isize;
        let cur = self.list.selected().unwrap_or(0) as isize;
        let next = (cur + delta).rem_euclid(len);
        self.list.select(Some(next as usize));
    }
}

/// +coherence → green, ~0 → gray, −coherence → red.
fn coherence_color(score: f32) -> Color {
    if score > 0.1 {
        Color::Green
    } else if score < -0.1 {
        Color::Red
    } else {
        Color::DarkGray
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let api = Api::from_env();

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let mut app = App::new();
    let res = run(&mut terminal, &mut app, &api).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

async fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    api: &Api,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != crossterm::event::KeyEventKind::Press {
            continue;
        }

        // Global quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(());
        }

        // Heatmap overlay: any key dismisses
        if app.heatmap.is_some() {
            app.heatmap = None;
            continue;
        }

        match key.code {
            KeyCode::Esc => return Ok(()),
            KeyCode::Enter => {
                let q = app.query.trim().to_string();
                if q.is_empty() {
                    continue;
                }
                app.status = "searching…".into();
                terminal.draw(|f| draw(f, app))?;
                match api.reconstruct(&q).await {
                    Ok(n) => {
                        app.results = n;
                        app.list.select(Some(0));
                        app.status = format!("{} matches · {}", app.results.len(), &q);
                    }
                    Err(e) => app.status = format!("reconstruct failed: {e}"),
                }
                app.top_cell = api.classify(&q).await.unwrap_or_default();
            }
            KeyCode::Char('h') if app.query.is_empty() => {
                match api.heatmap().await {
                    Ok(t) => app.heatmap = Some(t),
                    Err(e) => app.status = format!("heatmap failed: {e}"),
                }
            }
            KeyCode::Down => app.move_sel(1),
            KeyCode::Up => app.move_sel(-1),
            KeyCode::Char('j') if app.query.is_empty() => app.move_sel(1),
            KeyCode::Char('k') if app.query.is_empty() => app.move_sel(-1),
            KeyCode::Backspace => {
                app.query.pop();
            }
            KeyCode::Char(c) => app.query.push(c),
            _ => {}
        }
    }
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    // Heatmap overlay takes the whole screen
    if let Some(table) = &app.heatmap {
        let p = Paragraph::new(table.as_str())
            .block(
                Block::default()
                    .title(" SEMIOTIC HEATMAP — any key to close ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(p, f.size());
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search box
            Constraint::Min(0),    // results | preview
            Constraint::Length(1), // help
        ])
        .split(f.size());

    // Search box
    let search = Paragraph::new(Line::from(vec![
        Span::styled("search ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &app.query,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("▏", Style::default().fg(Color::Cyan)),
    ]))
    .block(
        Block::default()
            .title(" PHYSIS WIKI — browse by content ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(search, rows[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    // Results (cosine-ranked, coherence-colored)
    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|n| {
            ListItem::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(coherence_color(n.coherence_score))),
                Span::styled(
                    format!("{:.2}  ", n.cosine_similarity),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(n.id.clone()),
            ]))
        })
        .collect();
    let mut list_state = app.list.clone();
    let results = List::new(items)
        .block(
            Block::default()
                .title(format!(" RESULTS ({}) ", app.results.len()))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(results, main[0], &mut list_state);

    // Preview
    let mut lines: Vec<Line> = Vec::new();
    if let Some(n) = app.selected() {
        lines.push(Line::from(Span::styled(
            n.id.clone(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!("cosine     {:.4}", n.cosine_similarity)));
        lines.push(Line::from(vec![
            Span::raw("coherence  "),
            Span::styled(
                format!("{:+.2}", n.coherence_score),
                Style::default().fg(coherence_color(n.coherence_score)),
            ),
        ]));
        lines.push(Line::from(""));
    }
    let c = &app.top_cell;
    if !c.domain.is_empty() {
        lines.push(Line::from(Span::styled(
            "── classified as ──",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} × {}", c.domain, c.mode),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  ({:.2})", c.score), Style::default().fg(Color::Yellow)),
        ]));
        if !c.entries.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("nearby: {}", c.entries.join(", ")),
                Style::default().fg(Color::Gray),
            )));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "no results yet — type a query and press Enter",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let preview = Paragraph::new(lines)
        .block(Block::default().title(" PREVIEW ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(preview, main[1]);

    // Help / status
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" ↑/↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("move  "),
        Span::styled("Enter ", Style::default().fg(Color::Cyan)),
        Span::raw("search  "),
        Span::styled("h ", Style::default().fg(Color::Cyan)),
        Span::raw("heatmap  "),
        Span::styled("Esc ", Style::default().fg(Color::Cyan)),
        Span::raw("quit   "),
        Span::styled(&app.status, Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(help, rows[2]);
}
