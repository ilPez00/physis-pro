//! Ontology loading — built-in JSON files (compiled in), file paths, or raw
//! strings, resolved into domain maps.
//!
//! The classifier consumes `human_domains` (the base praxis grid) plus
//! `custom_domains` (every extra domain-specific ontology), all sharing the
//! HEAL/CONSTRUCT/FABRICATE/BOND/STUDY × LIFT/…/PLAN vocabulary.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;

use crate::models::{DomainDef, OntologyConfig, OntologyEntry};

pub const HUMAN_ONTOLOGY_NAME: &str = "praxis_ontology";

/// The named built-in ontologies. To add one: drop a JSON file in `config/`,
/// register its name constant, add a row here and a `kind` arm in `load_all`.
pub static BUILTIN_ONTOLOGIES: &[(&str, &str, &str)] = &[
    ("praxis", "praxis", include_str!("../config/praxis_ontology.json")),
    ("machine", "machine", include_str!("../config/machine_ontology.json")),
    ("semiotic", "semiotic", include_str!("../config/semiotic_ontology.json")),
    ("office", "office", include_str!("../config/office_ontology.json")),
    ("agent", "agent", include_str!("../config/agent_ontology.json")),
    ("engineering", "engineering", include_str!("../config/engineering_ontology.json")),
];

/// Extra domain-specific ontologies, always loaded into `custom_domains`.
/// To add one: drop a JSON file in `config/` using the grid vocabulary.
pub static EXTRA_ONTOLOGIES: &[(&str, &str)] = &[
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
    ("science_research", include_str!("../config/science_research_ontology.json")),
    ("industrial_semiconductor", include_str!("../config/industrial_semiconductor_ontology.json")),
    ("business_entrepreneurship", include_str!("../config/business_entrepreneurship_ontology.json")),
    ("language_learning", include_str!("../config/language_learning_ontology.json")),
    ("outdoors_sports", include_str!("../config/outdoors_sports_ontology.json")),
    // Targeted fill for empty/thin DOMAIN×MODE cells so every grid cell is a
    // reachable classification target.
    ("grid_fill", include_str!("../config/grid_fill_ontology.json")),
    // One MODE-pure anchor per cell — sharpens the mode axis.
    ("mode_anchors", include_str!("../config/mode_anchors_ontology.json")),
    // Italian-language anchors for Italian queries on shop floors.
    ("italian", include_str!("../config/italian_industrial_ontology.json")),
];

/// Loads and resolves ontologies into domain maps.
#[derive(Debug, Clone, Default)]
pub struct OntologyLoader {
    pub human_domains: HashMap<String, DomainDef>,
    pub machine_domains: HashMap<String, DomainDef>,
    pub custom_domains: HashMap<String, DomainDef>,
}

impl OntologyLoader {
    pub fn new() -> Self {
        Self::default()
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

    /// Parse domain definitions from a raw JSON string.
    pub fn load_from_str(json: &str) -> anyhow::Result<HashMap<String, DomainDef>> {
        let config: OntologyConfig = serde_json::from_str(json)?;
        Ok(Self::entries_to_map(&config.domains))
    }

    /// Load domain definitions from a JSON file on disk.
    pub fn load_from_path(path: &Path) -> anyhow::Result<HashMap<String, DomainDef>> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("reading ontology {}", path.display()))?;
        Self::load_from_str(&contents)
    }

    /// Load all built-in ontologies into human/machine/custom maps.
    pub fn load_all() -> Self {
        let mut loader = Self::new();

        for (kind, _name, json) in BUILTIN_ONTOLOGIES {
            let Ok(map) = Self::load_from_str(json) else {
                eprintln!("Warning: failed to parse built-in ontology '{kind}'");
                continue;
            };
            match *kind {
                "human" | "praxis" => {
                    for (k, v) in map {
                        loader.human_domains.insert(k, v);
                    }
                }
                "machine" => {
                    for (k, v) in map {
                        loader.machine_domains.insert(k, v);
                    }
                }
                _ => {
                    for (k, v) in map {
                        loader.custom_domains.insert(k, v);
                    }
                }
            }
        }

        // Extra domain-specific ontologies → custom_domains, keyed "kind:name"
        // so identical entry names across files don't collide.
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

    /// Merge a custom ontology (e.g. hand-edited in the studio) into custom_domains.
    pub fn merge_custom(&mut self, kind: &str, map: HashMap<String, DomainDef>) {
        for (name, def) in map {
            self.custom_domains.insert(format!("{kind}:{name}"), def);
        }
    }

    /// All domain definitions used for classification: the human grid plus every
    /// extra domain-specific ontology. All share the grid vocabulary.
    pub fn classification_domains(&self) -> impl Iterator<Item = &DomainDef> {
        self.human_domains.values().chain(self.custom_domains.values())
    }

    /// Every domain definition across all loaded ontologies, merged into one map.
    /// On key collision the later map wins (custom overrides builtin).
    pub fn all_domains(&self) -> HashMap<String, DomainDef> {
        let mut out = HashMap::new();
        for m in [&self.human_domains, &self.machine_domains, &self.custom_domains] {
            for (k, v) in m {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    /// Distinct categories across all domains (for the studio browser).
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .all_domains()
            .values()
            .filter_map(|d| d.category.clone())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }

    /// Total number of classification-relevant domains (human + custom).
    pub fn entry_count(&self) -> usize {
        self.human_domains.len() + self.custom_domains.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_human_ontology_loads() {
        let loader = OntologyLoader::load_all();
        assert!(loader.human_domains.len() >= 14, "expected >=14 human domains, got {}", loader.human_domains.len());
        assert!(loader.entry_count() > 100, "expected a rich grid, got {}", loader.entry_count());
        assert!(loader.custom_domains.len() > 50, "expected extra domains");
    }

    #[test]
    fn load_from_str_parses_entry() {
        let json = r#"{"kind":"test","domains":[{"name":"logic","category":"reasoning","domain":"STUDY","mode":"WORK","axis_kind":"epistemic","axis_name":"formal","unit":"tokens","hints":["therefore","implies"]}]}"#;
        let map = OntologyLoader::load_from_str(json).unwrap();
        assert_eq!(map.len(), 1);
        let d = &map["logic"];
        assert_eq!(d.domain.as_deref(), Some("STUDY"));
        assert_eq!(d.hints, vec!["therefore", "implies"]);
    }

    #[test]
    fn categories_are_distinct() {
        let loader = OntologyLoader::load_all();
        let cats = loader.categories();
        let mut sorted = cats.clone();
        sorted.dedup();
        assert_eq!(cats, sorted);
        assert!(cats.iter().any(|c| c.to_lowercase().contains("reason")));
    }

    #[test]
    fn faceted_example_loads_with_facets() {
        let json = include_str!("../config/ontology_facets.example.json");
        let map = OntologyLoader::load_from_str(json).unwrap();
        let pump = &map["Pump Maintenance"];
        assert_eq!(pump.facets.lifecycle, Some(crate::models::LifecyclePhase::Operate));
        assert_eq!(pump.facets.agency, Some(crate::models::Agency::Automated));
        assert_eq!(pump.facets.sub_domain.as_deref(), Some("Repair"));
        // A facet-less entry in the example still parses (back-compat default).
        assert!(map.contains_key("Contract Negotiation"));
    }
}
