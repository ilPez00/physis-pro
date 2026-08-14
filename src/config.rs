//! Configuration loading — builds PhysisConfig and loads ontologies from
//! built-in JSON, file paths, or raw strings into domain maps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::*;
use crate::ontology::*;
use crate::ontology_nonhuman::MACHINE_ONTOLOGY_NAME;
use crate::ontology::OFFICE_ONTOLOGY_NAME;

/// The named built-in ontologies, resolvable by `OntologyLoader::load_builtin`.
/// To add one: drop a JSON file in `config/`, register its name constant, add a
/// row here and (if it needs its own domain map) a `kind` arm in `load_all`.
static BUILTIN_ONTOLOGIES: &[(&str, &str)] = &[
    (HUMAN_ONTOLOGY_NAME, include_str!("../config/praxis_ontology.json")),
    (MACHINE_ONTOLOGY_NAME, include_str!("../config/machine_ontology.json")),
    (SEMIOTIC_ONTOLOGY_NAME, include_str!("../config/semiotic_ontology.json")),
    (CATEGORY_ONTOLOGY_NAME, include_str!("../config/category_ontology.json")),
    (AGENT_ONTOLOGY_NAME, include_str!("../config/agent_ontology.json")),
    (NATURAL_ONTOLOGY_NAME, include_str!("../config/natural_ontology.json")),
    (SOCIAL_ONTOLOGY_NAME, include_str!("../config/social_ontology.json")),
    (ABSTRACT_ONTOLOGY_NAME, include_str!("../config/abstract_ontology.json")),
    (ENGINEERING_ONTOLOGY_NAME, include_str!("../config/engineering_ontology.json")),
    (OFFICE_ONTOLOGY_NAME, include_str!("../config/office_ontology.json")),
];

/// Extra domain-specific ontologies, baked in at compile time. Each contributes
/// entries to `custom_domains` and is always loaded (CWD-independent). To add a
/// new ontology: drop a JSON file in `config/` (using the HEAL/CONSTRUCT/FABRICATE/
/// BOND/STUDY × LIFT/REST/WALK/WORK/CREATE/LEARN vocabulary) and add one line here.
static EXTRA_ONTOLOGIES: &[(&str, &str)] = &[
    ("software_dev", include_str!("../config/software_dev_ontology.json")),
    ("devops", include_str!("../config/devops_ontology.json")),
    ("data_science", include_str!("../config/data_science_ontology.json")),
    ("cybersecurity", include_str!("../config/cybersecurity_ontology.json")),
    ("health_fitness", include_str!("../config/health_fitness_ontology.json")),
    ("nutrition_cooking", include_str!("../config/nutrition_cooking_ontology.json")),
    ("personal_finance", include_str!("../config/personal_finance_ontology.json")),
    ("learning_education", include_str!("../config/learning_education_ontology.json")),
    ("creative_writing", include_str!("../config/creative_writing_ontology.json")),
    ("music", include_str!("../config/music_ontology.json")),
    ("visual_arts", include_str!("../config/visual_arts_ontology.json")),
    ("relationships", include_str!("../config/relationships_ontology.json")),
    ("productivity", include_str!("../config/productivity_ontology.json")),
    ("travel", include_str!("../config/travel_ontology.json")),
    ("home_maintenance", include_str!("../config/home_maintenance_ontology.json")),
    ("mindfulness_spirituality", include_str!("../config/mindfulness_spirituality_ontology.json")),
    ("science_research", include_str!("../config/science_research_ontology.json")),
    ("business_entrepreneurship", include_str!("../config/business_entrepreneurship_ontology.json")),
    ("language_learning", include_str!("../config/language_learning_ontology.json")),
    ("outdoors_sports", include_str!("../config/outdoors_sports_ontology.json")),
    // Targeted fill for the empty/thin DOMAIN×MODE cells (Phase 6F) so every
    // grid cell is a reachable classification target.
    ("grid_fill", include_str!("../config/grid_fill_ontology.json")),
    // One MODE-pure anchor per cell — sharpens the 6-mode axis (same-domain modes
    // were noisy, e.g. heavy lifting leaking to WALK). Phase 6F.
    ("mode_anchors", include_str!("../config/mode_anchors_ontology.json")),
    // Italian-language anchors: an Italian query against an Italian entry is
    // same-language matching, which sidesteps the cross-lingual weakness of the
    // English embedder (measured Italian tier 0.733 → see INDUSTRIAL_VERTICAL_PLAN
    // step 4). Vocabulary only — never fixture sentences (held-out test honesty).
    ("italian", include_str!("../config/italian_industrial_ontology.json")),
];

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
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut physis = PhysisConfig::default();
        // Disable Sanskrit dreaming and Wenyan compression by default for business use
        physis.linguistic.wenyan_enabled = false;
        physis.linguistic.sanskrit_enabled = false;
        Self {
            physis,
            ingest_dirs: vec![PathBuf::from(&home).join("inbox")],
            wiki_path: Some(PathBuf::from(&home).join(".physis").join("wiki")),
            industrial_endpoints: vec![],
            inventory_db: PathBuf::from(&home).join(".physis").join("inventory"),
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

fn return_true() -> bool { true }

/// Multi-embedder configuration block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
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
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            data_dir: PathBuf::from(home).join(DEFAULT_PHYSIS_DIR),
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
    /// Custom domain definitions loaded from file paths.
    pub custom_domains: HashMap<String, DomainDef>,
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
            custom_domains: HashMap::new(),
        }
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
        let contents = std::fs::read_to_string(path)?;
        let config: OntologyConfig = serde_json::from_str(&contents)?;
        Ok(Self::entries_to_map(&config.domains))
    }

    /// Parse domain definitions from a raw JSON string.
    pub fn load_from_str(json: &str) -> anyhow::Result<HashMap<String, DomainDef>> {
        let config: OntologyConfig = serde_json::from_str(json)?;
        Ok(Self::entries_to_map(&config.domains))
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
                    Self::load_from_path(path).unwrap_or_else(|e| {
                        eprintln!("Warning: failed to load ontology '{}' from {}: {}", source.name, path.display(), e);
                        HashMap::new()
                    })
                } else {
                    eprintln!("Warning: ontology path {} not found", path.display());
                    HashMap::new()
                }
            } else {
                Self::load_builtin(&source.name).unwrap_or_default()
            };

            for (name, def) in map {
                match source.kind.as_str() {
                    "human" => { loader.human_domains.insert(name, def); }
                    "machine" => { loader.machine_domains.insert(name, def); }
                    "semiotic" => { loader.semiotic_domains.insert(name, def); }
                    "category" => { loader.category_domains.insert(name, def); }
                    "agent" => { loader.agent_domains.insert(name, def); }
                    "natural" => { loader.natural_domains.insert(name, def); }
                    "social" => { loader.social_domains.insert(name, def); }
                    "abstract" => { loader.abstract_domains.insert(name, def); }
                    "engineering" => { loader.engineering_domains.insert(name, def); }
                    "office" => { loader.office_domains.insert(name, def); }
                    _ => { loader.custom_domains.insert(name, def); }
                }
            }
        }

        // Always-on extra domain-specific ontologies → custom_domains.
        // Keyed by "kind:name" so identical entry names across files don't collide.
        for (kind, json) in EXTRA_ONTOLOGIES {
            match Self::load_from_str(json) {
                Ok(map) => {
                    for (name, def) in map {
                        loader.custom_domains.insert(format!("{kind}:{name}"), def);
                    }
                }
                Err(e) => eprintln!("Warning: failed to parse built-in ontology '{kind}': {e}"),
            }
        }

        loader
    }

    /// All domain definitions used for classification: the human (praxis) grid
    /// plus every extra domain-specific ontology in `custom_domains`. All share
    /// the HEAL/CONSTRUCT/FABRICATE/BOND/STUDY × LIFT/REST/WALK/WORK/CREATE/LEARN
    /// vocabulary, so they enrich the same semiotic grid.
    pub fn classification_domains(&self) -> impl Iterator<Item = &DomainDef> {
        self.human_domains.values().chain(self.custom_domains.values())
    }

    /// Every domain definition across all loaded ontologies, merged into one map.
    /// Seeds theme nodes for the file-graph view. On key collision the later map
    /// wins (custom overrides builtin).
    pub fn all_domains(&self) -> HashMap<String, DomainDef> {
        let mut out = HashMap::new();
        for m in [
            &self.human_domains, &self.machine_domains, &self.semiotic_domains,
            &self.category_domains, &self.agent_domains, &self.natural_domains,
            &self.social_domains, &self.abstract_domains, &self.engineering_domains,
            &self.office_domains, &self.custom_domains,
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
        assert!(map.len() >= 14, "expected at least 14 human domains, got {}", map.len());
    }

    #[test]
    fn test_builtin_machine_ontology_count() {
        let map = OntologyLoader::load_builtin(MACHINE_ONTOLOGY_NAME).unwrap();
        assert!(map.len() >= 50, "expected at least 50 machine domains, got {}", map.len());
    }

    #[test]
    fn test_domain_resolution_exact() {
        let map = OntologyLoader::load_builtin(HUMAN_ONTOLOGY_NAME).unwrap();
        let def = map.get("Body & Fitness");
        assert!(def.is_some(), "Body & Fitness should be found");
        if let Some(d) = def {
            assert!(d.hints.iter().any(|h| h == "exercise"), "expected 'exercise' hint");
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
    fn test_builtin_office_ontology_count() {
        let map = OntologyLoader::load_builtin(OFFICE_ONTOLOGY_NAME).unwrap();
        assert!(map.len() >= 60, "expected at least 60 office domains, got {}", map.len());
    }

    #[test]
    fn test_all_ontologies_loaded() {
        let config = PhysisConfig::default();
        let loader = OntologyLoader::load_all(&config);
        assert!(!loader.office_domains.is_empty(), "office domains should be loaded");
        assert!(!loader.machine_domains.is_empty(), "machine domains should be loaded");
        assert!(!loader.agent_domains.is_empty(), "agent domains should be loaded");
        assert!(!loader.human_domains.is_empty(), "human domains should be loaded");
        assert!(!loader.engineering_domains.is_empty(), "engineering domains should be loaded");
    }

    #[test]
    fn test_office_domains_have_valid_grid_cells() {
        let map = OntologyLoader::load_builtin(OFFICE_ONTOLOGY_NAME).unwrap();
        let valid_domains = ["STUDY", "HEAL", "FABRICATE", "CONSTRUCT", "BOND"];
        let valid_modes = [
            "WORK", "WALK", "LEARN", "CREATE", "REST", "LIFT",
            "DESTROY", "SENSE", "GUIDE", "PLAY", "BRAINSTORM",
            "MAINTAIN", "MOVE", "PLAN",
        ];
        for (name, def) in &map {
            let d = def.domain.as_deref().unwrap_or("");
            let m = def.mode.as_deref().unwrap_or("");
            assert!(valid_domains.contains(&d), "office domain '{}' has invalid domain '{}'", name, d);
            assert!(valid_modes.contains(&m), "office domain '{}' has invalid mode '{}'", name, m);
        }
    }

    #[test]
    fn test_machine_ontology_has_new_process_domains() {
        let map = OntologyLoader::load_builtin(MACHINE_ONTOLOGY_NAME).unwrap();
        assert!(map.len() >= 100, "expected at least 100 machine domains after extension, got {}", map.len());
        assert!(map.contains_key("Thermal Zone Monitoring"), "should contain new ProcessMonitoring domains");
        assert!(map.contains_key("Anomaly Detection"), "should contain new MachineHealth domains");
        assert!(map.contains_key("OEE Metrics"), "should contain new ProductionMetrics domains");
    }

    #[test]
    fn test_agent_ontology_has_new_workflow_domains() {
        let map = OntologyLoader::load_builtin(AGENT_ONTOLOGY_NAME).unwrap();
        assert!(map.len() >= 60, "expected at least 60 agent domains after extension, got {}", map.len());
        assert!(map.contains_key("System Prompt"), "should contain new PromptPipeline domains");
        assert!(map.contains_key("Working Memory"), "should contain new AgentMemory domains");
        assert!(map.contains_key("PDCA Cycle"), "should contain new QualityAssurance domains");
    }
}
