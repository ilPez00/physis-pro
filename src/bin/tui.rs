//! physis-pro-tui — control panel for the physis engine.
//!
//! Three tabs (cycle with Tab):
//!   STATUS   — live engine health (HTTP), embedder, node/edge counts, connection.
//!   MACHINES — pick a philosophical machine and run it on a sample (in-process).
//!   CONFIG   — edit + save ~/.physis/config.json (dream/pdca/scan/embed knobs).
//!
//! Keys: Tab switch · ↑/↓ select · Enter run (machines) · +/- edit (config) ·
//!       s save (config) · r refresh (status) · q quit.

use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dotenvy::dotenv;

use chrono::{Duration as ChronoDuration, Utc};
use physis_pro::config::PhysisConfig;
use physis_pro::machines::{chronos, elenchus, harmony, lex, polis, soma, techne};

const TABS: [&str; 3] = ["STATUS", "MACHINES", "CONFIG"];
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

struct App {
    tab: usize,
    machine_sel: usize,
    machine_out: String,
    cfg: PhysisConfig,
    cfg_sel: usize,
    status_msg: String,
    url: String,
    token: Option<String>,
    /// The engine process, when started from here.
    engine: Option<std::process::Child>,
}

/// Resolve the physis-pro-web binary: sibling of this exe, else rely on PATH.
fn engine_bin() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sib = dir.join("physis-pro-web");
            if sib.exists() { return sib; }
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
        Ok(child) => { app.status_msg = format!("engine started (pid {})", child.id()); app.engine = Some(child); }
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
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => { app.status_msg = format!("reload runtime: {e}"); return; }
    };
    app.status_msg = rt.block_on(async move {
        let client = reqwest::Client::new();
        let mut rb = client.post(&url);
        if let Some(t) = token { rb = rb.bearer_auth(t); }
        match rb.send().await {
            Ok(r) => match r.json::<serde_json::Value>().await {
                Ok(v) => v.get("note").and_then(|n| n.as_str()).unwrap_or("reloaded").to_string(),
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
        0 => cfg.network_scan_interval_secs = (cfg.network_scan_interval_secs as i64 + (s * 5.0) as i64).max(1) as u64,
        1 => cfg.dream_batch_size = (cfg.dream_batch_size as i64 + s as i64).max(1) as usize,
        2 => cfg.pdca_stagnant_threshold = (cfg.pdca_stagnant_threshold + s * 0.05).clamp(0.0, 1.0),
        3 => cfg.pdca_stagnant_window = (cfg.pdca_stagnant_window as i64 + s as i64).max(1) as usize,
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
    let token = std::env::var("PHYSIS_API_TOKEN").ok().filter(|t| !t.is_empty());

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
                        if let Some(t) = &token { rb = rb.bearer_auth(t); }
                        if let Ok(resp) = rb.send().await {
                            if let Ok(v) = resp.json::<serde_json::Value>().await {
                                st.embedder = v.get("kind").and_then(|x| x.as_str()).unwrap_or("?").to_string();
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

    let mut app = App {
        tab: 0,
        machine_sel: 0,
        machine_out: "press Enter to run".into(),
        cfg: load_config(),
        cfg_sel: 0,
        status_msg: format!("engine: {url}"),
        url,
        token,
        engine: None,
    };

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

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    status: &Arc<Mutex<EngineStatus>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw(f, app, status))?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != crossterm::event::KeyEventKind::Press {
            continue;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(());
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Tab => app.tab = (app.tab + 1) % TABS.len(),
            KeyCode::BackTab => app.tab = (app.tab + TABS.len() - 1) % TABS.len(),
            KeyCode::Down | KeyCode::Char('j') => match app.tab {
                1 => app.machine_sel = (app.machine_sel + 1) % MACHINES.len(),
                2 => app.cfg_sel = (app.cfg_sel + 1) % CFG_FIELDS.len(),
                _ => {}
            },
            KeyCode::Up | KeyCode::Char('k') => match app.tab {
                1 => app.machine_sel = (app.machine_sel + MACHINES.len() - 1) % MACHINES.len(),
                2 => app.cfg_sel = (app.cfg_sel + CFG_FIELDS.len() - 1) % CFG_FIELDS.len(),
                _ => {}
            },
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
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(f.size());

    // Tab bar
    let tabs = Tabs::new(TABS.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
        .select(app.tab)
        .block(Block::default().title(" PHYSIS CONTROL ").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, rows[0]);

    match app.tab {
        0 => draw_status(f, rows[1], app, status),
        1 => draw_machines(f, rows[1], app),
        2 => draw_config(f, rows[1], app),
        _ => {}
    }

    let help = match app.tab {
        1 => " Tab switch · ↑/↓ select · Enter run · q quit",
        2 => " Tab switch · ↑/↓ field · +/- edit · s save · q quit",
        _ => " Tab switch · s start · x stop · r reload-config · q quit",
    };
    let foot = Paragraph::new(Line::from(vec![
        Span::styled(help, Style::default().fg(Color::Cyan)),
        Span::styled(format!("   {}", app.status_msg), Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(foot, rows[2]);
}

fn draw_status(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App, status: &Arc<Mutex<EngineStatus>>) {
    let st = status.lock().unwrap();
    let dot = if st.online { Span::styled("● ONLINE", Style::default().fg(Color::Green)) }
              else { Span::styled("● OFFLINE", Style::default().fg(Color::Red)) };
    let mut lines = vec![
        Line::from(vec![Span::raw("engine   "), dot]),
        Line::from(format!("url      {}", app.url)),
        Line::from(format!("embedder {}", if st.embedder.is_empty() { "—" } else { &st.embedder })),
        Line::from(""),
    ];
    if !st.detail.is_empty() {
        lines.push(Line::from(Span::styled("/api/v1/embedder:", Style::default().fg(Color::DarkGray))));
        lines.push(Line::from(st.detail.clone()));
    } else if !st.online {
        lines.push(Line::from(Span::styled("start the engine: physis-pro-web", Style::default().fg(Color::DarkGray))));
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

    let items: Vec<ListItem> = MACHINES.iter().map(|(n, d)| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("{n:<10}"), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(*d, Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();
    let mut state = ListState::default();
    state.select(Some(app.machine_sel));
    let list = List::new(items)
        .block(Block::default().title(" MACHINES ").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, cols[0], &mut state);

    let out = Paragraph::new(app.machine_out.as_str())
        .block(Block::default().title(format!(" {} OUTPUT ", MACHINES[app.machine_sel].0)).borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(out, cols[1]);
}

fn draw_config(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let mut lines = vec![
        Line::from(Span::styled(format!("file: {}", cfg_path(&app.cfg).display()), Style::default().fg(Color::DarkGray))),
        Line::from(""),
    ];
    for (i, name) in CFG_FIELDS.iter().enumerate() {
        let sel = i == app.cfg_sel;
        lines.push(Line::from(vec![
            Span::styled(if sel { "▶ " } else { "  " }, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{name:<30}"), Style::default().fg(if sel { Color::White } else { Color::Gray }).add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() })),
            Span::styled(cfg_value(&app.cfg, i), Style::default().fg(Color::Yellow)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("+/- to change · s to save", Style::default().fg(Color::DarkGray))));
    let p = Paragraph::new(lines).block(Block::default().title(" CONFIG (~/.physis/config.json) ").borders(Borders::ALL));
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
    let ev = |label: &str, days_ago: i64| chronos::TimedEvent { label: label.into(), at: now - ChronoDuration::days(days_ago) };
    let mut events: Vec<chronos::TimedEvent> = (0..7).map(|i| ev("gym", 2 + i * 2)).collect();
    for d in [32, 25, 18, 11] { events.push(ev("standup", d)); }
    for d in [21, 9, 2] { events.push(ev("call mum", d)); }
    let rhythms = chronos::detect_rhythms(&events);
    let mut s = format!("{} events → {} rhythms:\n", events.len(), rhythms.len());
    for r in &rhythms {
        s += &format!("  {:<10} every {:>4.1}d  conf {:.2}  next {}\n", r.label, r.period_days, r.confidence, r.next_due.format("%Y-%m-%d"));
    }
    let slip = chronos::slipping(&rhythms, now, 0.5);
    if !slip.is_empty() {
        s += &format!("slipping: {}", slip.iter().map(|r| r.label.as_str()).collect::<Vec<_>>().join(", "));
    }
    s
}

fn techne_demo() -> String {
    let f = |path: &str, lines: &[&str]| techne::SourceFile { path: path.into(), lines: lines.iter().map(|s| s.to_string()).collect() };
    let files = vec![
        f("src/embed/mod.rs", &[]),
        f("src/classify.rs", &["use crate::embed::VectorEmbed;"]),
        f("src/core.rs", &["use crate::trie::Trie;", "use crate::embed::VectorEmbed;"]),
        f("src/trie.rs", &[]),
        f("src/bin/web_server.rs", &["use crate::core::PhysisCore;"]),
        f("src/spanner.rs", &["use crate::core::PhysisCore;"]),
    ];
    let mut s = String::from("capabilities:\n");
    for c in techne::classify_capabilities(&files) {
        s += &format!("  {:<14} {}\n", c.name, c.files.join(", "));
    }
    let edges = techne::dependency_edges(&files);
    s += &format!("impact of `embed`: {}", techne::impact_of(&edges, "embed").join(", "));
    s
}

fn soma_demo() -> String {
    let s = |day: u32, load: f32, recovery: f32| soma::Session { day, activity: "lift".into(), load, recovery };
    let sessions = vec![s(1, 0.5, 0.9), s(2, 0.7, 0.8), s(3, 0.8, 0.6), s(4, 0.8, 0.5), s(5, 0.9, 0.4), s(6, 0.9, 0.35)];
    let mut out = String::new();
    for x in &sessions {
        out += &format!("  day {}  load {:.1} rec {:.1}  {:?}\n", x.day, x.load, x.recovery, soma::classify(x));
    }
    let d = soma::drift(&sessions, 6);
    out += &format!("drift {:+.2} → {}", d, if d > 0.5 { "overreaching risk" } else { "stable" });
    out
}

fn elenchus_demo() -> String {
    let st = |id: &str, kind: elenchus::Kind, text: &str, target: Option<&str>| elenchus::Statement { id: id.into(), kind, text: text.into(), target: target.map(|s| s.into()) };
    let arg = vec![
        st("c1", elenchus::Kind::Claim, "Remote work raises output", None),
        st("w1", elenchus::Kind::Warrant, "fewer commute hours", Some("c1")),
        st("c2", elenchus::Kind::Claim, "Open offices boost collaboration", None),
        st("r2", elenchus::Kind::Rebuttal, "studies show -70% face time", Some("c2")),
        st("c3", elenchus::Kind::Claim, "Four-day weeks always better", None),
    ];
    let mut s = String::new();
    for (id, standing) in elenchus::evaluate(&arg) {
        let t = arg.iter().find(|x| x.id == id).unwrap();
        s += &format!("  {:<3} {:<12?} {}\n", id, standing, t.text);
    }
    s += &format!("press: {:?}\nrecalibrate: {:?}", elenchus::unsupported_claims(&arg), elenchus::contested_claims(&arg));
    s
}

fn lex_demo() -> String {
    let c = |id: &str, kind: lex::Kind, requires: &[&str]| lex::Clause { id: id.into(), kind, text: id.into(), requires: requires.iter().map(|s| s.to_string()).collect() };
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
    let i = |from: &str, to: &str, w: f32| polis::Interaction { from: from.into(), to: to.into(), weight: w };
    let interactions = vec![
        i("you", "ana", 9.0), i("ana", "you", 8.0),
        i("you", "ben", 7.0), i("ben", "you", 1.0),
        i("ana", "ben", 4.0), i("ben", "ana", 4.0),
    ];
    let ties = polis::ties(&interactions);
    let mut s = String::from("ties:\n");
    for t in &ties {
        s += &format!("  {:<4}-{:<4} strength {:>4.1} recip {:.2}\n", t.a, t.b, t.strength, t.reciprocity);
    }
    let fade = polis::fading(&ties, 0.5);
    if !fade.is_empty() {
        s += &format!("fading: {}", fade.iter().map(|t| format!("{}-{}", t.a, t.b)).collect::<Vec<_>>().join(", "));
    }
    s
}
