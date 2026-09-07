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
    /// Directory for persistent data (hash cache, logs, etc.).
    pub data_dir: PathBuf,
    /// Ontology sources to load at startup.
    pub ontologies: Vec<OntologySource>,
    /// Seconds between network scans when watching directories.
    pub network_scan_interval_secs: u64,
    /// Number of dreams to generate per dream cycle.
    pub dream_batch_size: usize,
    /// Progress threshold below which a goal is flagged stagnant.
    pub pdca_stagnant_threshold: f32,
    /// Number of recent PDCA actions to inspect for stagnation.
    pub pdca_stagnant_window: usize,
    /// Directories to watch for file changes.
    pub watch_dirs: Vec<PathBuf>,
    /// Linguistic lense toggles (Wenyan, Piraha, Sanskrit).
    pub linguistic: LinguisticConfig,
    /// ONNX embedder tuning parameters.
    pub onnx: OnnxConfig,
    /// Embedding vector dimension (should match model).
    pub embed_dim: usize,
    /// Multi-embedder configuration (optional, overrides onnx fallback).
    pub embedders: EmbeddersConfig,
    /// Agent-orchestration configuration (ReAct loop, OpenCode coding sub-agent,
    /// tool palette). Drives the /api/v1/agent/run endpoint + /agent dashboard.
    #[serde(default)]
    pub agent: AgentRuntimeConfig,
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
    pub name: String,
    /// Filesystem path (None for built-in ontologies).
    pub path: Option<PathBuf>,
    /// Ontology kind: "human", "machine", or custom.
    pub kind: String,
    /// Whether this source is loaded at startup.
    pub enabled: bool,
}

impl Default for PhysisConfig {
    fn default() -> Self {
        let data_dir = std::env::var("PHYSIS_DATA_DIR")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                cwd.join(DEFAULT_PHYSIS_DIR)
            });
        Self {
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
        }
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
