use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use physis_pro::ai::agent::{run_agent, AgentConfig};
use physis_pro::ai::provider::ProviderCascade;
use physis_pro::ai::tools::ToolRegistry;
use physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_pro::{DreamEngine, Goal, OntologicalMap, OntologyLoader, PhysisConfig};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Settings {
    record_secs: u32,
    transcription_model: String,
    extraction_model: String,
    show_dreams: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            record_secs: 15,
            transcription_model: "whisper-large-v3".into(),
            extraction_model: "llama3.2".into(),
            show_dreams: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct AppState {
    map: OntologicalMap,
    transcripts: Vec<String>,
    dreams: Vec<String>,
    settings: Settings,
}

type SharedState = Arc<Mutex<AppState>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    env_logger::init();

    let shared_state: SharedState = Arc::new(Mutex::new(AppState::default()));
    let config = PhysisConfig::default();
    let ontology = OntologyLoader::load_all(&config);
    let _mapper = physis_pro::OntologyMapper::new(ontology, config.embed_dim);
    let embedder = RandomProjectionEmbedder::new(config.embed_dim);
    let mut dream_engine = DreamEngine::new();

    // Start Web Server
    let app_state_clone = shared_state.clone();
    tokio::spawn(async move {
        let app = Router::new()
            .route("/api/data", get(get_data))
            .route("/api/settings", post(update_settings))
            .route("/", get(index))
            .with_state(app_state_clone);

        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        println!("Physis Voice UI: http://localhost:3000");
        axum::serve(listener, app).await.unwrap();
    });

    let cascade = ProviderCascade::from_env();
    let tools = ToolRegistry::new();

    loop {
        let (wav_path, record_secs, trans_model, ext_model, show_dreams) = {
            let s = shared_state.lock().unwrap();
            (
                "chunk.wav",
                s.settings.record_secs,
                s.settings.transcription_model.clone(),
                s.settings.extraction_model.clone(),
                s.settings.show_dreams,
            )
        };

        println!("Recording {}s with {}...", record_secs, trans_model);
        // arecord with improved quality: 44100Hz Stereo
        let _ = Command::new("arecord")
            .args([
                "-d",
                &record_secs.to_string(),
                "-f",
                "S16_LE",
                "-r",
                "44100",
                "-c",
                "2",
                "-t",
                "wav",
                wav_path,
            ])
            .status();

        if let Ok(transcript) = cascade.transcribe(wav_path, Some(&trans_model)).await {
            if transcript.trim().is_empty() {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            println!("Transcript: {}", transcript);
            {
                let mut state = shared_state.lock().unwrap();
                state.transcripts.push(transcript.clone());
            }

            // Extraction Agent
            let extraction_prompt = r#"You are Physis. Extract entities and relationships.
Output ONLY JSON:
{
  "entities": { "id": {"id":"id", "name":"Name", "kind":"kind", "description":"...", "attributes":{}}},
  "relationships": [{"source":"id1", "target":"id2", "predicate":"verb", "weight":1.0}]
}"#;

            let agent_config = AgentConfig {
                system_prompt: extraction_prompt.into(),
                ..Default::default()
            };

            if let Ok(output) = run_agent(
                &cascade,
                &tools,
                &agent_config,
                &[],
                &transcript,
                None,
                "DATA",
                Some(&ext_model),
            )
            .await
            {
                if let (Some(s), Some(e)) = (output.text.find('{'), output.text.rfind('}')) {
                    if let Ok(new_map) = serde_json::from_str::<OntologicalMap>(&output.text[s..=e])
                    {
                        let mut state = shared_state.lock().unwrap();
                        state.map.merge(new_map);
                    }
                }
            }

            // Dreaming
            if show_dreams {
                let embedding = embedder.embed(&transcript);
                let goals = vec![Goal::new_vec(embedding)];
                let dreams = dream_engine.generate_dreams(&goals, 2);
                let mut state = shared_state.lock().unwrap();
                for d in dreams {
                    state.dreams.push(format!(
                        "{}: sim={:.3}",
                        d.id,
                        physis_pro::models::cosine_sim(&d.source, &d.embedding)
                    ));
                }
            }
        }
    }
}

async fn get_data(State(state): State<SharedState>) -> Json<AppState> {
    Json(state.lock().unwrap().clone())
}

async fn update_settings(
    State(state): State<SharedState>,
    Json(new_settings): Json<Settings>,
) -> Json<Settings> {
    let mut s = state.lock().unwrap();
    s.settings = new_settings.clone();
    Json(new_settings)
}

async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("index_v3.html"))
}
