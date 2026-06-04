use physis_pro::ai::provider::ProviderCascade;
use physis_pro::ai::agent::{run_agent, AgentConfig};
use physis_pro::ai::tools::ToolRegistry;
use physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_pro::{OntologyLoader, PhysisConfig, OntologicalMap, Goal};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, List, ListItem},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dotenvy::dotenv;

struct TuiApp {
    map: OntologicalMap,
    transcripts: Vec<String>,
    dreams: Vec<String>,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let app_state = Arc::new(Mutex::new(TuiApp {
        map: OntologicalMap::new(),
        transcripts: Vec::new(),
        dreams: Vec::new(),
        status: "Initializing...".into(),
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_state_clone = app_state.clone();
    
    // Background Processing Thread (Dedicated OS Thread to avoid Send issues with DreamEngine/Rng)
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let cascade = ProviderCascade::from_env();
            let tools = ToolRegistry::new();
            let config = PhysisConfig::default();
            let ontology = OntologyLoader::load_all(&config);
            let _mapper = physis_pro::OntologyMapper::new(ontology, config.embed_dim);
            let embedder = RandomProjectionEmbedder::new(config.embed_dim);
            let mut dream_engine = physis_pro::DreamEngine::new();

            loop {
                {
                    let mut state = app_state_clone.lock().unwrap();
                    state.status = "RECORDING...".into();
                }

                let wav_path = "chunk_tui.wav";
                let _ = Command::new("arecord")
                    .args(["-d", "10", "-f", "S16_LE", "-r", "16000", "-c", "1", "-t", "wav", wav_path])
                    .status();

                {
                    let mut state = app_state_clone.lock().unwrap();
                    state.status = "TRANSCRIBING...".into();
                }

                if let Ok(transcript) = cascade.transcribe(wav_path, None).await {
                    if transcript.trim().is_empty() { continue; }
                    
                    {
                        let mut state = app_state_clone.lock().unwrap();
                        state.transcripts.push(transcript.clone());
                        state.status = "EXTRACTING...".into();
                    }

                    let extraction_prompt = r#"You are Physis. Extract entities and relationships. Output ONLY JSON."#;
                    let agent_config = AgentConfig {
                        system_prompt: extraction_prompt.into(),
                        ..Default::default()
                    };

                    if let Ok(output) = run_agent(&cascade, &tools, &agent_config, &[], &transcript, None, "DATA", None).await {
                        if let (Some(s), Some(e)) = (output.text.find('{'), output.text.rfind('}')) {
                            if let Ok(new_map) = serde_json::from_str::<OntologicalMap>(&output.text[s..=e]) {
                                let mut state = app_state_clone.lock().unwrap();
                                state.map.merge(new_map);
                            }
                        }
                    }

                    // Dreaming
                    let embedding = embedder.embed(&transcript);
                    let goals = vec![Goal::new_vec(embedding)];
                    let dreams = dream_engine.generate_dreams(&goals, 1);
                    let mut state = app_state_clone.lock().unwrap();
                    for d in dreams {
                        state.dreams.push(format!("{}: sim={:.3}", d.id, physis_pro::models::cosine_sim(&d.source, &d.embedding)));
                    }
                }
            }
        });
    });

    // UI Loop
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                ].as_ref())
                .split(f.size());

            // Header
            let state = app_state.lock().unwrap();
            let header = Paragraph::new(format!(" PHYSIS TUI | Status: {} | Entities: {} | Press 'q' to exit", 
                state.status, state.map.entities.len()))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Green)));
            f.render_widget(header, chunks[0]);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ].as_ref())
                .split(chunks[1]);

            // Transcript
            let t_items: Vec<ListItem> = state.transcripts.iter().rev().take(30)
                .map(|t| ListItem::new(format!("> {}", t))).collect();
            let t_list = List::new(t_items)
                .block(Block::default().title(" LIVE TRANSCRIPT ").borders(Borders::ALL))
                .style(Style::default().fg(Color::White));
            f.render_widget(t_list, main_chunks[0]);

            // Entities
            let e_items: Vec<ListItem> = state.map.entities.values()
                .map(|e| ListItem::new(format!("# {}", e.name))).collect();
            let e_list = List::new(e_items)
                .block(Block::default().title(" ONTOLOGY NODES ").borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(e_list, main_chunks[1]);

            // Dreams / Relationships
            let r_items: Vec<ListItem> = state.map.relationships.iter().take(20)
                .map(|r| ListItem::new(format!("{} -[{}]-> {}", r.source, r.predicate, r.target))).collect();
            let r_list = List::new(r_items)
                .block(Block::default().title(" RELATIONSHIPS ").borders(Borders::ALL))
                .style(Style::default().fg(Color::Magenta));
            f.render_widget(r_list, main_chunks[2]);
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}