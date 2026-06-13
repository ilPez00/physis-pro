//! CLI application orchestrator — parses subcommands via clap and dispatches
//! to PhysisApp methods for scanning, querying, dreaming, and evaluating.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::actor::PDCActor;
use crate::config::{EmbedderKindConfig, OntologyLoader, PhysisConfig};
use crate::dream::DreamEngine;
#[cfg(feature = "embed-onnx")]
use crate::embed::clip::ClipEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::jina::JinaEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::onnx::OnnxEmbedder;
use crate::embed::RandomProjectionEmbedder;
use crate::embed::VectorEmbed;
use crate::linguistic::{LinguisticLense, LinguisticRouter};
use crate::mapper::OntologyMapper;
use crate::models::{Goal, Score};
use crate::network::NetworkScanner;
use crate::output;

#[derive(Parser)]
#[command(name = "physis", version, about = "Ontological mapper and PDCA dream engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan a directory and build ontological map
    Scan {
        /// Directory to scan
        dir: PathBuf,
        /// Output format (wiki, json, mermaid)
        #[arg(short, long, default_value = "wiki")]
        format: String,
    },
    /// Deep AI-powered scan of files and images
    DeepScan {
        /// Directory to scan
        dir: PathBuf,
    },
    /// Query the ontology trie
    Query {
        /// Search query
        query: String,
        #[arg(short, long, default_value = "10")]
        max_results: usize,
    },
    /// Generate stochastic dreams
    Dream {
        /// Number of dreams to generate
        #[arg(short, long, default_value = "5")]
        count: usize,
    },
    /// Evaluate a dream
    Evaluate {
        /// Dream ID
        id: String,
        /// Grade (0.0 - 1.0)
        grade: Score,
    },
    /// Start MCP server
    #[cfg(feature = "mcp")]
    Serve,
    /// Watch directories for changes
    Watch {
        /// Directories to watch
        dirs: Vec<PathBuf>,
    },
    /// Show stats
    Stats,
    /// Print config
    Config,
    /// Translate text through a linguistic lense
    Translate {
        /// Text to translate
        text: String,
        /// Lense to apply (wenyan, piraha, sanskrit). If omitted, applies all enabled.
        #[arg(short, long)]
        lense: Option<String>,
    },
    /// Classify a text file against the semiotic grid and output a map
    Classify {
        /// Path to .txt file
        file: PathBuf,
    },
}

/// Top-level application state: config, ontology, mapper, PDCA actor, dream engine, goals.
pub struct PhysisApp {
    /// Engine configuration.
    pub config: PhysisConfig,
    /// Loaded ontology domains.
    pub ontology: OntologyLoader,
    /// Ontological mapper with trie and vector search.
    pub mapper: OntologyMapper,
    /// PDCA cycle orchestrator.
    pub actor: PDCActor,
    /// Dream generation engine.
    pub dreams: DreamEngine,
    /// Current vector-space goals.
    pub goals: Vec<Goal>,
    /// Embedder for classify.
    embedder: Box<dyn VectorEmbed>,
    /// Pre-computed centroids per DOMAIN\x00MODE cell.
    pub cell_centroids: HashMap<String, Vec<f32>>,
    /// Nearest-entry classifier, built once (one full-ontology embedding pass).
    pub classifier: crate::classify::CellClassifier,
}

impl PhysisApp {
    /// Build a new PhysisApp from config — loads ontology, creates mapper, actor, dream engine.
    pub fn new(config: PhysisConfig) -> Self {
        let ontology = OntologyLoader::load_all(&config);
        let mapper = OntologyMapper::new(ontology.clone(), config.embed_dim);
        let actor = PDCActor::new(config.pdca_stagnant_threshold, config.pdca_stagnant_window);
        let dreams = DreamEngine::new();

        let embedder: Box<dyn VectorEmbed> = {
            #[cfg(feature = "embed-onnx")]
            {
                // Self-test gate (same as the web server): a candidate that loads
                // but emits non-semantic vectors (e.g. Jina fp16 → garbage) is
                // skipped so the next working candidate wins. Without this gate
                // the CLI silently classified with noise.
                let gate = |opt: Option<Box<dyn VectorEmbed>>| {
                    opt.filter(|e| crate::embed::semantic_self_test(e.as_ref()))
                };
                let try_primary = || -> Option<Box<dyn VectorEmbed>> {
                    let ec = config.embedders.primary.as_ref()?;
                    if !ec.enabled { return None; }
                    match ec.kind {
                        EmbedderKindConfig::JinaV2 => {
                            let j = JinaEmbedder::with_model_dir(&ec.model_dir);
                            j.is_available().then(|| Box::new(j) as Box<dyn VectorEmbed>)
                        }
                        EmbedderKindConfig::Clip => {
                            let c = ClipEmbedder::with_config(&config.onnx);
                            c.is_available().then(|| Box::new(c) as Box<dyn VectorEmbed>)
                        }
                        EmbedderKindConfig::MiniLM => {
                            let m = OnnxEmbedder::with_config(&config.onnx);
                            m.is_available().then(|| Box::new(m) as Box<dyn VectorEmbed>)
                        }
                        EmbedderKindConfig::BgeBase => {
                            let cfg = crate::config::OnnxConfig {
                                enabled: true,
                                model_dir: Some("models/bge-base-en-v1.5".to_string()),
                                dim: 768,
                                max_length: 512,
                                pooling: crate::config::PoolingStrategy::Mean,
                            };
                            let b = OnnxEmbedder::with_config(&cfg);
                            b.is_available().then(|| Box::new(b) as Box<dyn VectorEmbed>)
                        }
                        EmbedderKindConfig::BgeM3 => {
                            let cfg = crate::config::OnnxConfig {
                                enabled: true,
                                model_dir: Some("models/bge-m3".to_string()),
                                dim: 1024,
                                max_length: 512,
                                pooling: crate::config::PoolingStrategy::Cls,
                            };
                            let b = OnnxEmbedder::with_config(&cfg);
                            b.is_available().then(|| Box::new(b) as Box<dyn VectorEmbed>)
                        }
                        EmbedderKindConfig::RandomProjection => {
                            Some(Box::new(RandomProjectionEmbedder::new(config.embed_dim)))
                        }
                    }
                };
                // Mirror the web cascade: Jina → BGE → BGE-M3 → CLIP → MiniLM → RP,
                // every candidate self-test-gated. Both halves matter: this cascade
                // used to lack BGE *and* the gate, so the broken Jina fp16 export
                // loaded first and every CLI classification was noise (~0.06 top
                // score on real text). Found by dogfooding `physis-pro classify`.
                gate(try_primary())
                    .or_else(|| gate({
                        let j = JinaEmbedder::with_model_dir("models/jina-clip-v2");
                        j.is_available().then(|| Box::new(j) as Box<dyn VectorEmbed>)
                    }))
                    .or_else(|| gate({
                        let cfg = crate::config::OnnxConfig {
                            enabled: true,
                            model_dir: Some("models/bge-base-en-v1.5".to_string()),
                            dim: 768,
                            max_length: 512,
                            pooling: crate::config::PoolingStrategy::Mean,
                        };
                        let b = OnnxEmbedder::with_config(&cfg);
                        b.is_available().then(|| Box::new(b) as Box<dyn VectorEmbed>)
                    }))
                    .or_else(|| gate({
                        let cfg = crate::config::OnnxConfig {
                            enabled: true,
                            model_dir: Some("models/bge-m3".to_string()),
                            dim: 1024,
                            max_length: 512,
                            pooling: crate::config::PoolingStrategy::Cls,
                        };
                        let b = OnnxEmbedder::with_config(&cfg);
                        b.is_available().then(|| Box::new(b) as Box<dyn VectorEmbed>)
                    }))
                    .or_else(|| gate({
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available().then(|| Box::new(c) as Box<dyn VectorEmbed>)
                    }))
                    .or_else(|| gate({
                        let m = OnnxEmbedder::with_config(&config.onnx);
                        m.is_available().then(|| Box::new(m) as Box<dyn VectorEmbed>)
                    }))
                    .unwrap_or_else(|| Box::new(RandomProjectionEmbedder::new(config.embed_dim)))
            }
            #[cfg(not(feature = "embed-onnx"))]
            Box::new(RandomProjectionEmbedder::new(config.embed_dim))
        };
        // Embedder provenance on stderr — the CLI equivalent of the web banner.
        // A silent fall-through to random projection makes every verdict noise
        // (PREMORTEM P0); at least say which embedder is answering.
        eprintln!("[physis] embedder: dim={}", embedder.dimension());

        // ONE full-ontology embedding pass: build the nearest-entry classifier,
        // then derive the legacy mean centroids from its entry embeddings.
        // (Previously this constructor embedded all ~650 entries for centroids
        // AND each command rebuilt the classifier — a second identical pass.)
        let classifier = crate::classify::CellClassifier::build(&ontology, embedder.as_ref());
        let cell_centroids = classifier.cell_centroids();

        Self {
            config,
            ontology,
            mapper,
            actor,
            dreams,
            goals: Vec::new(),
            embedder,
            cell_centroids,
            classifier,
        }
    }

    /// Scan a directory and return ontology output in the requested format (wiki/json/mermaid).
    pub fn run_scan(&mut self, dir: &std::path::Path, format: &str) -> String {
        let goals = self.mapper.map_filesystem(dir, None);
        self.goals = goals;
        // trie lives in mapper; dreams uses vector operations directly

        match format {
            "json" => output::format_json_graph(&self.mapper.trie),
            "mermaid" => output::format_mermaid_mindmap(&self.mapper.trie, "Physis Scan"),
            _ => output::format_wiki(&self.mapper.trie, &self.goals, "Physis Scan"),
        }
    }

    /// Search the ontology trie for paths matching the query tokens.
    pub fn run_query(&self, query: &str, max_results: usize) -> Vec<Vec<String>> {
        let words: Vec<&str> = query.split_whitespace().collect();
        let tids: Vec<u32> = words
            .iter()
            .filter_map(|w| self.mapper.trie.token_id(w))
            .collect();
        if tids.is_empty() {
            return vec![];
        }
        self.mapper.trie.prefix_search(&tids, 2, max_results)
    }

    /// Generate `count` dreams from the current goal set.
    pub fn run_dream(&mut self, count: usize) -> Vec<crate::models::Dream> {
        self.dreams.generate_dreams(&self.goals, count)
    }

    /// Grade a dream by ID. Returns false if no dream with that ID exists.
    pub fn run_evaluate(&mut self, id: &str, grade: Score) -> bool {
        self.dreams.evaluate_dream(id, grade)
    }

    /// Deep AI-powered scan — sends file content to LLM for ontological map extraction.
    pub async fn run_deep_scan(&mut self, dir: &std::path::Path) -> anyhow::Result<String> {
        use crate::ai::provider::ProviderCascade;
        use crate::ai::agent::{run_agent, AgentConfig};
        use crate::ai::tools::ToolRegistry;
        use crate::scanner::scan_project;
        use base64::prelude::*;

        println!("Starting Deep Scan on {}...", dir.display());
        let cascade = ProviderCascade::from_env();
        let tools = ToolRegistry::new();
        let mut global_map = crate::models::OntologicalMap::new();

        let files = scan_project(dir, None);
        println!("Found {} candidate files.", files.len());

        for file in files {
            println!("Processing {}...", file.path);
            let ext = file.ext.to_lowercase();
            
            let result = if [".png", ".jpg", ".jpeg"].contains(&ext.as_str()) {
                // Image Deep Scan
                let bytes = std::fs::read(&file.abs_path)?;
                let b64 = BASE64_STANDARD.encode(bytes);
                let config = AgentConfig {
                    system_prompt: r#"You are Physis Vision. Extract an ontological map (JSON).
Output ONLY a JSON object with this EXACT structure:
{
  "entities": {
    "entity_id": {"id":"entity_id", "name":"Human Name", "kind":"kind", "description":"...", "attributes":{}}
  },
  "relationships": [
    {"source":"entity_id1", "target":"entity_id2", "predicate":"verb", "weight":1.0}
  ]
}
No other text."#.into(),
                    ..Default::default()
                };
                run_agent(&cascade, &tools, &config, &[], &format!("Analyze image: {}", file.path), Some(&b64), "DATA", None).await
            } else {
                // Text Deep Scan
                let text = std::fs::read_to_string(&file.abs_path)?;
                let config = AgentConfig {
                    system_prompt: r#"You are Physis. Extract an ontological map (JSON).
Output ONLY a JSON object with this EXACT structure:
{
  "entities": {
    "entity_id": {"id":"entity_id", "name":"Human Name", "kind":"kind", "description":"...", "attributes":{}}
  },
  "relationships": [
    {"source":"entity_id1", "target":"entity_id2", "predicate":"verb", "weight":1.0}
  ]
}
No other text."#.into(),
                    ..Default::default()
                };
                run_agent(&cascade, &tools, &config, &[], &format!("Analyze text ({}): \n{}", file.path, text), None, "DATA", None).await
            };

            if let Ok(output) = result {
                if let (Some(s), Some(e)) = (output.text.find('{'), output.text.rfind('}')) {
                    if let Ok(new_map) = serde_json::from_str::<crate::models::OntologicalMap>(&output.text[s..=e]) {
                        global_map.merge(new_map);
                        println!("Merged ontology for {}.", file.path);
                    }
                }
            }
        }

        Ok(serde_json::to_string_pretty(&global_map)?)
    }

    /// Watch directories for file changes using hash-cache diffing.
    pub fn run_watch(&mut self, dirs: Vec<PathBuf>) -> String {
        let cache_path = self.config.data_dir.join("hash_cache.json");
        let scanner = NetworkScanner::new(dirs.clone(), cache_path, self.config.network_scan_interval_secs);
        let diffs = scanner.scan_all();
        let mut changes = Vec::new();
        for diff in &diffs {
            changes.push(format!("{} new, {} changed, {} deleted",
                diff.summary.new, diff.summary.changed, diff.summary.deleted));
        }
        changes.join("\n")
    }

    /// Print trie statistics and PDCA metrics.
    pub fn run_stats(&self) -> String {
        let stats = self.mapper.stats();
        let mut out = String::new();
        out.push_str("=== Physis Stats ===\n");
        let mut pairs: Vec<_> = stats.into_iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in pairs {
            out.push_str(&format!("  {k}: {v}\n"));
        }
        let actor_stats = self.actor.stats(&self.goals);
        out.push_str(&format!("  total_actions: {}\n", actor_stats.total_actions));
        out.push_str(&format!("  avg_grade: {:.3}\n", actor_stats.avg_grade));
        if actor_stats.stagnant_count > 0 {
            out.push_str(&format!("  stagnant_goals: {}\n", actor_stats.stagnant_count));
        }
        out
    }

    /// Translate `text` through the specified lense (or all enabled lenses).
    pub fn run_translate(&self, text: &str, lense: Option<&str>) -> String {
        let router = LinguisticRouter::with_config(&self.config.linguistic);
        match lense {
            Some(l) => {
                let lense = match l.to_lowercase().as_str() {
                    "wenyan" => LinguisticLense::Wenyan,
                    "piraha" => LinguisticLense::Piraha,
                    "sanskrit" => LinguisticLense::Sanskrit,
                    _ => return format!("Unknown lense: {l}. Use: wenyan, piraha, sanskrit"),
                };
                router.route(text, lense)
            }
            None => {
                let results = router.route_all(text);
                if results.is_empty() {
                    return "No lenses are enabled.".to_string();
                }
                results
                    .iter()
                    .map(|(k, v)| format!("[{}] {}", k.as_str(), v))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }

    /// Classify a text file against the semiotic grid.
    /// Returns a formatted map: ranked domain×mode cells with scores + contributing entries.
    /// Ontology discovery over a directory: every non-empty line of every
    /// .txt/.md/.log/.csv file is one corpus text (notes and logs are
    /// line-oriented). Returns the human report; when `json_out` is set, the
    /// ready-to-edit ontology draft is written there.
    pub fn run_discover(
        &self,
        dir: &std::path::Path,
        json_out: Option<&std::path::Path>,
        threshold: Option<f32>,
    ) -> anyhow::Result<String> {
        const MAX_TEXTS: usize = 5000;
        let mut texts: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let ok_ext = matches!(
                entry.path().extension().and_then(|e| e.to_str()),
                Some("txt" | "md" | "log" | "csv")
            );
            if !entry.file_type().is_file() || !ok_ext {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.len() > 10 {
                        texts.push(line.to_string());
                        if texts.len() >= MAX_TEXTS {
                            break;
                        }
                    }
                }
            }
            if texts.len() >= MAX_TEXTS {
                break;
            }
        }
        if texts.is_empty() {
            anyhow::bail!("no corpus lines found under {} (.txt/.md/.log/.csv)", dir.display());
        }

        let mut cfg = crate::discovery::DiscoveryConfig::default();
        if let Some(t) = threshold {
            cfg.coverage_threshold = t;
        }
        let report =
            crate::discovery::discover(&texts, &self.classifier, self.embedder.as_ref(), &cfg);

        let mut out = String::new();
        out.push_str("━━━ Physis Ontology Discovery ━━━\n");
        out.push_str(&format!(
            "Corpus: {} texts · covered {} · unmapped {} (threshold {:.2})\n",
            report.total, report.covered, report.unmapped, cfg.coverage_threshold
        ));
        out.push_str(&format!(
            "Best-entry cosine: min {:.2} · mean {:.2} · max {:.2}  \
             (tune --threshold between mean and max to surface gaps)\n\n",
            report.sim_min, report.sim_mean, report.sim_max
        ));
        for p in &report.proposals {
            out.push_str(&format!(
                "▶ {} — {} texts, coverage {:.2}, nearest cell {}×{}\n",
                p.name, p.count, p.coverage, p.domain, p.mode
            ));
            out.push_str(&format!("   hints: {}\n", p.hints.join(", ")));
            for s in &p.samples {
                let s = if s.len() > 90 { &s[..90] } else { s };
                out.push_str(&format!("   · {s}\n"));
            }
        }
        if report.proposals.is_empty() {
            out.push_str("No proposals — the current ontology covers this corpus.\n");
        }
        if let Some(path) = json_out {
            std::fs::write(path, crate::discovery::to_ontology_json(&report, "discovered"))?;
            out.push_str(&format!("\nDraft ontology written to {}\n", path.display()));
        }
        Ok(out)
    }

    pub fn run_classify(&self, file: &std::path::Path) -> anyhow::Result<String> {
        // Determine if it's an image file
        let is_image = matches!(
            file.extension().and_then(|e| e.to_str()),
            Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp")
        );

        let embedding = if is_image {
            let bytes = std::fs::read(file)
                .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;
            self.embedder.embed_image(&bytes)
                .ok_or_else(|| anyhow::anyhow!("Image embedding not available for this embedder"))
                .map_err(|e| anyhow::anyhow!("Failed to classify image {}: {}", file.display(), e))?
        } else {
            let text = std::fs::read_to_string(file)
                .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;
            self.embedder.embed(&text)
        };

        // Nearest-entry scoring via the shared classifier — the same path the
        // web server and the gated accuracy test use. The previous mean-centroid
        // scoring predated the P3 fix and compressed every score toward zero.
        let results = self.classifier.classify(&embedding);

        let mut out = String::new();
        out.push_str("━━━ Physis Classification Map ━━━\n");
        out.push_str(&format!("File: {}\n\n", file.display()));
        for (i, r) in results.iter().enumerate() {
            if i == 0 {
                out.push_str(&format!("★ TOP: {}×{} — {:.3}\n", r.domain, r.mode, r.score));
                out.push_str(&format!(
                    "   Reasoning: nearest entries: {}\n",
                    r.entries.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                ));
                out.push('\n');
            } else if i < 5 {
                out.push_str(&format!("  {}×{} — {:.3}\n", r.domain, r.mode, r.score));
            }
        }
        out.push_str(&format!("\n{} cells scored (top 5 shown)\n", results.len()));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parser_scan() {
        let cli = Cli::try_parse_from(["physis", "scan", "/tmp"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Scan { dir, format } => {
                assert_eq!(dir, std::path::PathBuf::from("/tmp"));
                assert_eq!(format, "wiki");
            }
            _ => panic!("expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parser_scan_with_format() {
        let cli = Cli::try_parse_from(["physis", "scan", "/tmp", "--format", "json"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Scan { format, .. } => assert_eq!(format, "json"),
            _ => panic!("expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parser_query() {
        let cli = Cli::try_parse_from(["physis", "query", "hello world"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Query { query, max_results } => {
                assert_eq!(query, "hello world");
                assert_eq!(max_results, 10);
            }
            _ => panic!("expected Query command"),
        }
    }

    #[test]
    fn test_cli_parser_dream() {
        let cli = Cli::try_parse_from(["physis", "dream"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Dream { count } => assert_eq!(count, 5),
            _ => panic!("expected Dream command"),
        }
    }

    #[test]
    fn test_cli_parser_evaluate() {
        let cli = Cli::try_parse_from(["physis", "evaluate", "dream-1", "0.85"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Evaluate { id, grade } => {
                assert_eq!(id, "dream-1");
                assert!((grade - 0.85).abs() < 1e-6);
            }
            _ => panic!("expected Evaluate command"),
        }
    }

    #[test]
    fn test_cli_parser_stats() {
        let cli = Cli::try_parse_from(["physis", "stats"]);
        assert!(cli.is_ok());
        assert!(matches!(cli.unwrap().command, Commands::Stats));
    }

    #[test]
    fn test_cli_parser_config() {
        let cli = Cli::try_parse_from(["physis", "config"]);
        assert!(cli.is_ok());
        assert!(matches!(cli.unwrap().command, Commands::Config));
    }

    #[test]
    fn test_cli_parser_watch() {
        let cli = Cli::try_parse_from(["physis", "watch", "/dir1", "/dir2"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Watch { dirs } => assert_eq!(dirs.len(), 2),
            _ => panic!("expected Watch command"),
        }
    }

    #[test]
    fn test_cli_parser_unknown_command_fails() {
        let cli = Cli::try_parse_from(["physis", "bogus"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_parser_missing_arg_fails() {
        let cli = Cli::try_parse_from(["physis", "scan"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_command_factory_usage() {
        let cli = Cli::command();
        let name = cli.get_name();
        assert_eq!(name, "physis");
    }

    #[test]
    fn test_app_new_with_default_config() {
        let config = PhysisConfig::default();
        let app = PhysisApp::new(config);
        assert!(app.goals.is_empty());
        assert!(app.actor.stats(&[]).total_actions == 0);
    }

    #[test]
    fn test_run_stats_empty() {
        let config = PhysisConfig::default();
        let app = PhysisApp::new(config);
        let stats = app.run_stats();
        assert!(stats.contains("Physis Stats"));
        assert!(stats.contains("total_actions: 0"));
    }

    #[test]
    fn test_run_query_empty() {
        let config = PhysisConfig::default();
        let app = PhysisApp::new(config);
        let results = app.run_query("nonexistent", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_run_dream_empty_goals() {
        let config = PhysisConfig::default();
        let mut app = PhysisApp::new(config);
        let dreams = app.run_dream(3);
        assert_eq!(dreams.len(), 0, "no goals means no dreams");
    }

    #[test]
    fn test_run_evaluate_nonexistent() {
        let config = PhysisConfig::default();
        let mut app = PhysisApp::new(config);
        let result = app.run_evaluate("bogus", 0.5);
        assert!(!result);
    }
}
