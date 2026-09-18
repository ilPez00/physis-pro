//! Configuration loading — builds PhysisConfig and loads ontologies from
//! built-in JSON, file paths, or raw strings into domain maps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::models::*;
use crate::ontology::OFFICE_ONTOLOGY_NAME;
use crate::ontology::*;
use crate::ontology::{SPACE_ONTOLOGY_NAME, TIME_ONTOLOGY_NAME};
use crate::ontology_nonhuman::MACHINE_ONTOLOGY_NAME;

/// The named built-in ontologies, resolvable by `OntologyLoader::load_builtin`.
/// To add one: drop a JSON file in `config/`, register its name constant, add a
/// row here and (if it needs its own domain map) a `kind` arm in `load_all`.
/// The named built-in ontologies, resolvable by `OntologyLoader::load_builtin`.
///
/// The JSON no longer lives here. `physis-core` owns the ontology corpus as
/// `physis_core::ontology::ONTOLOGY_SOURCES`, and this list selects from it by
/// canonical name. Pro used to `include_str!` a byte-identical second copy of
/// 33 files out of its own `config/`; three of them had silently drifted
/// (health_fitness, machine and software_dev were missing the `interpretant`
/// block Pro's own loader reads).
///
/// It has to cross as data rather than as a path: `include_str!` reaching into
/// `../physis-core/config/` compiles in this working tree and breaks the
/// published crate, where `physis-core` resolves from the registry and the
/// submodule directory is not there.
///
/// To add one: add the file and a row to `ONTOLOGY_SOURCES` in physis-core,
/// then add its name here and (if it needs its own domain map) a `kind` arm in
/// `load_all`.
static BUILTIN_ONTOLOGY_NAMES: &[&str] = &[
    HUMAN_ONTOLOGY_NAME,
    MACHINE_ONTOLOGY_NAME,
    SEMIOTIC_ONTOLOGY_NAME,
    CATEGORY_ONTOLOGY_NAME,
    AGENT_ONTOLOGY_NAME,
    NATURAL_ONTOLOGY_NAME,
    SOCIAL_ONTOLOGY_NAME,
    ABSTRACT_ONTOLOGY_NAME,
    ENGINEERING_ONTOLOGY_NAME,
    OFFICE_ONTOLOGY_NAME,
    // Phase 10 datawork: the space and time filing axes. Deliberately NOT
    // classification themes — files are not *about* "Warehouse" — so they get
    // their own domain maps and never join the classifier's centroids.
    SPACE_ONTOLOGY_NAME,
    TIME_ONTOLOGY_NAME,
];

/// Resolve an ontology's JSON from the core registry, or fail loudly.
///
/// A miss means the name drifted apart from `ONTOLOGY_SOURCES`, which is a
/// build-configuration bug rather than anything a user did; `every_registered_
/// ontology_resolves` in the tests below turns it into a test failure instead
/// of a first-run panic.
fn source_or_panic(name: &str) -> &'static str {
    physis_core::ontology::ontology_source(name).unwrap_or_else(|| {
        panic!("ontology '{name}' is not in physis_core::ontology::ONTOLOGY_SOURCES")
    })
}

static BUILTIN_ONTOLOGIES: Lazy<Vec<(&'static str, &'static str)>> = Lazy::new(|| {
    BUILTIN_ONTOLOGY_NAMES
        .iter()
        .map(|n| (*n, source_or_panic(n)))
        .collect()
});

/// Extra domain-specific ontologies, always loaded into `custom_domains`.
///
/// Rows are `(kind, registry name)`: the kind is Pro's key prefix and is NOT
/// always the file's name — `italian` reads `italian_industrial_ontology.json`
/// — so the mapping is explicit rather than derived.
///
/// To add a new ontology: add the file and a row to `ONTOLOGY_SOURCES` in
/// physis-core, then add one line here.
static EXTRA_ONTOLOGY_NAMES: &[(&str, &str)] = &[
    ("software_dev", "software_dev"),
    ("devops", "devops"),
    ("data_science", "data_science"),
    ("cybersecurity", "cybersecurity"),
    ("health_fitness", "health_fitness"),
    ("nutrition_cooking", "nutrition_cooking"),
    ("personal_finance", "personal_finance"),
    ("learning_education", "learning_education"),
    ("creative_writing", "creative_writing"),
    ("music", "music"),
    ("visual_arts", "visual_arts"),
    ("relationships", "relationships"),
    ("productivity", "productivity"),
    ("travel", "travel"),
    ("home_maintenance", "home_maintenance"),
    ("mindfulness_spirituality", "mindfulness_spirituality"),
    ("science_research", "science_research"),
    ("business_entrepreneurship", "business_entrepreneurship"),
    ("language_learning", "language_learning"),
    ("outdoors_sports", "outdoors_sports"),
    ("grid_fill", "grid_fill"),
    ("mode_anchors", "mode_anchors"),
    ("italian", "italian_industrial"),
];

static EXTRA_ONTOLOGIES: Lazy<Vec<(&'static str, &'static str)>> = Lazy::new(|| {
    EXTRA_ONTOLOGY_NAMES
        .iter()
        .map(|(kind, src)| (*kind, source_or_panic(src)))
        .collect()
});

static DEFAULT_PHYSIS_DIR: &str = ".physis";

/// Where one effective setting came from. Surfaced by `physis-pro config`,
/// `doctor` and `/api/v1/settings` so "which input set this value" is
/// answered from the record, not guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    Builtin,
    File,
    Env,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Origin::Builtin => write!(f, "builtin"),
            Origin::File => write!(f, "file"),
            Origin::Env => write!(f, "env"),
        }
    }
}

/// Effective settings: the merged config plus per-field provenance.
///
/// Precedence: built-in defaults < `<data_dir>/config.json` < environment.
/// A file that is not valid JSON, names an unknown field, or sets a value
/// outside its bounds is a hard error — silently running on defaults while a
/// config file sits on disk is the failure this exists to prevent.
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub config: PhysisConfig,
    pub origins: HashMap<String, Origin>,
}

const KNOWN_TOP_LEVEL: &[&str] = &[
    "data_dir",
    "ontologies",
    "network_scan_interval_secs",
    "dream_batch_size",
    "pdca_stagnant_threshold",
    "pdca_stagnant_window",
    "watch_dirs",
    "linguistic",
    "onnx",
    "embed_dim",
    "embedders",
    "agent",
];

fn resolve_data_dir_with(get_env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    get_env("PHYSIS_DATA_DIR")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            cwd.join(DEFAULT_PHYSIS_DIR)
        })
}

fn parse_bool_env(var: &str, raw: &str) -> anyhow::Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("{var}: want a bool (1/0/true/false), got {raw:?}"),
    }
}

/// Controls which linguistic lenses (Wenyan, Piraha, Sanskrit) are active at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinguisticConfig {
    /// Apply Wenyan kanji-heavy compression.
    pub wenyan_enabled: bool,
    /// Apply Piraha filler-stripping minimalism.
    pub piraha_enabled: bool,
    /// Apply Sanskrit poetic expansion.
    pub sanskrit_enabled: bool,
}

impl Default for LinguisticConfig {
    fn default() -> Self {
        Self {
            wenyan_enabled: true,
            piraha_enabled: true,
            sanskrit_enabled: true,
        }
    }
}

/// Business backoffice configuration — wraps PhysisConfig with business-specific settings
/// and defaults with Sanskrit dreaming + Wenyan compression disabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysisProConfig {
    /// Base Physis engine configuration (linguistic defaults overridden below).
    pub physis: PhysisConfig,
    /// Directories to watch for document ingestion (invoices, emails, spreadsheets).
    pub ingest_dirs: Vec<PathBuf>,
    /// llmwiki directory path for semantic search fallback.
    pub wiki_path: Option<PathBuf>,
    /// Industrial machine endpoints (MQTT broker URLs, Modbus TCP addresses).
    pub industrial_endpoints: Vec<String>,
    /// Path to sled inventory database (default: ~/.physis/inventory).
    pub inventory_db: PathBuf,
    /// Inventory ordering: "manual" (human-in-loop) or "auto".
    pub order_mode: String,
    /// Cooldown hours between duplicate order suggestions.
    pub order_cooldown_hours: u64,
}

impl Default for PhysisProConfig {
    fn default() -> Self {
        let data_dir = PhysisConfig::default().data_dir;
        let mut physis = PhysisConfig::default();
        // Disable Sanskrit dreaming and Wenyan compression by default for business use
        physis.linguistic.wenyan_enabled = false;
        physis.linguistic.sanskrit_enabled = false;
        Self {
            physis,
            ingest_dirs: vec![],
            wiki_path: Some(data_dir.join("wiki")),
            industrial_endpoints: vec![],
            inventory_db: data_dir.join("inventory"),
            order_mode: "manual".to_string(),
            order_cooldown_hours: 24,
        }
    }
}

/// Runtime parameters for the ONNX MiniLM embedder (used behind `embed-onnx` feature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnnxConfig {
    /// Whether to attempt ONNX embedding at runtime. Falls back to RP when unavailable.
    pub enabled: bool,
    /// Directory containing model.onnx and tokenizer.json. Uses `./models` when None.
    pub model_dir: Option<String>,
    /// Embedding vector dimension (must match the ONNX model output).
    pub dim: usize,
    /// Maximum sequence length for tokenization.
    pub max_length: usize,
    /// How to reduce the token matrix to one sentence vector. BERT-family
    /// sentence encoders (MiniLM, BGE-base) are trained for masked mean
    /// pooling; BGE-M3's dense vector is the CLS (first) token — mean pooling
    /// scrambles it (measured: CORE 0.923 vs 1.000 on the fixture set).
    #[serde(default)]
    pub pooling: PoolingStrategy,
}

/// Sentence-pooling strategy for ONNX encoder outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PoolingStrategy {
    /// Masked mean over real tokens (BERT-family sentence encoders).
    #[default]
    #[serde(rename = "mean")]
    Mean,
    /// First-token (CLS) vector (BGE-M3 dense).
    #[serde(rename = "cls")]
    Cls,
}

/// Which embedder kind to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbedderKindConfig {
    #[serde(rename = "random-projection")]
    RandomProjection,
    #[serde(rename = "minilm")]
    MiniLM,
    #[serde(rename = "clip")]
    Clip,
    #[serde(rename = "jina-v2")]
    JinaV2,
    /// BGE-base-en-v1.5 (768-d text). Strong, ONNX-friendly fallback that loads
    /// cleanly under the current `ort` when Jina's quant won't. Text-only.
    #[serde(rename = "bge-base")]
    BgeBase,
    /// BGE-M3 (1024-d text, XLM-R based, 100+ languages). Preferred over
    /// BGE-base when present — Italian/multilingual industrial input is the
    /// reason it exists here. Text-only.
    #[serde(rename = "bge-m3")]
    BgeM3,
    /// Any sentence-transformers model exported to ONNX
    /// (`optimum-cli export onnx -m sentence-transformers/<model> models/<dir>/`).
    /// Dimension and pooling are resolved from the directory name — see
    /// [`st_layout`]. Text-only; runs through the same `OnnxEmbedder` engine
    /// and self-test gate as the BGE kinds.
    #[serde(rename = "sentence-transformers")]
    SentenceTransformers,
}

/// Resolve dimension and pooling for a sentence-transformers export from its
/// model directory name. Known ST exports are mean-pooled; the map below is
/// the single place to extend when a new layout is adopted.
pub fn st_layout(model_dir: &str) -> (usize, PoolingStrategy) {
    let name = model_dir
        .rsplit('/')
        .next()
        .unwrap_or(model_dir)
        .to_ascii_lowercase();
    if name.contains("mpnet") || name.contains("bge") {
        (768, PoolingStrategy::Mean)
    } else if name.contains("distiluse") || name.contains("distilbert") {
        (512, PoolingStrategy::Mean)
    } else {
        // all-MiniLM*, paraphrase-multilingual-MiniLM*, and any unlisted
        // transformer-style export default to 384-d mean pooling.
        (384, PoolingStrategy::Mean)
    }
}

/// Configuration for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderConfig {
    pub kind: EmbedderKindConfig,
    /// Directory containing model files.
    pub model_dir: String,
    /// Whether this embedder is enabled.
    #[serde(default = "return_true")]
    pub enabled: bool,
}

fn return_true() -> bool {
    true
}

/// Multi-embedder configuration block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmbeddersConfig {
    /// Primary embedder for text (and optionally image).
    pub primary: Option<EmbedderConfig>,
    /// Optional image-only embedder (if primary doesn't handle images).
    pub image: Option<EmbedderConfig>,
}

impl Default for OnnxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model_dir: Some("./models".to_string()),
            dim: 384,
            max_length: 128,
            pooling: PoolingStrategy::Mean,
        }
    }
}

/// Top-level configuration for a Physis engine instance.
///
/// Controls data directories, ontology sources, network scanning, dream batching,
/// PDCA stagnation detection parameters, linguistic lenses, and embedder settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysisConfig {
    /// Directory for persistent data (hash cache, logs, etc.). Discovered
    /// (PHYSIS_DATA_DIR or cwd/.physis), never set in the file — see
    /// `load_effective`.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Ontology sources to load at startup.
    #[serde(default = "default_ontologies")]
    pub ontologies: Vec<OntologySource>,
    /// Seconds between network scans when watching directories.
    #[serde(default = "default_scan_interval")]
    pub network_scan_interval_secs: u64,
    /// Number of dreams to generate per dream cycle.
    #[serde(default = "default_dream_batch")]
    pub dream_batch_size: usize,
    /// Progress threshold below which a goal is flagged stagnant.
    #[serde(default = "default_pdca_threshold")]
    pub pdca_stagnant_threshold: f32,
    /// Number of recent PDCA actions to inspect for stagnation.
    #[serde(default = "default_pdca_window")]
    pub pdca_stagnant_window: usize,
    /// Directories to watch for file changes.
    #[serde(default)]
    pub watch_dirs: Vec<PathBuf>,
    /// Linguistic lense toggles (Wenyan, Piraha, Sanskrit).
    #[serde(default)]
    pub linguistic: LinguisticConfig,
    /// ONNX embedder tuning parameters.
    #[serde(default)]
    pub onnx: OnnxConfig,
    /// Embedding vector dimension (should match model).
    #[serde(default = "default_embed_dim")]
    pub embed_dim: usize,
    /// Multi-embedder configuration (optional, overrides onnx fallback).
    #[serde(default)]
    pub embedders: EmbeddersConfig,
    /// Agent-orchestration configuration (ReAct loop, OpenCode coding sub-agent,
    /// tool palette). Drives the /api/v1/agent/run endpoint + /agent dashboard.
    #[serde(default)]
    pub agent: AgentRuntimeConfig,
}

fn default_data_dir() -> PathBuf {
    resolve_data_dir_with(&|k| std::env::var(k).ok())
}
fn default_ontologies() -> Vec<OntologySource> {
    defaults_for(default_data_dir(), false).ontologies
}
fn default_scan_interval() -> u64 {
    60
}
fn default_dream_batch() -> usize {
    5
}
fn default_pdca_threshold() -> f32 {
    0.2
}
fn default_pdca_window() -> usize {
    5
}
fn default_embed_dim() -> usize {
    384
}

/// Configuration for the in-process agent orchestration surface.
///
/// Physis is the host/orchestrator; the LLM brain is the provider cascade and
/// the "hands" are tools. One of those tools is OpenCode running as a sandboxed
/// subprocess — a coding sub-agent — so Physis can plan, then delegate actual
/// file edits/builds to a real coding engine. See `src/web/agent.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeConfig {
    /// Absolute path to the OpenCode binary. Not on PATH by default in many
    /// setups, so it is configured explicitly. Empty/`None` disables the
    /// `opencode` tool (the agent still runs with the other native tools).
    #[serde(default)]
    pub opencode_bin: Option<PathBuf>,
    /// Model passed to `opencode run` (e.g. "llm7/codestral-latest").
    #[serde(default = "default_opencode_model")]
    pub opencode_model: String,
    /// Maximum tool-call rounds per agent run (safety rail).
    #[serde(default = "default_agent_max_rounds")]
    pub max_rounds: u32,
    /// Per-response token cap for the orchestrating LLM.
    #[serde(default = "default_agent_max_tokens")]
    pub max_tokens: u32,
    /// System prompt for the orchestrator agent.
    #[serde(default = "default_agent_system_prompt")]
    pub system_prompt: String,
    /// Run every agent answer through the coherence harness before it is
    /// presented, so the arbiter that judges every other claim in Physis also
    /// judges the agent's. Defaults on: an agent that emits ungated claims into
    /// a system built to retain only interpretations that keep working is the
    /// one component exempt from the thesis.
    ///
    /// The gate never rewrites or withholds the answer — it attaches a verdict.
    /// Suppressing output would make a coherence miss indistinguishable from a
    /// crashed run, and the arbiter is a judgement, not an oracle.
    #[serde(default = "default_true")]
    pub harness_gate: bool,
    /// Composite-fitness floor for an agent answer to be reported APPROVED.
    /// Separate from the standalone harness default because an agent answer is
    /// synthesised from tool output already grounded in the graph, so it is
    /// held to the same bar rather than a looser one.
    #[serde(default = "default_agent_gate_threshold")]
    pub harness_threshold: f32,
}

fn default_opencode_model() -> String {
    "llm7/codestral-latest".into()
}
fn default_agent_max_rounds() -> u32 {
    12
}
fn default_agent_max_tokens() -> u32 {
    1536
}
fn default_true() -> bool {
    true
}
fn default_agent_gate_threshold() -> f32 {
    0.60
}
fn default_agent_system_prompt() -> String {
    "You are Physis, an industrial backoffice intelligence agent. You have tools to \
search and edit the knowledge graph, classify text onto the semiotic grid, run a \
coherence/dream cycle, and delegate coding tasks to OpenCode. Use the knowledge \
graph and retrieval tools to ground your reasoning, and use OpenCode only when a \
task requires writing or modifying real files. Think step by step, then act."
        .into()
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            opencode_bin: None,
            opencode_model: default_opencode_model(),
            max_rounds: default_agent_max_rounds(),
            max_tokens: default_agent_max_tokens(),
            system_prompt: default_agent_system_prompt(),
            harness_gate: default_true(),
            harness_threshold: default_agent_gate_threshold(),
        }
    }
}

/// Describes a single ontology source: built-in (no path) or file-based.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologySource {
    /// Display name for this ontology.
    #[serde(default = "default_human_ontology")]
    pub name: String,
    /// Filesystem path (None for built-in ontologies).
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// Ontology kind: "human", "machine", or custom.
    #[serde(default)]
    pub kind: String,
    /// Whether this source is loaded at startup.
    #[serde(default = "default_true_src")]
    pub enabled: bool,
}

fn default_human_ontology() -> String {
    HUMAN_ONTOLOGY_NAME.to_string()
}
fn default_true_src() -> bool {
    true
}

impl Default for PhysisConfig {
    fn default() -> Self {
        Self::default_at(resolve_data_dir_with(&|k| std::env::var(k).ok()))
    }
}

impl PhysisConfig {
    fn default_at(data_dir: PathBuf) -> Self {
        defaults_for(data_dir, true)
    }

    fn default_no_env(data_dir: PathBuf) -> Self {
        defaults_for(data_dir, false)
    }
}

/// Shared builtin constructor. `apply_env=true` layers the
/// PHYSIS_* overrides (warn-and-keep-builtin on a bad value so Default never
/// panics); `load_effective` passes false and applies env itself as hard
/// errors after the file layer.
fn defaults_for(data_dir: PathBuf, apply_env: bool) -> PhysisConfig {
    let mut cfg = PhysisConfig {
        data_dir,
        ontologies: vec![
            OntologySource {
                name: HUMAN_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "human".to_string(),
                enabled: true,
            },
            OntologySource {
                name: MACHINE_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "machine".to_string(),
                enabled: true,
            },
            OntologySource {
                name: SEMIOTIC_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "semiotic".to_string(),
                enabled: true,
            },
            OntologySource {
                name: CATEGORY_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "category".to_string(),
                enabled: true,
            },
            OntologySource {
                name: AGENT_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "agent".to_string(),
                enabled: true,
            },
            OntologySource {
                name: NATURAL_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "natural".to_string(),
                enabled: true,
            },
            OntologySource {
                name: SOCIAL_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "social".to_string(),
                enabled: true,
            },
            OntologySource {
                name: ABSTRACT_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "abstract".to_string(),
                enabled: true,
            },
            OntologySource {
                name: ENGINEERING_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "engineering".to_string(),
                enabled: true,
            },
            OntologySource {
                name: OFFICE_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "office".to_string(),
                enabled: true,
            },
            OntologySource {
                name: SPACE_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "place".to_string(),
                enabled: true,
            },
            OntologySource {
                name: TIME_ONTOLOGY_NAME.to_string(),
                path: None,
                kind: "period".to_string(),
                enabled: true,
            },
        ],
        network_scan_interval_secs: 60,
        dream_batch_size: 5,
        pdca_stagnant_threshold: 0.2,
        pdca_stagnant_window: 5,
        watch_dirs: vec![],
        linguistic: LinguisticConfig::default(),
        onnx: OnnxConfig::default(),
        embed_dim: 384,
        embedders: EmbeddersConfig::default(),
        agent: AgentRuntimeConfig::default(),
    };
    // Env wins over built-ins for the same knobs the Studio/TUI CONFIG
    // tab edits. A bad value warns and keeps the builtin here so Default
    // never panics; `load_effective` applies env itself as hard errors.
    if apply_env {
        if let Err(e) = cfg.apply_env(&|k| std::env::var(k).ok()) {
            eprintln!("[physis] WARNING: {e} — keeping builtin default");
        }
    }
    cfg
}

impl PhysisConfig {
    /// Load the effective config: builtins < `<data_dir>/config.json` < env.
    ///
    /// Data-dir discovery: `PHYSIS_DATA_DIR` wins, else `cwd/.physis`. The
    /// file is read at the discovered dir; a `data_dir` inside the file moves
    /// nothing (self-reference rejected as an error) so every layer agrees on
    /// where state lives.
    pub fn load_effective() -> anyhow::Result<EffectiveConfig> {
        Self::load_effective_with(&|k| std::env::var(k).ok())
    }

    fn load_effective_with(
        get_env: &dyn Fn(&str) -> Option<String>,
    ) -> anyhow::Result<EffectiveConfig> {
        let data_dir = resolve_data_dir_with(get_env);
        let builtin = Self::default_no_env(data_dir.clone());
        let mut origins: HashMap<String, Origin> = builtin
            .field_names()
            .into_iter()
            .map(|n| (n.to_string(), Origin::Builtin))
            .collect();
        let mut cfg = builtin;
        let mut file_keys: Vec<String> = Vec::new();
        let path = data_dir.join("config.json");
        if let Some(raw) = std::fs::read_to_string(&path).ok().filter(|s| !s.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| anyhow::anyhow!("{}: invalid JSON: {e}", path.display()))?;
            let obj = v
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("{}: top level must be an object", path.display()))?;
            let keys: Vec<String> = obj.keys().cloned().collect();
            for k in &keys {
                if !KNOWN_TOP_LEVEL.contains(&k.as_str()) {
                    anyhow::bail!("{}: unknown field `{k}`", path.display());
                }
            }
            if obj.contains_key("data_dir") {
                anyhow::bail!(
                    "{}: `data_dir` is discovered (PHYSIS_DATA_DIR or cwd/.physis), not configured",
                    path.display()
                );
            }
            let merged: PhysisConfig = serde_json::from_str(&raw)
                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
            for k in keys {
                let file_val = serde_json::to_value(&merged).ok().and_then(|c| c.get(&k).cloned());
                let base_val = serde_json::to_value(&cfg).ok().and_then(|c| c.get(&k).cloned());
                if file_val != base_val {
                    file_keys.push(k);
                }
            }
            cfg = merged;
            cfg.data_dir = data_dir.clone();
        }
        cfg.validate_bounds(&path)?;
        for k in file_keys {
            origins.insert(k, Origin::File);
        }
        let env_keys = cfg.apply_env(get_env)?;
        for k in env_keys {
            origins.insert(k, Origin::Env);
        }
        cfg.data_dir = data_dir;
        cfg.validate_bounds(&path)?;
        Ok(EffectiveConfig {
            config: cfg,
            origins,
        })
    }

    fn field_names(&self) -> Vec<&'static str> {
        KNOWN_TOP_LEVEL.to_vec()
    }

    /// Bounds that make a broken config fail loudly instead of running wrong.
    /// `label` names the source in the error (file path or "env").
    fn validate_bounds(&self, label: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.network_scan_interval_secs >= 1,
            "{}: network_scan_interval_secs must be >= 1, got {}",
            label.display(),
            self.network_scan_interval_secs
        );
        anyhow::ensure!(
            self.dream_batch_size >= 1,
            "{}: dream_batch_size must be >= 1, got {}",
            label.display(),
            self.dream_batch_size
        );
        anyhow::ensure!(
            (0.0..=1.0).contains(&self.pdca_stagnant_threshold),
            "{}: pdca_stagnant_threshold must be in [0,1], got {}",
            label.display(),
            self.pdca_stagnant_threshold
        );
        anyhow::ensure!(
            self.pdca_stagnant_window >= 1,
            "{}: pdca_stagnant_window must be >= 1, got {}",
            label.display(),
            self.pdca_stagnant_window
        );
        anyhow::ensure!(
            self.embed_dim >= 64,
            "{}: embed_dim must be >= 64, got {}",
            label.display(),
            self.embed_dim
        );
        Ok(())
    }

    /// Layer env overrides onto self. Returns top-level keys the env touched.
    /// Same knobs the Studio/TUI CONFIG tab edits, plus embedder identity
    /// (PHYSIS_EMBEDDER) which `select_embedder` already honors.
    fn apply_env(
        &mut self,
        get_env: &dyn Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Vec<String>> {
        let mut touched = Vec::new();
        let mut get = |var: &str, key: &str, set: &mut dyn FnMut(String) -> anyhow::Result<()>| -> anyhow::Result<()> {
            if let Some(raw) = get_env(var).filter(|s| !s.trim().is_empty()) {
                set(raw).map_err(|e| anyhow::anyhow!("env {var}: {e}"))?;
                if !touched.contains(&key.to_string()) {
                    touched.push(key.to_string());
                }
            }
            Ok(())
        };
        get("PHYSIS_SCAN_INTERVAL", "network_scan_interval_secs", &mut |raw| {
            self.network_scan_interval_secs = raw
                .trim()
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("want an integer >= 1, got {raw:?}"))?;
            Ok(())
        })?;
        get("PHYSIS_DREAM_BATCH", "dream_batch_size", &mut |raw| {
            self.dream_batch_size = raw
                .trim()
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("want an integer >= 1, got {raw:?}"))?;
            Ok(())
        })?;
        get("PHYSIS_PDCA_THRESHOLD", "pdca_stagnant_threshold", &mut |raw| {
            self.pdca_stagnant_threshold = raw
                .trim()
                .parse::<f32>()
                .map_err(|_| anyhow::anyhow!("want a float in [0,1], got {raw:?}"))?;
            Ok(())
        })?;
        get("PHYSIS_PDCA_WINDOW", "pdca_stagnant_window", &mut |raw| {
            self.pdca_stagnant_window = raw
                .trim()
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("want an integer >= 1, got {raw:?}"))?;
            Ok(())
        })?;
        get("PHYSIS_EMBED_DIM", "embed_dim", &mut |raw| {
            self.embed_dim = raw
                .trim()
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("want an integer >= 64, got {raw:?}"))?;
            Ok(())
        })?;
        get("PHYSIS_WENYAN", "linguistic", &mut |raw| {
            self.linguistic.wenyan_enabled = parse_bool_env("PHYSIS_WENYAN", &raw)?;
            Ok(())
        })?;
        get("PHYSIS_SANSKRIT", "linguistic", &mut |raw| {
            self.linguistic.sanskrit_enabled = parse_bool_env("PHYSIS_SANSKRIT", &raw)?;
            Ok(())
        })?;
        Ok(touched)
    }
}

/// Loads and resolves ontologies from built-in JSON, file paths, or raw strings.
///
/// Maintains three separate domain maps (human, machine, custom) populated
/// by `load_all` based on `PhysisConfig.ontologies`.
#[derive(Debug, Clone)]
pub struct OntologyLoader {
    /// Human domain definitions (praxis ontology).
    pub human_domains: HashMap<String, DomainDef>,
    /// Machine domain definitions (machine ontology).
    pub machine_domains: HashMap<String, DomainDef>,
    /// Semiotic domain definitions.
    pub semiotic_domains: HashMap<String, DomainDef>,
    /// Category theory domain definitions.
    pub category_domains: HashMap<String, DomainDef>,
    /// AI agent domain definitions.
    pub agent_domains: HashMap<String, DomainDef>,
    /// Natural world domain definitions.
    pub natural_domains: HashMap<String, DomainDef>,
    /// Social domain definitions.
    pub social_domains: HashMap<String, DomainDef>,
    /// Abstract domain definitions.
    pub abstract_domains: HashMap<String, DomainDef>,
    /// Engineering domain definitions.
    pub engineering_domains: HashMap<String, DomainDef>,
    /// Office/business domain definitions.
    pub office_domains: HashMap<String, DomainDef>,
    /// Place definitions (space axis, Phase 10 datawork). Not classification
    /// themes: they name *where* things live.
    pub space_domains: HashMap<String, DomainDef>,
    /// Period definitions (time axis, Phase 10 datawork): shifts, sprints,
    /// quarters — the rhythm vocabulary addresses bucket into.
    pub time_domains: HashMap<String, DomainDef>,
    /// Custom domain definitions loaded from file paths.
    pub custom_domains: HashMap<String, DomainDef>,
    /// Per-ontology interpretant, keyed by the source's `kind`.
    ///
    /// Kept beside the domain maps rather than copied onto every entry: the
    /// interpretant is a property of the universe of discourse, not of the
    /// individual sign. Sources with no declaration are absent here and fall
    /// back to [`Interpretant::default`].
    pub interpretants: HashMap<String, crate::models::Interpretant>,
}

impl OntologyLoader {
    /// Create an empty ontology loader.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            human_domains: HashMap::new(),
            machine_domains: HashMap::new(),
            semiotic_domains: HashMap::new(),
            category_domains: HashMap::new(),
            agent_domains: HashMap::new(),
            natural_domains: HashMap::new(),
            social_domains: HashMap::new(),
            abstract_domains: HashMap::new(),
            engineering_domains: HashMap::new(),
            office_domains: HashMap::new(),
            space_domains: HashMap::new(),
            time_domains: HashMap::new(),
            custom_domains: HashMap::new(),
            interpretants: HashMap::new(),
        }
    }

    /// The interpretant governing an ontology kind, or the default when the
    /// source declares none.
    pub fn interpretant(&self, kind: &str) -> crate::models::Interpretant {
        self.interpretants.get(kind).cloned().unwrap_or_default()
    }

    /// Load a built-in ontology by its registered name (see `BUILTIN_ONTOLOGIES`).
    /// Returns `None` for names that aren't built in.
    pub fn load_builtin(name: &str) -> Option<HashMap<String, DomainDef>> {
        let (_, json) = BUILTIN_ONTOLOGIES.iter().find(|(n, _)| *n == name)?;
        let entries: OntologyConfig = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("Built-in ontology '{name}' is invalid JSON: {e}"));
        Some(Self::entries_to_map(&entries.domains))
    }

    /// Load domain definitions from a JSON file on disk.
    pub fn load_from_path(path: &Path) -> anyhow::Result<HashMap<String, DomainDef>> {
        Ok(Self::load_file(path)?.1)
    }

    /// Load an ontology file, returning its declared interpretant alongside its
    /// entries. An invalid interpretant is reported and replaced with the
    /// default rather than failing the load: a bad feedback rule should not
    /// cost the user the 900 entries in the same file.
    pub fn load_file(
        path: &Path,
    ) -> anyhow::Result<(
        Option<crate::models::Interpretant>,
        HashMap<String, DomainDef>,
    )> {
        let contents = std::fs::read_to_string(path)?;
        Self::load_str(&contents, &path.display().to_string())
    }

    /// Parse domain definitions from a raw JSON string.
    pub fn load_from_str(json: &str) -> anyhow::Result<HashMap<String, DomainDef>> {
        Ok(Self::load_str(json, "<embedded>")?.1)
    }

    /// Parse an ontology from JSON, returning its interpretant with its
    /// entries. `label` only names the source in warnings.
    pub fn load_str(
        json: &str,
        label: &str,
    ) -> anyhow::Result<(
        Option<crate::models::Interpretant>,
        HashMap<String, DomainDef>,
    )> {
        let config: OntologyConfig = serde_json::from_str(json)?;
        let interpretant = config.interpretant.and_then(|i| match i.validate() {
            Ok(()) => Some(i),
            Err(e) => {
                eprintln!(
                    "Warning: ontology '{label}' declares an invalid interpretant ({e}); falling back to the default"
                );
                None
            }
        });
        Ok((interpretant, Self::entries_to_map(&config.domains)))
    }

    /// Load a built-in ontology with its interpretant.
    pub fn load_builtin_full(
        name: &str,
    ) -> Option<(
        Option<crate::models::Interpretant>,
        HashMap<String, DomainDef>,
    )> {
        let (_, json) = BUILTIN_ONTOLOGIES.iter().find(|(n, _)| *n == name)?;
        Self::load_str(json, name).ok()
    }

    fn entries_to_map(entries: &[OntologyEntry]) -> HashMap<String, DomainDef> {
        let mut map = HashMap::new();
        for e in entries {
            map.insert(
                e.name.clone(),
                DomainDef {
                    name: e.name.clone(),
                    category: e.category.clone(),
                    domain: Some(e.domain.clone()),
                    mode: Some(e.mode.clone()),
                    axis_kind: Some(e.axis_kind.clone()),
                    axis_name: Some(e.axis_name.clone()),
                    unit: e.unit.clone(),
                    hints: e.hints.clone(),
                    facets: e.facets.clone(),
                },
            );
        }
        map
    }

    /// Load all enabled ontologies from a `PhysisConfig`, populating human, machine,
    /// and custom domain maps. Built-in ontologies are loaded when no path is given.
    pub fn load_all(config: &PhysisConfig) -> Self {
        let mut loader = Self::new();

        for source in &config.ontologies {
            if !source.enabled {
                continue;
            }
            let map = if let Some(ref path) = source.path {
                if path.exists() {
                    match Self::load_file(path) {
                        Ok((interpretant, map)) => {
                            // Keyed by kind, so several files sharing a kind
                            // agree on how their signs are judged. Last one
                            // wins, matching how the domain maps merge.
                            if let Some(i) = interpretant {
                                loader.interpretants.insert(source.kind.clone(), i);
                            }
                            map
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: failed to load ontology '{}' from {}: {}",
                                source.name,
                                path.display(),
                                e
                            );
                            HashMap::new()
                        }
                    }
                } else {
                    eprintln!("Warning: ontology path {} not found", path.display());
                    HashMap::new()
                }
            } else {
                match Self::load_builtin_full(&source.name) {
                    Some((interpretant, map)) => {
                        if let Some(i) = interpretant {
                            loader.interpretants.insert(source.kind.clone(), i);
                        }
                        map
                    }
                    None => HashMap::new(),
                }
            };

            for (name, def) in map {
                match source.kind.as_str() {
                    "human" => {
                        loader.human_domains.insert(name, def);
                    }
                    "machine" => {
                        loader.machine_domains.insert(name, def);
                    }
                    "semiotic" => {
                        loader.semiotic_domains.insert(name, def);
                    }
                    "category" => {
                        loader.category_domains.insert(name, def);
                    }
                    "agent" => {
                        loader.agent_domains.insert(name, def);
                    }
                    "natural" => {
                        loader.natural_domains.insert(name, def);
                    }
                    "social" => {
                        loader.social_domains.insert(name, def);
                    }
                    "abstract" => {
                        loader.abstract_domains.insert(name, def);
                    }
                    "engineering" => {
                        loader.engineering_domains.insert(name, def);
                    }
                    "office" => {
                        loader.office_domains.insert(name, def);
                    }
                    "place" => {
                        loader.space_domains.insert(name, def);
                    }
                    "period" => {
                        loader.time_domains.insert(name, def);
                    }
                    _ => {
                        loader.custom_domains.insert(name, def);
                    }
                }
            }
        }

        // Always-on extra domain-specific ontologies → custom_domains.
        // Keyed by "kind:name" so identical entry names across files don't collide.
        for (kind, json) in EXTRA_ONTOLOGIES.iter() {
            match Self::load_str(json, kind) {
                Ok((interpretant, map)) => {
                    // Keyed by the extra ontology's own name, not by the
                    // `custom` bucket it lands in: all 20 share that bucket,
                    // but each judges its signs differently — that difference
                    // is the whole point of a per-ontology interpretant.
                    if let Some(i) = interpretant {
                        loader.interpretants.insert(kind.to_string(), i);
                    }
                    for (name, def) in map {
                        loader.custom_domains.insert(format!("{kind}:{name}"), def);
                    }
                }
                Err(e) => eprintln!("Warning: failed to parse built-in ontology '{kind}': {e}"),
            }
        }

        // Industry packs (Phase 10b T4, PLAN 4.2-lite): drop JSON ontology
        // files into `{data_dir}/packs/` and they join custom_domains the same
        // way as baked extras — keyed "kind:name", interpretant honored. No
        // code change, no rebuild: an industry pack is data, and packs a
        // customer hasn't installed must not silently classify their files.
        let packs_dir = config.data_dir.join("packs");
        if let Ok(entries) = std::fs::read_dir(&packs_dir) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
                .collect();
            paths.sort(); // deterministic order across machines
            for path in paths {
                match Self::load_file(&path) {
                    Ok((interpretant, map)) => {
                        // Pack identity = file stem, so interpretants and
                        // domains stay attributable to the pack that shipped them.
                        let kind = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("pack")
                            .to_string();
                        if let Some(i) = interpretant {
                            loader.interpretants.insert(format!("pack:{kind}"), i);
                        }
                        let n = map.len();
                        for (name, def) in map {
                            loader
                                .custom_domains
                                .insert(format!("{}:{name}", kind), def);
                        }
                        eprintln!(
                            "[physis] loaded ontology pack '{}' ({n} entries)",
                            path.display()
                        );
                    }
                    Err(e) => {
                        eprintln!("Warning: skipping broken pack {}: {e}", path.display())
                    }
                }
            }
        }

        loader
    }

    /// All domain definitions used for classification: the human (praxis) grid
    /// plus every domain ontology and pack. All share the grid vocabulary.
    ///
    /// Phase 10c fix — this used to chain only `human + custom`, which locked
    /// machine/agent/engineering/office/semiotic/category/natural/social/
    /// abstract entries out of the classifier entirely (~360 authored entries
    /// that could never be scored): industrial text fell back to human cells
    /// even when a perfect machine-ontology entry existed. Only `place`/
    /// `period` stay excluded — files are not *about* "Warehouse" (Phase 10).
    pub fn classification_domains(&self) -> impl Iterator<Item = &DomainDef> {
        self.human_domains
            .values()
            .chain(self.machine_domains.values())
            .chain(self.agent_domains.values())
            .chain(self.engineering_domains.values())
            .chain(self.semiotic_domains.values())
            .chain(self.category_domains.values())
            .chain(self.natural_domains.values())
            .chain(self.social_domains.values())
            .chain(self.abstract_domains.values())
            .chain(self.office_domains.values())
            .chain(self.custom_domains.values())
    }

    /// Every domain definition across all loaded ontologies, merged into one map.
    /// Seeds theme nodes for the file-graph view. On key collision the later map
    /// wins (custom overrides builtin).
    pub fn all_domains(&self) -> HashMap<String, DomainDef> {
        let mut out = HashMap::new();
        for m in [
            &self.human_domains,
            &self.machine_domains,
            &self.semiotic_domains,
            &self.category_domains,
            &self.agent_domains,
            &self.natural_domains,
            &self.social_domains,
            &self.abstract_domains,
            &self.engineering_domains,
            &self.office_domains,
            &self.custom_domains,
        ] {
            for (k, v) in m {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    /// Resolve a goal name to a human domain definition. Currently returns the first available.
    pub fn resolve_domain(&self, _goal_name: &str) -> Option<&DomainDef> {
        self.human_domains.values().next()
    }

    /// Format a goal as a human-readable string with domain context and progress.
    pub fn enrich_goal(&self, goal: &Goal) -> String {
        let _ = goal;
        let def = self.human_domains.values().next();
        match def {
            Some(d) => format!("[{}] progress={}%", d.unit, (0.0 * 100.0) as u32),
            None => format!("[VECTOR] progress={}%", (0.0 * 100.0) as u32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Precedence: builtins < file < env, with each layer's origin recorded.
    /// A file value differs from the builtin only when the file sets it; an
    /// env value always wins and is marked env.
    #[test]
    fn effective_config_records_origin_per_layer() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join(".physis");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::write(
            data_dir.join("config.json"),
            r#"{"dream_batch_size": 9, "embed_dim": 512}"#,
        )
        .unwrap();
        let get_env: &dyn Fn(&str) -> Option<String> = &|k: &str| match k {
            "PHYSIS_DATA_DIR" => Some(data_dir.display().to_string()),
            "PHYSIS_EMBED_DIM" => Some("768".into()),
            _ => None,
        };
        let eff = PhysisConfig::load_effective_with(get_env).unwrap();
        assert_eq!(eff.config.dream_batch_size, 9);
        assert_eq!(eff.config.embed_dim, 768);
        assert_eq!(eff.origins["dream_batch_size"], Origin::File);
        assert_eq!(eff.origins["embed_dim"], Origin::Env);
        assert_eq!(eff.origins["pdca_stagnant_window"], Origin::Builtin);
    }

    /// Env-only, no file: the same knobs apply without a config.json present.
    #[test]
    fn effective_config_env_without_file() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join(".physis");
        std::fs::create_dir_all(&data_dir).unwrap();
        let get_env: &dyn Fn(&str) -> Option<String> = &|k: &str| match k {
            "PHYSIS_DATA_DIR" => Some(data_dir.display().to_string()),
            "PHYSIS_PDCA_THRESHOLD" => Some("0.7".into()),
            _ => None,
        };
        let eff = PhysisConfig::load_effective_with(get_env).unwrap();
        assert!((eff.config.pdca_stagnant_threshold - 0.7).abs() < f32::EPSILON);
        assert_eq!(eff.origins["pdca_stagnant_threshold"], Origin::Env);
    }

    /// Invalid JSON, unknown fields, self-referential data_dir and
    /// out-of-bounds values are hard errors, not silent defaults.
    #[test]
    fn effective_config_rejects_broken_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join(".physis");
        std::fs::create_dir_all(&data_dir).unwrap();
        let no_env: &dyn Fn(&str) -> Option<String> = &|k: &str| {
            if k == "PHYSIS_DATA_DIR" {
                Some(data_dir.display().to_string())
            } else {
                None
            }
        };
        // Bad JSON.
        std::fs::write(data_dir.join("config.json"), "{not json").unwrap();
        let e = PhysisConfig::load_effective_with(no_env).unwrap_err();
        assert!(e.to_string().contains("invalid JSON"), "{e}");
        // Unknown field.
        std::fs::write(data_dir.join("config.json"), r#"{"nope": 1}"#).unwrap();
        let e = PhysisConfig::load_effective_with(no_env).unwrap_err();
        assert!(e.to_string().contains("unknown field"), "{e}");
        // data_dir inside the file moves nothing.
        std::fs::write(
            data_dir.join("config.json"),
            r#"{"data_dir": "/tmp/x"}"#,
        )
        .unwrap();
        let e = PhysisConfig::load_effective_with(no_env).unwrap_err();
        assert!(e.to_string().contains("data_dir"), "{e}");
        // Out of bounds.
        std::fs::write(data_dir.join("config.json"), r#"{"embed_dim": 8}"#).unwrap();
        let e = PhysisConfig::load_effective_with(no_env).unwrap_err();
        assert!(e.to_string().contains("embed_dim"), "{e}");
        // Bad env value is a hard error too (Default only warns).
        std::fs::remove_file(data_dir.join("config.json")).unwrap();
        let bad_env: &dyn Fn(&str) -> Option<String> = &|k: &str| match k {
            "PHYSIS_DATA_DIR" => Some(data_dir.display().to_string()),
            "PHYSIS_EMBED_DIM" => Some("abc".into()),
            _ => None,
        };
        let e = PhysisConfig::load_effective_with(bad_env).unwrap_err();
        assert!(e.to_string().contains("PHYSIS_EMBED_DIM"), "{e}");
    }

    #[test]
    fn test_builtin_human_ontology_count() {
        let map = OntologyLoader::load_builtin(HUMAN_ONTOLOGY_NAME).unwrap();
        assert!(
            map.len() >= 14,
            "expected at least 14 human domains, got {}",
            map.len()
        );
    }

    #[test]
    fn st_layout_resolves_known_export_names() {
        // Multilingual MiniLM: the Italian-gap candidate, 384-d mean.
        assert_eq!(
            st_layout("models/paraphrase-multilingual-MiniLM-L12-v2"),
            (384, PoolingStrategy::Mean)
        );
        assert_eq!(
            st_layout("models/all-MiniLM-L6-v2"),
            (384, PoolingStrategy::Mean)
        );
        // mpnet-family exports are 768-d.
        assert_eq!(
            st_layout("models/all-mpnet-base-v2"),
            (768, PoolingStrategy::Mean)
        );
        // DistilUSE is 512-d.
        assert_eq!(
            st_layout("models/distiluse-base-multilingual-cased-v2"),
            (512, PoolingStrategy::Mean)
        );
        // Unlisted transformer exports default to 384-d mean.
        assert_eq!(st_layout("models/some-new-export"), (384, PoolingStrategy::Mean));
    }

    #[test]
    fn st_kind_round_trips_through_config_json() {
        let json = r#"{"kind": "sentence-transformers", "model_dir": "models/all-MiniLM-L6-v2"}"#;
        let ec: EmbedderConfig = serde_json::from_str(json).expect("parse");
        assert!(matches!(ec.kind, EmbedderKindConfig::SentenceTransformers));
        let back = serde_json::to_string(&ec.kind).unwrap();
        assert_eq!(back, r#""sentence-transformers""#);
    }

    #[test]
    fn test_builtin_machine_ontology_count() {
        let map = OntologyLoader::load_builtin(MACHINE_ONTOLOGY_NAME).unwrap();
        assert!(
            map.len() >= 50,
            "expected at least 50 machine domains, got {}",
            map.len()
        );
    }

    #[test]
    fn test_domain_resolution_exact() {
        let map = OntologyLoader::load_builtin(HUMAN_ONTOLOGY_NAME).unwrap();
        let def = map.get("Body & Fitness");
        assert!(def.is_some(), "Body & Fitness should be found");
        if let Some(d) = def {
            assert!(
                d.hints.iter().any(|h| h == "exercise"),
                "expected 'exercise' hint"
            );
        }
    }

    #[test]
    fn test_load_from_str_valid() {
        let json = r#"{"kind":"human","domains":[{"name":"Test","category":null,"domain":"WORK","mode":"WORK","axis_kind":"human","axis_name":"physical","unit":"units","hints":["test"]}]}"#;
        let map = OntologyLoader::load_from_str(json).unwrap();
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_load_from_str_invalid() {
        let result = OntologyLoader::load_from_str("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn classification_reaches_every_domain_ontology_but_not_filing_axes() {
        // Phase 10c regression: classification_domains() used to chain only
        // human+custom, leaving machine/agent/office/engineering authored but
        // unscoreable (~360 invisible entries). Industrial text then fell back
        // to human cells even when a perfect machine entry existed.
        let config = PhysisConfig::default();
        let loader = OntologyLoader::load_all(&config);
        let reachable: Vec<&str> = loader
            .classification_domains()
            .map(|d| d.name.as_str())
            .collect();
        for name in ["OEE", "Error Recovery"] {
            assert!(
                reachable.iter().any(|n| n.contains(name)),
                "`{name}` must be scoreable, not just displayed"
            );
        }
        // The filing axes stay out by design (Phase 10): files are not *about*
        // "Warehouse" or "Sprint".
        assert!(
            !reachable
                .iter()
                .any(|n| n == &"Warehouse" || n == &"Sprint"),
            "place/period entries must not become classification themes"
        );
        // And the count must track what the classifier receives.
        let total: usize = loader.all_domains().values().count();
        assert!(
            reachable.len() > total / 2,
            "classification lost reach: {}/{}",
            reachable.len(),
            total
        );
    }

    #[test]
    fn test_builtin_office_ontology_count() {
        let map = OntologyLoader::load_builtin(OFFICE_ONTOLOGY_NAME).unwrap();
        assert!(
            map.len() >= 60,
            "expected at least 60 office domains, got {}",
            map.len()
        );
    }

    #[test]
    fn test_builtin_space_and_time_ontologies_load() {
        let space = OntologyLoader::load_builtin(SPACE_ONTOLOGY_NAME).unwrap();
        let time = OntologyLoader::load_builtin(TIME_ONTOLOGY_NAME).unwrap();
        assert!(
            space.len() >= 20,
            "expected at least 20 places, got {}",
            space.len()
        );
        assert!(
            time.len() >= 15,
            "expected at least 15 periods, got {}",
            time.len()
        );
    }

    #[test]
    fn test_space_time_loaded_into_dedicated_buckets() {
        let config = PhysisConfig::default();
        let loader = OntologyLoader::load_all(&config);
        assert!(
            !loader.space_domains.is_empty(),
            "place entries must land in their own bucket"
        );
        assert!(
            !loader.time_domains.is_empty(),
            "period entries must land in their own bucket"
        );
        // Filing axes are NOT classification themes: none of their entry names
        // may appear in the custom (classification) bucket.
        for (bucket, other) in [
            (&loader.space_domains, &loader.custom_domains),
            (&loader.time_domains, &loader.custom_domains),
        ] {
            for name in bucket.keys() {
                assert!(
                    !other.contains_key(name),
                    "space/time entry leaked into classification themes: {name}"
                );
            }
        }
    }

    #[test]
    fn test_space_time_have_valid_grid_cells() {
        let valid_domains = ["STUDY", "HEAL", "FABRICATE", "CONSTRUCT", "BOND"];
        let valid_modes = [
            "WORK",
            "WALK",
            "LEARN",
            "CREATE",
            "REST",
            "LIFT",
            "DESTROY",
            "SENSE",
            "GUIDE",
            "PLAY",
            "BRAINSTORM",
            "MAINTAIN",
            "MOVE",
            "PLAN",
        ];
        for (axis, name_const) in [
            ("place", SPACE_ONTOLOGY_NAME),
            ("period", TIME_ONTOLOGY_NAME),
        ] {
            let map = OntologyLoader::load_builtin(name_const).unwrap();
            for (name, def) in &map {
                let d = def.domain.as_deref().unwrap_or("");
                let m = def.mode.as_deref().unwrap_or("");
                assert!(
                    valid_domains.contains(&d),
                    "{name}: domain `{d}` not on the grid"
                );
                assert!(
                    valid_modes.contains(&m),
                    "{name}: mode `{m}` not on the grid"
                );
                assert_eq!(
                    def.axis_kind.as_deref(),
                    Some(axis),
                    "{name}: wrong axis_kind"
                );
            }
        }
    }

    #[test]
    fn test_industry_packs_load_from_data_dir_and_broken_ones_are_skipped() {
        // T4 / PLAN 4.2-lite: packs are data, not code — drop JSON into
        // {data_dir}/packs/ and they join the classification themes.
        let dir = tempfile::tempdir().unwrap();
        let packs = dir.path().join("packs");
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/examples/logistics_pack.json"
            ),
            packs.join("logistics_pack.json"),
        )
        .unwrap();
        std::fs::write(packs.join("broken.json"), "{not json").unwrap();

        let config = PhysisConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let loader = OntologyLoader::load_all(&config);

        assert!(
            loader
                .custom_domains
                .contains_key("logistics_pack:Freight Booking"),
            "pack entries join custom_domains keyed pack:name; got {:?}",
            loader
                .custom_domains
                .keys()
                .filter(|k| k.contains("Logistics"))
                .take(5)
                .collect::<Vec<_>>()
        );
        assert!(
            !loader.custom_domains.contains_key("broken:anything"),
            "a broken pack must not poison the loader"
        );
        // The pack enriches classification like any extra ontology.
        assert!(loader
            .classification_domains()
            .any(|d| d.name == "Freight Booking"));
    }

    #[test]
    fn test_all_ontologies_loaded() {
        let config = PhysisConfig::default();
        let loader = OntologyLoader::load_all(&config);
        assert!(
            !loader.office_domains.is_empty(),
            "office domains should be loaded"
        );
        assert!(
            !loader.machine_domains.is_empty(),
            "machine domains should be loaded"
        );
        assert!(
            !loader.agent_domains.is_empty(),
            "agent domains should be loaded"
        );
        assert!(
            !loader.human_domains.is_empty(),
            "human domains should be loaded"
        );
        assert!(
            !loader.engineering_domains.is_empty(),
            "engineering domains should be loaded"
        );
    }

    #[test]
    fn test_office_domains_have_valid_grid_cells() {
        let map = OntologyLoader::load_builtin(OFFICE_ONTOLOGY_NAME).unwrap();
        let valid_domains = ["STUDY", "HEAL", "FABRICATE", "CONSTRUCT", "BOND"];
        let valid_modes = [
            "WORK",
            "WALK",
            "LEARN",
            "CREATE",
            "REST",
            "LIFT",
            "DESTROY",
            "SENSE",
            "GUIDE",
            "PLAY",
            "BRAINSTORM",
            "MAINTAIN",
            "MOVE",
            "PLAN",
        ];
        for (name, def) in &map {
            let d = def.domain.as_deref().unwrap_or("");
            let m = def.mode.as_deref().unwrap_or("");
            assert!(
                valid_domains.contains(&d),
                "office domain '{}' has invalid domain '{}'",
                name,
                d
            );
            assert!(
                valid_modes.contains(&m),
                "office domain '{}' has invalid mode '{}'",
                name,
                m
            );
        }
    }

    #[test]
    fn test_machine_ontology_has_new_process_domains() {
        let map = OntologyLoader::load_builtin(MACHINE_ONTOLOGY_NAME).unwrap();
        assert!(
            map.len() >= 100,
            "expected at least 100 machine domains after extension, got {}",
            map.len()
        );
        assert!(
            map.contains_key("Thermal Zone Monitoring"),
            "should contain new ProcessMonitoring domains"
        );
        assert!(
            map.contains_key("Anomaly Detection"),
            "should contain new MachineHealth domains"
        );
        assert!(
            map.contains_key("OEE Metrics"),
            "should contain new ProductionMetrics domains"
        );
    }

    #[test]
    fn test_agent_ontology_has_new_workflow_domains() {
        let map = OntologyLoader::load_builtin(AGENT_ONTOLOGY_NAME).unwrap();
        assert!(
            map.len() >= 60,
            "expected at least 60 agent domains after extension, got {}",
            map.len()
        );
        assert!(
            map.contains_key("System Prompt"),
            "should contain new PromptPipeline domains"
        );
        assert!(
            map.contains_key("Working Memory"),
            "should contain new AgentMemory domains"
        );
        assert!(
            map.contains_key("PDCA Cycle"),
            "should contain new QualityAssurance domains"
        );
    }

    /// Every name this crate selects must exist in the core registry.
    ///
    /// The two lists live in different crates now, so a rename on either side
    /// would otherwise surface as a panic on someone's first run. This turns
    /// that into a test failure here.
    #[test]
    fn every_registered_ontology_resolves() {
        for name in BUILTIN_ONTOLOGY_NAMES {
            assert!(
                physis_core::ontology::ontology_source(name).is_some(),
                "built-in ontology '{name}' is missing from physis_core ONTOLOGY_SOURCES",
            );
        }
        for (kind, src) in EXTRA_ONTOLOGY_NAMES {
            assert!(
                physis_core::ontology::ontology_source(src).is_some(),
                "extra ontology '{kind}' -> '{src}' is missing from physis_core ONTOLOGY_SOURCES",
            );
        }
        // Forcing the Lazy values proves resolution end to end, not just presence.
        assert_eq!(BUILTIN_ONTOLOGIES.len(), BUILTIN_ONTOLOGY_NAMES.len());
        assert_eq!(EXTRA_ONTOLOGIES.len(), EXTRA_ONTOLOGY_NAMES.len());
    }

    /// The corpus has one home; Pro must not grow a second copy again.
    ///
    /// Checks for DUPLICATES of the canonical corpus specifically, rather than
    /// for any ontology JSON at all: a genuinely Pro-only ontology that is not
    /// in the registry is legitimate, a second copy of one that is is the
    /// drift this change removed.
    #[test]
    fn pro_config_holds_no_copy_of_the_core_corpus() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return; // no config/ at all is fine
        };
        let dupes: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .filter(|n| {
                let stem = n.trim_end_matches(".json");
                physis_core::ontology::ontology_source(stem).is_some()
                    || physis_core::ontology::ontology_source(stem.trim_end_matches("_ontology"))
                        .is_some()
            })
            .collect();
        assert!(
            dupes.is_empty(),
            "config/ has a second copy of registry ontologies: {dupes:?} — the \
             corpus lives in physis-core/config and is reached through \
             ONTOLOGY_SOURCES",
        );
    }
}
