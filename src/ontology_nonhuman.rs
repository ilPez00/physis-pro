use crate::models::*;

pub const MACHINE_ONTOLOGY_NAME: &str = "machine_ontology";
pub const MACHINE_TYPE: &str = "machine";

pub fn resolve_nonhuman_domain<'a>(
    name: &str,
    domains: &'a std::collections::HashMap<String, DomainDef>,
) -> Option<&'a DomainDef> {
    let lower = name.to_lowercase();

    if let Some(def) = domains.get(name) {
        return Some(def);
    }

    for def in domains.values() {
        if def.hints.iter().any(|h| lower.contains(h)) {
            return Some(def);
        }
    }

    domains.values().find(|&def| lower.contains(&def.name.to_lowercase())).map(|v| v as _)
}

pub fn enrich_nonhuman_goal(
    goal: &Goal,
    domains: &std::collections::HashMap<String, DomainDef>,
) -> String {
    let def = resolve_nonhuman_domain(&goal.id, domains);
    match def {
        Some(d) => {
            format!(
                "• \"{}\" [{} +{}] progress={}%  [MACHINE]",
                goal.id,
                d.name,
                d.unit,
                (goal.progress * 100.0) as u32
            )
        }
        None => format!(
            "• \"{}\" [UNKNOWN_MACHINE_DOMAIN] progress={}%",
            goal.id,
            (goal.progress * 100.0) as u32
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_domains() -> HashMap<String, DomainDef> {
        [(
            "testing".into(),
            DomainDef {
                name: "testing".into(),
                category: Some("qa".into()),
                domain: Some("STUDY".into()),
                mode: Some("WALK".into()),
                axis_kind: Some("machine".into()),
                axis_name: Some("operational".into()),
                unit: "tests".into(),
                hints: vec!["assert".into(), "verify".into()],
                facets: Facets::default(),
            },
        )]
        .into()
    }

    #[test]
    fn test_machine_constants() {
        assert_eq!(MACHINE_ONTOLOGY_NAME, "machine_ontology");
        assert_eq!(MACHINE_TYPE, "machine");
    }

    #[test]
    fn test_resolve_nonhuman_by_exact_name() {
        let domains = sample_domains();
        let result = resolve_nonhuman_domain("testing", &domains);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "testing");
    }

    #[test]
    fn test_resolve_nonhuman_by_hint() {
        let domains = sample_domains();
        let result = resolve_nonhuman_domain("assert_all_the_things", &domains);
        assert!(result.is_some(), "should match via 'assert' hint");
        assert_eq!(result.unwrap().name, "testing");
    }

    #[test]
    fn test_resolve_nonhuman_by_name_lowercase() {
        let domains = sample_domains();
        let result = resolve_nonhuman_domain("Testing stuff", &domains);
        assert!(result.is_some(), "should match via case-insensitive name");
        assert_eq!(result.unwrap().name, "testing");
    }

    #[test]
    fn test_resolve_nonhuman_unknown() {
        let domains = sample_domains();
        let result = resolve_nonhuman_domain("unknown_thing", &domains);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_nonhuman_empty_domains() {
        let domains = HashMap::new();
        let result = resolve_nonhuman_domain("anything", &domains);
        assert!(result.is_none());
    }

    #[test]
    fn test_enrich_nonhuman_goal_found() {
        let domains = sample_domains();
        let goal = Goal::new_vec(vec![0.1, 0.2]);
        let goal = Goal {
            id: "testing".into(),
            ..goal
        };
        let s = enrich_nonhuman_goal(&goal, &domains);
        assert!(s.contains("[testing +tests]"));
        assert!(s.contains("[MACHINE]"));
        assert!(s.contains("progress=0%"));
    }

    #[test]
    fn test_enrich_nonhuman_goal_unknown() {
        let domains = sample_domains();
        let goal = Goal::new_vec(vec![0.1, 0.2]);
        let s = enrich_nonhuman_goal(&goal, &domains);
        assert!(s.contains("[UNKNOWN_MACHINE_DOMAIN]"));
    }

    #[test]
    fn test_enrich_nonhuman_goal_progress_format() {
        let domains = sample_domains();
        let goal = Goal {
            id: "verify_ok".into(),
            embedding: vec![0.5; 4],
            progress: 0.756,
            domain_name: "machine".to_string(),
            label: None,
        };
        let s = enrich_nonhuman_goal(&goal, &domains);
        assert!(s.contains("progress=75%"));
    }
}
