use crate::models::*;

pub const HUMAN_ONTOLOGY_NAME: &str = "praxis_ontology";
pub const HUMAN_TYPE: &str = "human";

pub fn parse_ontology_entry(entry: &OntologyEntry) -> DomainDef {
    DomainDef {
        name: entry.name.clone(),
        category: entry.category.clone(),
        domain: Some(entry.domain.clone()),
        mode: Some(entry.mode.clone()),
        axis_kind: Some(entry.axis_kind.clone()),
        axis_name: Some(entry.axis_name.clone()),
        unit: entry.unit.clone(),
        hints: entry.hints.clone(),
        facets: entry.facets.clone(),
    }
}

pub const SEMIOTIC_ONTOLOGY_NAME: &str = "semiotic_ontology";
pub const CATEGORY_ONTOLOGY_NAME: &str = "category_ontology";
pub const AGENT_ONTOLOGY_NAME: &str = "agent_ontology";
pub const NATURAL_ONTOLOGY_NAME: &str = "natural_ontology";
pub const SOCIAL_ONTOLOGY_NAME: &str = "social_ontology";
pub const ABSTRACT_ONTOLOGY_NAME: &str = "abstract_ontology";
pub const ENGINEERING_ONTOLOGY_NAME: &str = "engineering_ontology";
pub const OFFICE_ONTOLOGY_NAME: &str = "office_ontology";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_exist() {
        assert_eq!(HUMAN_ONTOLOGY_NAME, "praxis_ontology");
        assert_eq!(HUMAN_TYPE, "human");
    }

    #[test]
    fn test_parse_ontology_entry() {
        let entry = OntologyEntry {
            name: "logic".into(),
            category: Some("reasoning".into()),
            domain: "reasoning".into(),
            mode: "COHERENCE".into(),
            axis_kind: "epistemic".into(),
            axis_name: "formal".into(),
            unit: "tokens".into(),
            hints: vec!["therefore".into(), "implies".into()],
            facets: Facets::default(),
        };
        let def = parse_ontology_entry(&entry);
        assert_eq!(def.name, "logic");
        assert_eq!(def.category.as_deref(), Some("reasoning"));
        assert_eq!(def.unit, "tokens");
        assert_eq!(def.hints, vec!["therefore", "implies"]);
    }

    #[test]
    fn test_parse_ontology_entry_no_category() {
        let entry = OntologyEntry {
            name: "empty".into(),
            category: None,
            domain: "generic".into(),
            mode: "PLAN".into(),
            axis_kind: "none".into(),
            axis_name: "none".into(),
            unit: "count".into(),
            hints: vec![],
            facets: Facets::default(),
        };
        let def = parse_ontology_entry(&entry);
        assert!(def.category.is_none());
        assert!(def.hints.is_empty());
    }
}
